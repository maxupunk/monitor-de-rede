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
    str::FromStr,
    time::Duration,
};

use async_trait::async_trait;
use tokio::net::TcpStream;

use crate::services::{
    agents::{
        policy::{Permission, Policy},
        protocol::{ErrorCode, RemoteError},
        tunnel::TunnelStream,
    },
    docker::{maintenance::DockerMaintenance, source::LocalEngine},
    plugins::transport::is_reachable_target,
};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Um destino liberado: um IP ou uma faixa, numa porta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunnelTarget {
    pub net: ipnet::IpNet,
    pub port: u16,
}

impl TunnelTarget {
    #[must_use]
    pub fn matches(&self, addr: SocketAddr) -> bool {
        self.port == addr.port() && self.net.contains(&addr.ip())
    }
}

impl FromStr for TunnelTarget {
    type Err = String;

    /// `10.0.0.20:5432`, `10.0.0.0/24:3306` ou `[fd00::5]:5432`.
    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let raw = raw.trim();
        let invalid =
            || format!("destino de ponte inválido: '{raw}' (use IP:porta ou faixa/prefixo:porta)");
        let (host, port) = raw.rsplit_once(':').ok_or_else(invalid)?;
        let port: u16 = port.parse().map_err(|_| invalid())?;
        let host = host.trim_start_matches('[').trim_end_matches(']');
        let net = if host.contains('/') {
            host.parse().map_err(|_| invalid())?
        } else {
            ipnet::IpNet::from(host.parse::<IpAddr>().map_err(|_| invalid())?)
        };
        if port == 0 {
            return Err(invalid());
        }
        Ok(Self { net, port })
    }
}

/// Lê `AGENT_DATABASE_TARGETS` (separada por vírgula).
///
/// # Errors
///
/// Qualquer entrada inválida — um erro de digitação não pode virar "nenhum
/// destino" em silêncio, nem liberar outro.
pub fn parse_targets(raw: &str) -> Result<Vec<TunnelTarget>, String> {
    raw.split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(TunnelTarget::from_str)
        .collect()
}

/// Quem abre a ponta local de uma ponte. Trait para os testes trocarem a
/// rede e a Engine.
#[async_trait]
pub trait TunnelOpener: Send + Sync {
    /// Conexão TCP até o banco `host:port`.
    async fn open(&self, host: &str, port: u16) -> Result<TunnelStream, RemoteError>;
    /// O tar do volume `name`. A escrita é descartada: a ponte é só de ida.
    async fn open_volume(&self, name: &str) -> Result<TunnelStream, RemoteError>;
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
}

impl LocalTunnelOpener {
    #[must_use]
    pub const fn new(policy: Policy, targets: Vec<TunnelTarget>) -> Self {
        Self { policy, targets }
    }

    /// O primeiro endereço de `host` que as regras deixam alcançar.
    fn allowed(&self, candidates: &[SocketAddr]) -> Option<SocketAddr> {
        candidates.iter().copied().find(|addr| {
            is_reachable_target(addr.ip())
                && self.targets.iter().any(|target| target.matches(*addr))
        })
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
        let resolved: Vec<SocketAddr> =
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
                .collect();
        let addr = self.allowed(&resolved).ok_or_else(|| {
            RemoteError::new(
                ErrorCode::Forbidden,
                format!("{host}:{port} não está em AGENT_DATABASE_TARGETS deste host"),
            )
        })?;
        let stream = tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(addr))
            .await
            .map_err(|_| {
                RemoteError::new(
                    ErrorCode::Timeout,
                    format!("Tempo esgotado conectando em {addr}"),
                )
            })?
            .map_err(|error| {
                RemoteError::new(
                    ErrorCode::Unavailable,
                    format!("O agente não conectou em {addr}: {error}"),
                )
            })?;
        let _ = stream.set_nodelay(true);
        Ok(Box::new(stream))
    }

    async fn open_volume(&self, name: &str) -> Result<TunnelStream, RemoteError> {
        if !self.policy.allows(Permission::VolumeExport) {
            return Err(forbidden(Permission::VolumeExport));
        }
        let archive = LocalEngine
            .export_volume(name)
            .await
            .map_err(RemoteError::from)?;
        Ok(Box::new(tokio::io::join(archive.reader, tokio::io::sink())))
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
        assert_eq!(opener.allowed(&["8.8.8.8:53".parse().unwrap()]), None);
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
