//! Expansão de faixas CIDR para a varredura de descoberta.
//!
//! Reexporta e delega para o módulo unificado [`crate::services::shared::cidr`].

use std::net::IpAddr;

use crate::services::shared::{
    cidr::{self, DiscoveryCidrRange},
    errors::AppError,
};

/// Tamanho de cada lote. A execução percorre todos os lotes do CIDR.
pub const MAX_SCAN_HOSTS: u32 = cidr::MAX_SCAN_HOSTS;

pub type CidrRange = DiscoveryCidrRange;

/// Interpreta e valida uma faixa. Aceita host único (sem `/`) e prefixos de
/// /8 a /32 ou IPv6 de /112 a /128.
pub fn parse_cidr_range(cidr: &str) -> Result<CidrRange, AppError> {
    cidr::parse_discovery_cidr(cidr)
}

/// `true` se o CIDR é utilizável numa varredura.
#[must_use]
pub fn is_scannable_cidr(cidr: &str) -> bool {
    cidr::is_scannable_cidr(cidr)
}

/// Expande somente endereços utilizáveis e limita a memória/tempo da operação.
pub fn expand_cidr(cidr: &str, max_hosts: usize) -> Result<Vec<IpAddr>, AppError> {
    cidr::expand_cidr(cidr, max_hosts)
}

/// Expande um lote do CIDR sem materializar a faixa inteira em memória.
pub fn expand_cidr_batch(
    cidr: &str,
    offset: u32,
    max_hosts: usize,
) -> Result<Vec<IpAddr>, AppError> {
    cidr::expand_cidr_batch(cidr, offset, max_hosts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normaliza_o_endereco_de_rede() {
        let range = parse_cidr_range("192.168.1.77/24").unwrap();
        assert_eq!(range.network_address, "192.168.1.0");
        assert_eq!(range.prefix, 24);
        assert_eq!(range.usable_hosts, 254);
        assert!(!range.truncated);
    }

    #[test]
    fn host_avulso_vira_prefixo_32() {
        let range = parse_cidr_range("10.0.0.9").unwrap();
        assert_eq!(range.prefix, 32);
        assert_eq!(range.usable_hosts, 1);
        assert_eq!(range.network_address, "10.0.0.9");
    }

    #[test]
    fn rfc_3021_nao_reserva_rede_e_broadcast() {
        assert_eq!(parse_cidr_range("10.0.0.0/31").unwrap().usable_hosts, 2);
        assert_eq!(parse_cidr_range("10.0.0.0/32").unwrap().usable_hosts, 1);
        assert_eq!(parse_cidr_range("10.0.0.0/30").unwrap().usable_hosts, 2);
    }

    #[test]
    fn faixas_grandes_nao_sao_mais_truncadas() {
        assert!(!parse_cidr_range("10.0.0.0/22").unwrap().truncated);
        assert!(!parse_cidr_range("10.0.0.0/21").unwrap().truncated);
    }

    #[test]
    fn primeiro_octeto_acima_de_127_nao_vira_negativo() {
        let range = parse_cidr_range("200.160.0.0/24").unwrap();
        assert_eq!(range.network_address, "200.160.0.0");
    }

    #[test]
    fn recusa_entrada_malformada_com_a_mensagem_do_backend_atual() {
        for (cidr, trecho) in [
            ("", "valor vazio"),
            ("   ", "valor vazio"),
            ("192.168.1", "endereço IP malformado"),
            ("192.168.1.256/24", "endereço IP malformado"),
            ("192.168.1.1.1/24", "endereço IP malformado"),
            ("abc/24", "endereço IP malformado"),
            ("10.0.0.0/7", "prefixo deve estar entre /8 e /32"),
            ("10.0.0.0/33", "prefixo deve estar entre /8 e /32"),
            ("10.0.0.0/abc", "prefixo deve estar entre /8 e /32"),
        ] {
            let err = parse_cidr_range(cidr).unwrap_err();
            let msg = err.to_string();
            assert!(
                msg.contains(trecho),
                "`{cidr}` devia falhar com `{trecho}`, veio `{msg}`"
            );
            assert!(!is_scannable_cidr(cidr));
        }
    }

    #[test]
    fn faixas_validas_sao_varreduraveis() {
        for cidr in ["192.168.0.0/24", "10.0.0.0/8", "172.16.5.9", "10.0.0.0/32"] {
            assert!(is_scannable_cidr(cidr), "`{cidr}` devia ser varredurável");
        }
    }

    #[test]
    fn expansao_respeita_rfc_3021_e_limite() {
        assert_eq!(
            expand_cidr("10.0.0.0/31", 1024).unwrap(),
            vec![
                "10.0.0.0".parse::<IpAddr>().unwrap(),
                "10.0.0.1".parse::<IpAddr>().unwrap()
            ]
        );
        assert_eq!(expand_cidr("192.168.1.0/24", 2).unwrap().len(), 2);
    }

    #[test]
    fn ipv6_e_normalizado_e_expandido_em_lotes() {
        let range = parse_cidr_range("fd00::7/126").unwrap();
        assert_eq!(range.network_address, "fd00::4");
        assert_eq!(range.usable_hosts, 4);
        assert_eq!(
            expand_cidr_batch("fd00::7/126", 2, 2).unwrap(),
            vec![
                "fd00::6".parse::<IpAddr>().unwrap(),
                "fd00::7".parse().unwrap()
            ]
        );
        assert!(parse_cidr_range("fd00::/64").is_err());
    }
}
