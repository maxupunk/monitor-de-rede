//! Cálculo IPv4/CIDR do IPAM da VPN (§8.10.1).
//!
//! Reexporta e delega para o módulo unificado [`crate::services::shared::cidr`].

use std::net::Ipv4Addr;

use crate::services::shared::{
    cidr::{self, Ipv4Cidr},
    errors::AppResult,
};

pub type CidrRange = Ipv4Cidr;

/// Faz o parsing da faixa CIDR da VPN.
///
/// # Errors
///
/// Falha quando o endereço ou o prefixo não são IPv4/CIDR válidos.
pub fn parse_cidr(cidr: &str) -> AppResult<CidrRange> {
    cidr::parse_vpn_cidr(cidr)
}

/// Primeiro endereço utilizável da faixa — por convenção, o servidor da VPN.
pub fn first_usable_address(cidr: &str) -> AppResult<Ipv4Addr> {
    cidr::first_usable_address(cidr)
}

/// Verifica pertinência à faixa.
pub fn is_ip_in_cidr(ip: Ipv4Addr, cidr: &str) -> AppResult<bool> {
    cidr::is_ip_in_cidr(ip, cidr)
}

/// Itera os endereços utilizáveis da faixa (exclui rede e broadcast).
pub fn iterate_usable_addresses(cidr: &str) -> AppResult<impl Iterator<Item = Ipv4Addr>> {
    cidr::iterate_usable_addresses(cidr)
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
        let range = parse_cidr("10.8.0.37/24").unwrap();
        assert_eq!(range.network_address, ip("10.8.0.0"));
    }

    #[test]
    fn prefixos_pequenos_e_extremos() {
        assert_eq!(parse_cidr("10.8.0.0/30").unwrap().usable_hosts, 2);
        assert_eq!(parse_cidr("10.8.0.0/31").unwrap().usable_hosts, 0);
        assert_eq!(parse_cidr("10.8.0.5/32").unwrap().usable_hosts, 0);
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
