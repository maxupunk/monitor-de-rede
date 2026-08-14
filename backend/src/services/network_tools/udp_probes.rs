//! Payloads UDP mínimos para aumentar a chance de uma resposta útil.
//!
//! Silêncio em UDP não prova que a porta está fechada; por isso o scanner só
//! classifica como `closed` quando o sistema devolve `ECONNREFUSED`.

/// Retorna um payload conhecido para portas comuns, ou um byte neutro.
#[must_use]
pub fn probe_for(port: u16) -> Vec<u8> {
    match port {
        // DNS A/IN para a raiz.
        53 => vec![
            0x00, 0x01, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x01, 0x00, 0x01,
        ],
        // NTP v3 client packet.
        123 => {
            let mut packet = vec![0; 48];
            packet[0] = 0x1b;
            packet
        }
        // NetBIOS node-status wildcard query.
        137 => vec![
            0x80, 0x94, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20,
            0x43, 0x4b, 0x41, 0x41, 0x41, 0x41, 0x41, 0x41, 0x41, 0x41, 0x41, 0x41, 0x41,
            0x41, 0x41, 0x41, 0x41, 0x41, 0x41, 0x41, 0x41, 0x41, 0x41, 0x41, 0x41, 0x41,
            0x41, 0x41, 0x41, 0x41, 0x00, 0x00, 0x21, 0x00, 0x01,
        ],
        // SNMPv2c GetRequest para sysDescr.0, comunidade public.
        161 => vec![
            0x30, 0x26, 0x02, 0x01, 0x01, 0x04, 0x06, 0x70, 0x75, 0x62, 0x6c, 0x69, 0x63,
            0xa0, 0x19, 0x02, 0x04, 0x00, 0x00, 0x00, 0x01, 0x02, 0x01, 0x00, 0x02, 0x01,
            0x00, 0x30, 0x0b, 0x30, 0x09, 0x06, 0x05, 0x2b, 0x06, 0x01, 0x02, 0x01, 0x05,
            0x00,
        ],
        1900 => b"M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: 1\r\nST: ssdp:all\r\n\r\n".to_vec(),
        5353 => vec![
            0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 9, b'_', b's', b'e', b'r', b'v', b'i', b'c',
            b'e', b's', 7, b'_', b'd', b'n', b's', b'-', b's', b'd', 4, b'_', b'u', b'd', b'p',
            5, b'l', b'o', b'c', b'a', b'l', 0, 0, 12, 0, 1,
        ],
        _ => vec![0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mantem_payload_ntp_de_48_bytes() {
        assert_eq!(probe_for(123).len(), 48);
    }
    #[test]
    fn usa_fallback_neutro() {
        assert_eq!(probe_for(9999), vec![0]);
    }
}
