//! Ponte de banco no agente (ADR 013): quem abre o socket até o banco.
//!
//! O agente não fala SQL — só conecta e copia bytes ([`crate::services::agents::tunnel::pump`]).
//! O que ele decide, e decide sozinho, é **se** conecta:
//!
//! 1. a política local precisa liberar `database` (`AGENT_ALLOW`);
//! 2. o destino precisa estar em `AGENT_DATABASE_TARGETS` — lista obrigatória,
//!    vazia recusa tudo; a central não a amplia;
//! 3. o IP resolvido precisa ser de rede privada/CGNAT/link-local, a mesma
//!    regra do `device_io`.

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
    },
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

/// Quem abre a conexão até o banco. Trait para os testes trocarem a rede.
#[async_trait]
pub trait TunnelOpener: Send + Sync {
    async fn open(&self, host: &str, port: u16) -> Result<TcpStream, RemoteError>;
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
    async fn open(&self, host: &str, port: u16) -> Result<TcpStream, RemoteError> {
        if !self.policy.allows(Permission::Database) {
            return Err(RemoteError::new(
                ErrorCode::Forbidden,
                "A política deste host não libera a permissão 'database'",
            ));
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
        Ok(stream)
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
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::Forbidden);
        let error = opener(&[Permission::Database], "")
            .open("127.0.0.1", 5432)
            .await
            .unwrap_err();
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
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::Forbidden);
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
