//! Redes que o host do agente aceita como alvo de equipamento — plugins
//! (`Command::DeviceIo`) e testes de vazão (`Command::Throughput`).
//!
//! Sempre rede privada, CGNAT ou link-local
//! ([`crate::services::plugins::transport::is_reachable_target`]); com
//! `AGENT_DEVICE_CIDRS=192.168.10.0/24,10.20.0.0/16`, só dentro dessas faixas.
//! Um IP público nunca é aceito, com ou sem a lista.

use std::net::IpAddr;

use ipnet::IpNet;

use crate::services::{
    agents::protocol::{ErrorCode, RemoteError},
    plugins::transport::target_allowed,
};

/// Variável com as faixas extras aceitas pelo host.
pub const DEVICE_CIDRS_ENV: &str = "AGENT_DEVICE_CIDRS";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeviceNetworks(Vec<IpNet>);

impl DeviceNetworks {
    #[must_use]
    pub const fn new(cidrs: Vec<IpNet>) -> Self {
        Self(cidrs)
    }

    /// Lê `AGENT_DEVICE_CIDRS`; faixa inválida é ignorada.
    #[must_use]
    pub fn from_env() -> Self {
        Self::parse(&std::env::var(DEVICE_CIDRS_ENV).unwrap_or_default())
    }

    #[must_use]
    pub fn parse(raw: &str) -> Self {
        Self(
            raw.split(',')
                .filter_map(|cidr| cidr.trim().parse().ok())
                .collect(),
        )
    }

    /// # Errors
    ///
    /// `Forbidden` quando `ip` está fora das redes que este host aceita.
    pub fn check(&self, ip: IpAddr) -> Result<(), RemoteError> {
        if target_allowed(ip, &self.0) {
            return Ok(());
        }
        Err(RemoteError::new(
            ErrorCode::Forbidden,
            format!("O agente não acessa {ip}: fora das redes permitidas neste host"),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ip_publico_nunca_e_alvo() {
        let error = DeviceNetworks::default()
            .check("8.8.8.8".parse().unwrap())
            .expect_err("público");
        assert_eq!(error.code, ErrorCode::Forbidden);
        assert!(DeviceNetworks::default()
            .check("192.168.1.1".parse().unwrap())
            .is_ok());
    }

    #[test]
    fn lista_do_host_restringe_a_rede_privada() {
        let networks = DeviceNetworks::parse("192.168.10.0/24, lixo ,10.20.0.0/16");
        assert!(networks.check("192.168.10.7".parse().unwrap()).is_ok());
        assert!(networks.check("10.20.3.4".parse().unwrap()).is_ok());
        assert!(networks.check("192.168.11.7".parse().unwrap()).is_err());
        assert!(networks.check("127.0.0.1".parse().unwrap()).is_err());
    }
}
