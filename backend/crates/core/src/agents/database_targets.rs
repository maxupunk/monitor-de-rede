//! Destinos da ponte de banco (ADR 013): `AGENT_DATABASE_TARGETS` no host do
//! agente. O agente lê a lista e a central a escreve no comando de instalação
//! — o mesmo formato nas duas pontas.

use std::{
    fmt,
    net::{IpAddr, SocketAddr},
    str::FromStr,
};

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

impl fmt::Display for TunnelTarget {
    /// A forma canônica, que o `FromStr` lê de volta: um IP sozinho sai sem
    /// prefixo e o IPv6 entre colchetes.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let host = if self.net.prefix_len() == self.net.max_prefix_len() {
            self.net.addr().to_string()
        } else {
            self.net.to_string()
        };
        match self.net {
            ipnet::IpNet::V4(_) => write!(f, "{host}:{}", self.port),
            ipnet::IpNet::V6(_) => write!(f, "[{host}]:{}", self.port),
        }
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

/// A lista no formato da variável.
#[must_use]
pub fn format_targets(targets: &[TunnelTarget]) -> String {
    targets
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_forma_canonica_e_lida_de_volta() {
        let raw = "10.0.0.20:5432,172.16.0.0/12:3306,[fd00::5]:5432,[fd00::/8]:5432";
        let targets = parse_targets(raw).expect("destinos");
        assert_eq!(format_targets(&targets), raw);
        assert_eq!(parse_targets(&format_targets(&targets)).unwrap(), targets);
    }
}
