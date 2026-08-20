//! Cálculo e parsing unificado de faixas IPv4 e IPv6 / CIDR.
//!
//! Centraliza as operações de rede compartilhadas entre a varredura de
//! descoberta ([`crate::services::discovery`]), o IPAM da VPN
//! ([`crate::services::vpn`]) e os endpoints HTTP de redes e dispositivos.
//!
//! Utiliza estritamente os tipos de endereço da biblioteca padrão ([`Ipv4Addr`],
//! [`Ipv6Addr`], [`IpAddr`]) e fornece iteradores eficientes sem alocação
//! desnecessária em memória.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use crate::services::shared::errors::{AppError, AppResult};

/// Tamanho máximo padrão de cada lote de varredura.
pub const MAX_SCAN_HOSTS: u32 = 1024;

/// Representação unificada de uma faixa CIDR IPv4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ipv4Cidr {
    /// Endereço de rede normalizado (ex.: `10.8.0.0` ou `192.168.1.0`).
    pub network_address: Ipv4Addr,
    /// Endereço de broadcast (ex.: `10.8.0.255`).
    pub broadcast_address: Ipv4Addr,
    /// Comprimento do prefixo em bits (0..=32).
    pub prefix_length: u8,
    /// Máscara de rede em notação decimal (ex.: `255.255.255.0`).
    pub netmask: Ipv4Addr,
    /// Quantidade de endereços utilizáveis por hosts (exclui rede e broadcast).
    pub usable_hosts: u32,
}

impl Ipv4Cidr {
    /// Cria e normaliza uma faixa a partir de um endereço base e prefixo.
    #[must_use]
    pub fn new(address: Ipv4Addr, prefix_length: u8) -> Self {
        let prefix_length = prefix_length.min(32);
        let mask: u32 = if prefix_length == 0 {
            0
        } else {
            u32::MAX << (32 - prefix_length)
        };
        let network = u32::from(address) & mask;
        let broadcast = network | !mask;
        let total = 2_u32.checked_pow(u32::from(32 - prefix_length));

        Self {
            network_address: Ipv4Addr::from(network),
            broadcast_address: Ipv4Addr::from(broadcast),
            prefix_length,
            netmask: Ipv4Addr::from(mask),
            usable_hosts: total.map_or(u32::MAX - 1, |total| total.saturating_sub(2)),
        }
    }

    /// Primeiro endereço utilizável da faixa (por convenção, o servidor/gateway).
    #[must_use]
    pub fn first_usable_address(&self) -> Ipv4Addr {
        Ipv4Addr::from(u32::from(self.network_address).saturating_add(1))
    }

    /// Verifica se um IP pertence a esta faixa.
    #[must_use]
    pub fn contains(&self, ip: Ipv4Addr) -> bool {
        let target = u32::from(ip);
        target >= u32::from(self.network_address) && target <= u32::from(self.broadcast_address)
    }

    /// Itera os endereços utilizáveis da faixa (exclui rede e broadcast).
    pub fn usable_addresses(&self) -> impl Iterator<Item = Ipv4Addr> {
        let start = u32::from(self.network_address).saturating_add(1);
        let end = u32::from(self.broadcast_address).saturating_sub(1);
        (start..=end).map(Ipv4Addr::from)
    }
}

/// Representação unificada de uma faixa CIDR IPv6.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ipv6Cidr {
    pub network_address: Ipv6Addr,
    pub prefix_length: u8,
    pub usable_hosts: u32,
}

impl Ipv6Cidr {
    /// Cria e normaliza uma faixa IPv6.
    #[must_use]
    pub fn new(address: Ipv6Addr, prefix_length: u8) -> Self {
        let prefix_length = prefix_length.min(128);
        let host_bits = 128 - u32::from(prefix_length);
        let size = 1_u128 << host_bits;
        let mask = if prefix_length == 128 {
            u128::MAX
        } else {
            u128::MAX << host_bits
        };
        let network = u128::from(address) & mask;

        Self {
            network_address: Ipv6Addr::from(network),
            prefix_length,
            usable_hosts: u32::try_from(size).unwrap_or(u32::MAX),
        }
    }

    /// Expande um lote de endereços IPv6 a partir de um offset.
    #[must_use]
    pub fn expand_batch(&self, offset: u32, limit: usize) -> Vec<IpAddr> {
        let limit = limit.min(MAX_SCAN_HOSTS as usize);
        let base = u128::from(self.network_address);
        (0..self.usable_hosts)
            .skip(offset as usize)
            .take(limit)
            .map(|index| IpAddr::V6(Ipv6Addr::from(base + u128::from(index))))
            .collect()
    }
}

/// Estrutura para compatibilidade com o subsistema de descoberta e APIs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryCidrRange {
    /// Endereço de rede normalizado, ex.: `192.168.1.0` ou `fd00::4`.
    pub network_address: String,
    pub prefix: u8,
    /// Total de endereços utilizáveis na faixa.
    pub usable_hosts: u32,
    /// Flag de truncamento histórico (sempre falso nas versões modernas).
    pub truncated: bool,
}

// --- Funções de Parsing de Alto Nível ---

fn invalid_discovery(cidr: &str, reason: &str) -> AppError {
    AppError::validation(format!("Faixa CIDR inválida \"{cidr}\": {reason}"))
}

fn invalid_vpn(cidr: &str) -> AppError {
    AppError::validation(format!("CIDR inválido: {cidr}"))
}

/// Faz o parsing de um CIDR IPv4 estrito (ex.: `10.8.0.0/24`) utilizado pelo IPAM da VPN.
///
/// # Errors
///
/// Retorna [`AppError::Validation`] se a string não contiver uma barra ou se o IP/prefixo forem inválidos.
pub fn parse_vpn_cidr(cidr: &str) -> AppResult<Ipv4Cidr> {
    let (address, prefix) = cidr.split_once('/').ok_or_else(|| invalid_vpn(cidr))?;
    let address: Ipv4Addr = address.trim().parse().map_err(|_| invalid_vpn(cidr))?;
    let prefix_length: u8 = prefix.trim().parse().map_err(|_| invalid_vpn(cidr))?;
    if prefix_length > 32 {
        return Err(invalid_vpn(cidr));
    }
    Ok(Ipv4Cidr::new(address, prefix_length))
}

/// Primeiro endereço utilizável da faixa para a VPN.
pub fn first_usable_address(cidr: &str) -> AppResult<Ipv4Addr> {
    let range = parse_vpn_cidr(cidr)?;
    Ok(range.first_usable_address())
}

/// Verifica pertinência de um endereço à faixa da VPN.
pub fn is_ip_in_cidr(ip: Ipv4Addr, cidr: &str) -> AppResult<bool> {
    let range = parse_vpn_cidr(cidr)?;
    Ok(range.contains(ip))
}

/// Itera os endereços utilizáveis da faixa da VPN.
pub fn iterate_usable_addresses(cidr: &str) -> AppResult<impl Iterator<Item = Ipv4Addr>> {
    let range = parse_vpn_cidr(cidr)?;
    Ok(range.usable_addresses())
}

/// Interpreta e valida uma faixa CIDR para descoberta (suporta IPv4 /8–/32, host único sem barra, e IPv6 /112–/128).
pub fn parse_discovery_cidr(cidr: &str) -> Result<DiscoveryCidrRange, AppError> {
    let value = cidr.trim();
    if value.is_empty() {
        return Err(invalid_discovery(cidr, "valor vazio"));
    }

    let (address_part, prefix_part) = match value.split_once('/') {
        Some((address, prefix)) => (address.trim(), Some(prefix.trim())),
        None => (value, None),
    };

    if address_part.contains(':') {
        let address: Ipv6Addr = address_part
            .parse()
            .map_err(|_| invalid_discovery(cidr, "endereço IP malformado"))?;
        let prefix = match prefix_part {
            None => 128,
            Some(text) => text.parse::<u8>().map_err(|_| {
                invalid_discovery(cidr, "prefixo IPv6 deve estar entre /112 e /128")
            })?,
        };
        if !(112..=128).contains(&prefix) {
            return Err(invalid_discovery(
                cidr,
                "prefixo IPv6 deve estar entre /112 e /128",
            ));
        }
        let ipv6_cidr = Ipv6Cidr::new(address, prefix);
        return Ok(DiscoveryCidrRange {
            network_address: ipv6_cidr.network_address.to_string(),
            prefix: ipv6_cidr.prefix_length,
            usable_hosts: ipv6_cidr.usable_hosts,
            truncated: false,
        });
    }

    let address: Ipv4Addr = address_part
        .parse()
        .map_err(|_| invalid_discovery(cidr, "endereço IP malformado"))?;

    let prefix: u8 = match prefix_part {
        None => 32,
        Some(text) => text
            .parse()
            .map_err(|_| invalid_discovery(cidr, "prefixo deve estar entre /8 e /32"))?,
    };
    if !(8..=32).contains(&prefix) {
        return Err(invalid_discovery(cidr, "prefixo deve estar entre /8 e /32"));
    }

    let size = 1u64 << (32 - u32::from(prefix));
    let mask = if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    };
    let network = u32::from(address) & mask;
    let usable_hosts = if prefix >= 31 {
        size
    } else {
        size.saturating_sub(2)
    };
    let usable_hosts = u32::try_from(usable_hosts).unwrap_or(u32::MAX);

    Ok(DiscoveryCidrRange {
        network_address: Ipv4Addr::from(network).to_string(),
        prefix,
        usable_hosts,
        truncated: false,
    })
}

/// Verifica se uma string de CIDR é válida para varredura de descoberta.
#[must_use]
pub fn is_scannable_cidr(cidr: &str) -> bool {
    parse_discovery_cidr(cidr).is_ok()
}

/// Expande os endereços utilizáveis de uma faixa CIDR.
pub fn expand_cidr(cidr: &str, max_hosts: usize) -> Result<Vec<IpAddr>, AppError> {
    expand_cidr_batch(cidr, 0, max_hosts)
}

/// Expande um lote de endereços utilizáveis a partir de um deslocamento.
pub fn expand_cidr_batch(
    cidr: &str,
    offset: u32,
    max_hosts: usize,
) -> Result<Vec<IpAddr>, AppError> {
    let value = cidr.trim();
    if value.contains(':') {
        let range = parse_discovery_cidr(cidr)?;
        let addr: Ipv6Addr = range
            .network_address
            .parse()
            .map_err(|_| invalid_discovery(cidr, "endereço IP malformado"))?;
        let ipv6 = Ipv6Cidr::new(addr, range.prefix);
        return Ok(ipv6.expand_batch(offset, max_hosts));
    }

    let range = parse_discovery_cidr(cidr)?;
    let base: Ipv4Addr = range
        .network_address
        .parse()
        .map_err(|_| invalid_discovery(cidr, "endereço IP malformado"))?;
    let base_num = u32::from(base);
    let size = 1u64 << (32 - u32::from(range.prefix));
    let (first, last) = if range.prefix >= 31 {
        (u64::from(base_num), u64::from(base_num) + size - 1)
    } else {
        (u64::from(base_num) + 1, u64::from(base_num) + size - 2)
    };

    let limit = max_hosts.min(MAX_SCAN_HOSTS as usize);
    Ok((first..=last)
        .skip(offset as usize)
        .take(limit)
        .map(|address| IpAddr::V4(Ipv4Addr::from(address as u32)))
        .collect())
}
