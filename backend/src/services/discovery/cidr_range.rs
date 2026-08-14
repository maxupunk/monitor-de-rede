//! Expansão de faixas CIDR IPv4 para a varredura de descoberta.
//!
//! Vive fora dos scanners porque três lugares precisam da mesma resposta: o
//! scanner ICMP (quais IPs pingar), o endpoint que dispara a varredura de uma
//! rede (o CIDR cadastrado é utilizável?) e a UI (quantos hosts serão varridos).
//!
//! A Fase 1 usa daqui só o que o `Network.scannable`/`usableHosts` do §6.1
//! precisa. O `expand_cidr` (lista de endereços, com truncamento em
//! [`MAX_SCAN_HOSTS`]) entra na Fase 5, junto com os scanners.

use std::net::Ipv4Addr;

use crate::services::shared::errors::AppError;

/// Teto de endereços varridos por execução — /22 completo.
pub const MAX_SCAN_HOSTS: u32 = 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CidrRange {
    /// Endereço de rede normalizado, ex.: `192.168.1.0`
    pub network_address: String,
    pub prefix: u8,
    /// Total de endereços utilizáveis na faixa, antes de qualquer truncamento
    pub usable_hosts: u32,
    /// `true` quando `usable_hosts` excede [`MAX_SCAN_HOSTS`]
    pub truncated: bool,
}

fn invalid(cidr: &str, reason: &str) -> AppError {
    // Texto voltado ao operador, não ao log: esta mensagem chega à tela no 422
    // de `POST /api/networks/:id/scan`. Mudá-la muda o que o usuário lê.
    AppError::validation(format!("Faixa CIDR inválida \"{cidr}\": {reason}"))
}

fn to_number(ip: &str) -> Option<u32> {
    let mut value: u32 = 0;
    let mut octets = 0;

    for part in ip.split('.') {
        // A contagem é verificada **dentro** do laço: `1.2.3.4.5` acumularia um
        // quinto octeto e estouraria o u32 antes de a validação de quantidade
        // rodar. (O original em JS usava float e passava batido.)
        octets += 1;
        if octets > 4 {
            return None;
        }
        if part.is_empty() || part.len() > 3 || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let octet: u32 = part.parse().ok()?;
        if octet > 255 {
            return None;
        }
        value = value * 256 + octet;
    }

    (octets == 4).then_some(value)
}

fn to_address(value: u32) -> String {
    let [a, b, c, d] = value.to_be_bytes();
    format!("{a}.{b}.{c}.{d}")
}

/// Interpreta e valida uma faixa. Aceita host único (sem `/`) e prefixos de
/// /8 a /32 — abaixo de /8 a varredura deixa de fazer sentido para o alvo do
/// produto (redes residenciais e de pequenas empresas).
///
/// # Errors
///
/// [`AppError::Validation`] com a mensagem do backend atual quando o valor é
/// vazio, o endereço é malformado ou o prefixo está fora de /8–/32.
pub fn parse_cidr_range(cidr: &str) -> Result<CidrRange, AppError> {
    let value = cidr.trim();
    if value.is_empty() {
        return Err(invalid(cidr, "valor vazio"));
    }

    let (address, prefix_part) = match value.split_once('/') {
        Some((address, prefix)) => (address, Some(prefix)),
        None => (value, None),
    };

    let base =
        to_number(address.trim()).ok_or_else(|| invalid(cidr, "endereço IPv4 malformado"))?;

    // Host avulso: uma varredura de um endereço só é legítima (testar um alvo).
    let prefix: u8 = match prefix_part {
        None => 32,
        Some(text) => text
            .trim()
            .parse()
            .map_err(|_| invalid(cidr, "prefixo deve estar entre /8 e /32"))?,
    };
    if !(8..=32).contains(&prefix) {
        return Err(invalid(cidr, "prefixo deve estar entre /8 e /32"));
    }

    // `u64` no cálculo: um /8 tem 2^24 endereços, mas o `size` de um /0 (que a
    // validação já barrou) estouraria o u32. Manter em 64 bits deixa a conta
    // óbvia em vez de depender da validação anterior.
    let size = 1u64 << (32 - u32::from(prefix));
    let network_number = (u64::from(base) / size) * size;

    // /31 e /32 não têm endereço de rede nem de broadcast reservados (RFC 3021)
    let usable_hosts = if prefix >= 31 { size } else { size - 2 };
    let usable_hosts = u32::try_from(usable_hosts).unwrap_or(u32::MAX);

    Ok(CidrRange {
        network_address: to_address(u32::try_from(network_number).unwrap_or(0)),
        prefix,
        usable_hosts,
        truncated: usable_hosts > MAX_SCAN_HOSTS,
    })
}

/// `true` se o CIDR é utilizável numa varredura.
#[must_use]
pub fn is_scannable_cidr(cidr: &str) -> bool {
    parse_cidr_range(cidr).is_ok()
}

/// Expande somente endereços utilizáveis e limita a memória/tempo da operação.
/// Em /31 e /32 todos os endereços pertencem ao enlace conforme RFC 3021.
pub fn expand_cidr(cidr: &str, max_hosts: usize) -> Result<Vec<Ipv4Addr>, AppError> {
    let range = parse_cidr_range(cidr)?;
    let base = to_number(&range.network_address).expect("rede normalizada é IPv4 válido");
    let size = 1u64 << (32 - u32::from(range.prefix));
    let (first, last) = if range.prefix >= 31 {
        (base as u64, base as u64 + size - 1)
    } else {
        (base as u64 + 1, base as u64 + size - 2)
    };
    let limit = max_hosts.min(MAX_SCAN_HOSTS as usize);
    Ok((first..=last)
        .take(limit)
        .map(|address| Ipv4Addr::from(address as u32))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normaliza_o_endereco_de_rede() {
        // 192.168.1.77/24 pertence à rede 192.168.1.0.
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
        // /30 volta a reservar os dois.
        assert_eq!(parse_cidr_range("10.0.0.0/30").unwrap().usable_hosts, 2);
    }

    #[test]
    fn marca_truncamento_acima_do_teto() {
        // /22 = 1022 utilizáveis, dentro do teto; /21 = 2046, acima.
        assert!(!parse_cidr_range("10.0.0.0/22").unwrap().truncated);
        assert!(parse_cidr_range("10.0.0.0/21").unwrap().truncated);
    }

    #[test]
    fn primeiro_octeto_acima_de_127_nao_vira_negativo() {
        // O bug que o comentário do backend atual documenta: `<<` em 32 bits com
        // sinal produzia valor negativo aqui.
        let range = parse_cidr_range("200.160.0.0/24").unwrap();
        assert_eq!(range.network_address, "200.160.0.0");
    }

    #[test]
    fn recusa_entrada_malformada_com_a_mensagem_do_backend_atual() {
        for (cidr, trecho) in [
            ("", "valor vazio"),
            ("   ", "valor vazio"),
            ("192.168.1", "endereço IPv4 malformado"),
            ("192.168.1.256/24", "endereço IPv4 malformado"),
            ("192.168.1.1.1/24", "endereço IPv4 malformado"),
            ("abc/24", "endereço IPv4 malformado"),
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
                "10.0.0.0".parse::<Ipv4Addr>().unwrap(),
                "10.0.0.1".parse::<Ipv4Addr>().unwrap()
            ]
        );
        assert_eq!(expand_cidr("192.168.1.0/24", 2).unwrap().len(), 2);
    }
}
