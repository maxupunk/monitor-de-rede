//! Redes que o host do agente aceita como alvo de equipamento — plugins
//! (`Command::DeviceIo`) e testes de vazão (`Command::Throughput`).
//!
//! Sempre rede privada, CGNAT ou link-local
//! ([`crate::shared::cidr::is_reachable_target`]); com
//! `AGENT_DEVICE_CIDRS=192.168.10.0/24,10.20.0.0/16`, só dentro dessas faixas.
//! Um IP público nunca é aceito, com ou sem a lista.

use std::net::IpAddr;

use ipnet::IpNet;

use agent_protocol::{ErrorCode, RemoteError};

use crate::shared::cidr::target_allowed;

/// Variável com as faixas extras aceitas pelo host.
pub const DEVICE_CIDRS_ENV: &str = "AGENT_DEVICE_CIDRS";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeviceNetworks {
    cidrs: Vec<IpNet>,
    /// Quando `AGENT_DEVICE_CIDRS` foi informado com valor inválido, falha fechado
    /// (recusa tudo) em vez de voltar para "qualquer rede privada".
    deny_all: bool,
}

impl DeviceNetworks {
    #[must_use]
    pub const fn new(cidrs: Vec<IpNet>) -> Self {
        Self {
            cidrs,
            deny_all: false,
        }
    }

    /// Lê `AGENT_DEVICE_CIDRS`; faixa inválida falha fechado (bloqueia tudo e loga erro).
    #[must_use]
    pub fn from_env() -> Self {
        Self::parse(&std::env::var(DEVICE_CIDRS_ENV).unwrap_or_default())
    }

    /// Interpreta a lista separada por vírgula de forma estrita.
    ///
    /// # Errors
    ///
    /// Qualquer entrada não vazia que não seja um CIDR válido.
    pub fn try_parse(raw: &str) -> Result<Self, String> {
        let mut cidrs = Vec::new();
        for item in raw
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
        {
            let net: IpNet = item
                .parse()
                .map_err(|_| format!("CIDR inválido em {DEVICE_CIDRS_ENV}: '{item}'"))?;
            cidrs.push(net);
        }
        Ok(Self {
            cidrs,
            deny_all: false,
        })
    }

    /// Como [`Self::try_parse`], mas falha fechado (`deny_all`) se houver entrada
    /// inválida — um erro de digitação nunca amplia acesso para toda a rede privada.
    #[must_use]
    pub fn parse(raw: &str) -> Self {
        match Self::try_parse(raw) {
            Ok(networks) => networks,
            Err(error) => {
                tracing::error!(
                    %error,
                    "AGENT_DEVICE_CIDRS inválido; bloqueando acesso a equipamentos por segurança"
                );
                Self {
                    cidrs: Vec::new(),
                    deny_all: true,
                }
            }
        }
    }

    /// # Errors
    ///
    /// `Forbidden` quando `ip` está fora das redes que este host aceita.
    pub fn check(&self, ip: IpAddr) -> Result<(), RemoteError> {
        if self.deny_all {
            return Err(RemoteError::new(
                ErrorCode::Forbidden,
                format!(
                    "O agente não acessa {ip}: {DEVICE_CIDRS_ENV} contém faixa inválida neste host"
                ),
            ));
        }
        if target_allowed(ip, &self.cidrs) {
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
        let networks = DeviceNetworks::parse("192.168.10.0/24, 10.20.0.0/16");
        assert!(networks.check("192.168.10.7".parse().unwrap()).is_ok());
        assert!(networks.check("10.20.3.4".parse().unwrap()).is_ok());
        assert!(networks.check("192.168.11.7".parse().unwrap()).is_err());
        assert!(networks.check("127.0.0.1".parse().unwrap()).is_err());
    }

    #[test]
    fn cidr_invalido_falha_fechado_em_vez_de_liberar_tudo() {
        assert!(DeviceNetworks::try_parse("192.168.10.0/240").is_err());
        let typo = DeviceNetworks::parse("192.168.10.0/240");
        assert!(typo.check("192.168.10.7".parse().unwrap()).is_err());
        assert!(typo.check("127.0.0.1".parse().unwrap()).is_err());

        let partial_bad = DeviceNetworks::parse("192.168.10.0/24, lixo");
        assert!(partial_bad.check("192.168.10.7".parse().unwrap()).is_err());
    }
}
