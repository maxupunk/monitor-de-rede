//! Cálculo IPv4/CIDR do IPAM da VPN (§8.10.1).
//!
//! Existe separado de [`crate::services::discovery::cidr_range`] porque as duas
//! perguntas são diferentes: o discovery quer saber *quantos hosts varrer* e
//! trunca faixas grandes; aqui interessa *qual endereço entregar ao próximo
//! peer*, sem teto e sem truncamento. Sem I/O e sem estado — puramente
//! funcional e testável.

use std::net::Ipv4Addr;

use crate::services::shared::errors::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CidrRange {
    /// Endereço de rede (ex.: `10.8.0.0`).
    pub network_address: Ipv4Addr,
    /// Endereço de broadcast (ex.: `10.8.0.255`).
    pub broadcast_address: Ipv4Addr,
    /// Máscara em bits (ex.: 24).
    pub prefix_length: u8,
    /// Máscara em notação decimal (ex.: `255.255.255.0`).
    pub netmask: Ipv4Addr,
    /// Quantidade de endereços utilizáveis por hosts.
    pub usable_hosts: u32,
}

fn invalid(cidr: &str) -> AppError {
    AppError::validation(format!("CIDR inválido: {cidr}"))
}

/// # Errors
///
/// Falha quando o endereço ou o prefixo não são IPv4/CIDR válidos.
pub fn parse_cidr(cidr: &str) -> AppResult<CidrRange> {
    let (address, prefix) = cidr.split_once('/').ok_or_else(|| invalid(cidr))?;
    let address: Ipv4Addr = address.parse().map_err(|_| invalid(cidr))?;
    let prefix_length: u8 = prefix.parse().map_err(|_| invalid(cidr))?;
    if prefix_length > 32 {
        return Err(invalid(cidr));
    }

    let mask: u32 = if prefix_length == 0 {
        0
    } else {
        u32::MAX << (32 - prefix_length)
    };
    let network = u32::from(address) & mask;
    let broadcast = network | !mask;
    // `/31` e `/32` não têm endereços utilizáveis pela conta clássica; o
    // `saturating_sub` evita o estouro que o `Math.max(n-2, 0)` do original
    // escondia atrás da aritmética de ponto flutuante.
    let total = 2_u32.checked_pow(u32::from(32 - prefix_length));

    Ok(CidrRange {
        network_address: Ipv4Addr::from(network),
        broadcast_address: Ipv4Addr::from(broadcast),
        prefix_length,
        netmask: Ipv4Addr::from(mask),
        usable_hosts: total.map_or(u32::MAX - 1, |total| total.saturating_sub(2)),
    })
}

/// Primeiro endereço utilizável da faixa — por convenção, o servidor da VPN.
///
/// # Errors
///
/// Propaga a validação de [`parse_cidr`].
pub fn first_usable_address(cidr: &str) -> AppResult<Ipv4Addr> {
    let range = parse_cidr(cidr)?;
    Ok(Ipv4Addr::from(u32::from(range.network_address) + 1))
}

/// # Errors
///
/// Propaga a validação de [`parse_cidr`].
pub fn is_ip_in_cidr(ip: Ipv4Addr, cidr: &str) -> AppResult<bool> {
    let range = parse_cidr(cidr)?;
    let target = u32::from(ip);
    Ok(target >= u32::from(range.network_address) && target <= u32::from(range.broadcast_address))
}

/// Itera os endereços utilizáveis da faixa (exclui rede e broadcast).
///
/// Devolve um iterador preguiçoso: uma faixa `/8` tem 16 milhões de endereços e
/// materializá-los para achar o primeiro livre seria absurdo.
///
/// # Errors
///
/// Propaga a validação de [`parse_cidr`].
pub fn iterate_usable_addresses(cidr: &str) -> AppResult<impl Iterator<Item = Ipv4Addr>> {
    let range = parse_cidr(cidr)?;
    let start = u32::from(range.network_address).saturating_add(1);
    let end = u32::from(range.broadcast_address).saturating_sub(1);
    Ok((start..=end).map(Ipv4Addr::from))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(text: &str) -> Ipv4Addr {
        text.parse().expect("IPv4 do teste")
    }

    #[test]
    fn calcula_a_faixa_padrao_da_vpn() {
        let range = parse_cidr("10.8.0.0/24").unwrap();
        assert_eq!(range.network_address, ip("10.8.0.0"));
        assert_eq!(range.broadcast_address, ip("10.8.0.255"));
        assert_eq!(range.netmask, ip("255.255.255.0"));
        assert_eq!(range.usable_hosts, 254);
        assert_eq!(first_usable_address("10.8.0.0/24").unwrap(), ip("10.8.0.1"));
    }

    #[test]
    fn normaliza_endereco_que_nao_e_o_da_rede() {
        // 10.8.0.37/24 descreve a mesma faixa que 10.8.0.0/24.
        let range = parse_cidr("10.8.0.37/24").unwrap();
        assert_eq!(range.network_address, ip("10.8.0.0"));
    }

    #[test]
    fn prefixos_pequenos_e_extremos() {
        assert_eq!(parse_cidr("10.8.0.0/30").unwrap().usable_hosts, 2);
        assert_eq!(parse_cidr("10.8.0.0/31").unwrap().usable_hosts, 0);
        assert_eq!(parse_cidr("10.8.0.5/32").unwrap().usable_hosts, 0);
        // `/0` estouraria `2^32` num u32 — o original em JS usava float.
        assert_eq!(parse_cidr("0.0.0.0/0").unwrap().usable_hosts, u32::MAX - 1);
    }

    #[test]
    fn cidr_invalido_e_erro_e_nao_panico() {
        for candidato in [
            "",
            "10.8.0.0",
            "10.8.0.0/33",
            "10.8.0.0/abc",
            "999.1.1.1/24",
        ] {
            assert!(parse_cidr(candidato).is_err(), "aceitou {candidato:?}");
        }
    }

    #[test]
    fn pertinencia_respeita_os_limites_da_faixa() {
        assert!(is_ip_in_cidr(ip("10.8.0.1"), "10.8.0.0/24").unwrap());
        assert!(is_ip_in_cidr(ip("10.8.0.255"), "10.8.0.0/24").unwrap());
        assert!(!is_ip_in_cidr(ip("10.8.1.1"), "10.8.0.0/24").unwrap());
    }

    #[test]
    fn a_iteracao_exclui_rede_e_broadcast() {
        let enderecos: Vec<_> = iterate_usable_addresses("10.8.0.0/29").unwrap().collect();
        assert_eq!(enderecos.len(), 6);
        assert_eq!(enderecos.first(), Some(&ip("10.8.0.1")));
        assert_eq!(enderecos.last(), Some(&ip("10.8.0.6")));
    }

    #[test]
    fn faixa_sem_hosts_nao_itera() {
        assert_eq!(iterate_usable_addresses("10.8.0.0/31").unwrap().count(), 0);
        assert_eq!(iterate_usable_addresses("10.8.0.5/32").unwrap().count(), 0);
    }
}
