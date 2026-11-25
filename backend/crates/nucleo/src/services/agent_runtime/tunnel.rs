//! Pontes no agente: quem abre a ponta local — o socket até o banco (ADR 013)
//! ou o tar de um volume (ADR 014).
//!
//! O agente não fala SQL — só conecta e copia bytes ([`crate::services::agents::tunnel::pump`]).
//! O que ele decide, e decide sozinho, é **se** conecta:
//!
//! 1. a política local precisa liberar `database` (`AGENT_ALLOW`);
//! 2. o destino precisa estar em `AGENT_DATABASE_TARGETS` — lista obrigatória,
//!    vazia recusa tudo; a central não a amplia;
//! 3. o IP resolvido precisa ser de rede privada/CGNAT/link-local, a mesma
//!    regra do `device_io`.
//!
//! O volume só exige `volume_export` na política: o nome vai à Engine deste
//! host, que valida a existência, e o tar sai só de ida.

use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use agent_protocol::docker::images::ImageImportRequest;
use async_trait::async_trait;
pub use netmonitor_core::agents::database_targets::{parse_targets, TunnelTarget};
use netmonitor_core::agents::host::{HostDocker, NoHostDocker};
use tokio::net::TcpStream;

use crate::services::{
    agents::{
        policy::{Permission, Policy},
        protocol::{ErrorCode, RemoteError},
        tunnel::TunnelStream,
    },
    shared::cidr::is_reachable_target,
};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Quem abre a ponta local de uma ponte. Trait para os testes trocarem a
/// rede e a Engine.
#[async_trait]
pub trait TunnelOpener: Send + Sync {
    /// Conexão TCP até o banco `host:port`.
    async fn open(&self, host: &str, port: u16) -> Result<TunnelStream, RemoteError>;
    /// O tar do volume `name`. A escrita é descartada: a ponte é só de ida.
    async fn open_volume(&self, name: &str) -> Result<TunnelStream, RemoteError>;
    /// O arquivo de uma imagem compilada aqui (ADR 023). Sem Docker: não há.
    async fn open_artifact(&self, _artifact: uuid::Uuid) -> Result<TunnelStream, RemoteError> {
        Err(RemoteError::new(
            ErrorCode::Unsupported,
            "este agente não compila imagens",
        ))
    }

    /// O `docker load` daqui (ADR 022). Sem Docker: não há.
    async fn open_import(
        &self,
        _request: &ImageImportRequest,
    ) -> Result<TunnelStream, RemoteError> {
        Err(RemoteError::new(
            ErrorCode::Unsupported,
            "este agente não recebe imagens",
        ))
    }
}

fn forbidden(permission: Permission) -> RemoteError {
    RemoteError::new(
        ErrorCode::Forbidden,
        format!("A política deste host não libera a permissão '{permission}'"),
    )
}

/// O de verdade: política + lista + rede privada, e então TCP.
pub struct LocalTunnelOpener {
    policy: Policy,
    targets: Vec<TunnelTarget>,
    /// O Docker deste host, para a exportação de volume (Servidores).
    docker: Arc<dyn HostDocker>,
}

impl LocalTunnelOpener {
    /// Sem Docker: a exportação de volume é recusada.
    #[must_use]
    pub fn new(policy: Policy, targets: Vec<TunnelTarget>) -> Self {
        Self {
            policy,
            targets,
            docker: Arc::new(NoHostDocker),
        }
    }

    /// O Docker deste host, que exporta os volumes.
    #[must_use]
    pub fn with_docker(mut self, docker: Arc<dyn HostDocker>) -> Self {
        self.docker = docker;
        self
    }

    /// Todos os endereços de `host` que as regras deixam alcançar, preservando a ordem.
    fn allowed(&self, candidates: &[SocketAddr]) -> Vec<SocketAddr> {
        candidates
            .iter()
            .copied()
            .filter(|addr| {
                is_reachable_target(addr.ip())
                    && self.targets.iter().any(|target| target.matches(*addr))
            })
            .collect()
    }
}

#[async_trait]
impl TunnelOpener for LocalTunnelOpener {
    async fn open(&self, host: &str, port: u16) -> Result<TunnelStream, RemoteError> {
        if !self.policy.allows(Permission::Database) {
            return Err(forbidden(Permission::Database));
        }
        if self.targets.is_empty() {
            return Err(RemoteError::new(
                ErrorCode::Forbidden,
                "Nenhum banco liberado neste host: defina AGENT_DATABASE_TARGETS (ex.: 10.0.0.20:5432)",
            ));
        }
        let resolved: Vec<SocketAddr> = if let Ok(ip) = host.trim().parse::<IpAddr>() {
            vec![SocketAddr::new(ip, port)]
        } else {
            tokio::time::timeout(CONNECT_TIMEOUT, tokio::net::lookup_host((host, port)))
                .await
                .map_err(|_| {
                    RemoteError::new(
                        ErrorCode::Timeout,
                        format!("Tempo esgotado resolvendo {host}"),
                    )
                })?
                .map_err(|error| {
                    RemoteError::new(
                        ErrorCode::Unavailable,
                        format!("Não foi possível resolver {host}: {error}"),
                    )
                })?
                .collect()
        };
        let allowed = self.allowed(&resolved);
        if allowed.is_empty() {
            return Err(RemoteError::new(
                ErrorCode::Forbidden,
                format!("{host}:{port} não está em AGENT_DATABASE_TARGETS deste host"),
            ));
        }
        let mut last_error = None;
        for addr in allowed {
            match tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(addr)).await {
                Ok(Ok(stream)) => {
                    let _ = stream.set_nodelay(true);
                    return Ok(Box::new(stream));
                }
                Ok(Err(error)) => {
                    last_error = Some(RemoteError::new(
                        ErrorCode::Unavailable,
                        format!("O agente não conectou em {addr}: {error}"),
                    ));
                }
                Err(_) => {
                    last_error = Some(RemoteError::new(
                        ErrorCode::Timeout,
                        format!("Tempo esgotado conectando em {addr}"),
                    ));
                }
            }
        }
        Err(last_error.unwrap_or_else(|| {
            RemoteError::new(
                ErrorCode::Unavailable,
                format!("O agente não conectou em {host}:{port}"),
            )
        }))
    }

    async fn open_volume(&self, name: &str) -> Result<TunnelStream, RemoteError> {
        if !self.policy.allows(Permission::VolumeExport) {
            return Err(forbidden(Permission::VolumeExport));
        }
        self.docker.export_volume(name).await
    }

    async fn open_artifact(&self, artifact: uuid::Uuid) -> Result<TunnelStream, RemoteError> {
        if !self.policy.allows(Permission::Build) {
            return Err(forbidden(Permission::Build));
        }
        self.docker.open_artifact(artifact).await
    }

    async fn open_import(&self, request: &ImageImportRequest) -> Result<TunnelStream, RemoteError> {
        if !self.policy.allows(Permission::Deploy) {
            return Err(forbidden(Permission::Deploy));
        }
        self.docker.open_import(request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_ip_faixa_e_ipv6() {
        let targets = parse_targets("10.0.0.20:5432, 192.168.1.0/24:3306,[fd00::5]:5432").unwrap();
        assert_eq!(targets.len(), 3);
        assert!(targets[0].matches("10.0.0.20:5432".parse().unwrap()));
        assert!(!targets[0].matches("10.0.0.20:3306".parse().unwrap()));
        assert!(targets[1].matches("192.168.1.77:3306".parse().unwrap()));
        assert!(targets[2].matches("[fd00::5]:5432".parse().unwrap()));
    }

    #[test]
    fn entrada_invalida_e_erro_e_nao_silencio() {
        for raw in [
            "10.0.0.20",
            "banco.local:5432",
            "10.0.0.20:0",
            "10.0.0.20:porta",
        ] {
            assert!(parse_targets(raw).is_err(), "aceitou {raw}");
        }
        assert!(parse_targets("").unwrap().is_empty());
    }

    fn opener(permissions: &[Permission], targets: &str) -> LocalTunnelOpener {
        LocalTunnelOpener::new(
            Policy::from_permissions(permissions.iter().copied()),
            parse_targets(targets).unwrap(),
        )
    }

    #[tokio::test]
    async fn sem_permissao_ou_sem_lista_recusa_antes_de_conectar() {
        let error = opener(&[], "127.0.0.1:5432")
            .open("127.0.0.1", 5432)
            .await
            .err()
            .expect("recusado");
        assert_eq!(error.code, ErrorCode::Forbidden);
        let error = opener(&[Permission::Database], "")
            .open("127.0.0.1", 5432)
            .await
            .err()
            .expect("recusado");
        assert!(
            error.message.contains("AGENT_DATABASE_TARGETS"),
            "{}",
            error.message
        );
    }

    #[tokio::test]
    async fn destino_fora_da_lista_e_recusado() {
        let error = opener(&[Permission::Database], "127.0.0.1:5432")
            .open("127.0.0.1", 3306)
            .await
            .err()
            .expect("recusado");
        assert_eq!(error.code, ErrorCode::Forbidden);
    }

    #[tokio::test]
    async fn volume_exige_a_permissao_antes_de_tocar_a_engine() {
        let error = opener(&[Permission::Database], "")
            .open_volume("dados")
            .await
            .err()
            .expect("recusado");
        assert_eq!(error.code, ErrorCode::Forbidden);
        assert!(error.message.contains("volume_export"), "{}", error.message);
    }

    #[test]
    fn ip_publico_nunca_passa_mesmo_listado() {
        let opener = opener(&[Permission::Database], "8.8.8.8:53");
        assert!(opener.allowed(&["8.8.8.8:53".parse().unwrap()]).is_empty());
    }

    #[tokio::test]
    async fn destino_liberado_conecta() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let stream = opener(&[Permission::Database], &format!("127.0.0.1:{port}"))
            .open("127.0.0.1", port)
            .await;
        assert!(stream.is_ok());
    }
}
