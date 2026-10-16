//! Formato de fio do MikroTik Bandwidth Test em TCP — o mesmo codec para o
//! servidor (central) e o cliente (agente).
//!
//! Não há especificação oficial. A referência é o `btest-opensource` de Alex
//! Samorukov (MIT, `btest.c` e `README.md`), reconstruído de capturas do
//! RouterOS 6:
//!
//! * o servidor abre com `01 00 00 00` ("hello");
//! * o cliente manda o comando de 16 bytes ([`Command`]);
//! * o servidor responde `01 00 00 00` (sem autenticação), `02 00 00 00` +
//!   16 bytes de desafio (MD5, RouterOS < 6.43) ou `03 00 00 00` (EC-SRP5,
//!   RouterOS ≥ 6.43 — não suportado);
//! * no MD5 o cliente devolve 16 bytes de hash + 32 de usuário, e o servidor
//!   responde `01 00 00 00` (ok) ou `00 00 00 00` (falhou);
//! * a cada ~1 s quem recebe manda uma mensagem de estado de 12 bytes
//!   ([`Status`]) com os bytes recebidos no intervalo.

use md5::{Digest, Md5};

use crate::services::shared::errors::{AppError, AppResult};

/// "Hello" do servidor e, também, a resposta "sem autenticação / ok".
pub(super) const HELLO: [u8; 4] = [0x01, 0x00, 0x00, 0x00];
/// Autenticação aceita (ou dispensada).
pub(super) const AUTH_OK: [u8; 4] = HELLO;
/// Autenticação recusada.
pub(super) const AUTH_FAILED: [u8; 4] = [0x00, 0x00, 0x00, 0x00];
/// Desafio MD5 (seguido de 16 bytes).
pub(super) const AUTH_MD5: [u8; 4] = [0x02, 0x00, 0x00, 0x00];
/// Autenticação EC-SRP5 (RouterOS ≥ 6.43).
pub(super) const AUTH_EC_SRP5: [u8; 4] = [0x03, 0x00, 0x00, 0x00];

pub(super) const COMMAND_LEN: usize = 16;
pub(super) const STATUS_LEN: usize = 12;
pub(super) const NONCE_LEN: usize = 16;
pub(super) const USER_LEN: usize = 32;
/// Resposta MD5 do cliente: hash + usuário.
pub(super) const AUTH_REPLY_LEN: usize = 16 + USER_LEN;

const PROTO_UDP: u8 = 0x00;
const PROTO_TCP: u8 = 0x01;
/// Bit de direção: **o cliente transmite** (o servidor recebe).
const DIR_CLIENT_TX: u8 = 0x01;
/// Bit de direção: **o cliente recebe** (o servidor transmite).
const DIR_CLIENT_RX: u8 = 0x02;
const STATUS_ID: u8 = 0x07;
/// Tamanho de bloco que o RouterOS usa em TCP (`00 80` = 32 768).
pub(super) const TCP_TX_SIZE: u16 = 0x8000;

/// Comando de início de teste, 16 bytes.
///
/// | byte | campo |
/// |---|---|
/// | 0 | protocolo: `01` TCP, `00` UDP |
/// | 1 | direção: `01` cliente transmite, `02` cliente recebe, `03` ambos |
/// | 2 | `01` dados zerados, `00` aleatórios |
/// | 3 | `tcp-connection-count` (`0` quando 1) |
/// | 4–5 | tamanho do bloco, u16 LE (`00 80` em TCP) |
/// | 6–7 | buffer do cliente, u16 LE |
/// | 8–11 | `remote-tx-speed` (bit/s que o **servidor** pode enviar), u32 LE, `0` = livre |
/// | 12–15 | `local-tx-speed` (bit/s que o **cliente** envia), u32 LE, `0` = livre |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Command {
    pub tcp: bool,
    pub client_tx: bool,
    pub client_rx: bool,
    pub zero_data: bool,
    pub tcp_connection_count: u8,
    pub tx_size: u16,
    pub client_buffer: u16,
    pub remote_tx_speed: u32,
    pub local_tx_speed: u32,
}

impl Command {
    /// Comando TCP de uma conexão, dados zerados, com o teto opcional nos dois
    /// sentidos (o `btest-opensource` faz o mesmo).
    pub(super) fn tcp(client_tx: bool, client_rx: bool, limit_bps: Option<u64>) -> Self {
        let speed = limit_bps.map_or(0, |bps| u32::try_from(bps).unwrap_or(u32::MAX));
        Self {
            tcp: true,
            client_tx,
            client_rx,
            zero_data: true,
            tcp_connection_count: 0,
            tx_size: TCP_TX_SIZE,
            client_buffer: 0,
            remote_tx_speed: speed,
            local_tx_speed: speed,
        }
    }

    pub(super) fn encode(&self) -> [u8; COMMAND_LEN] {
        let mut buf = [0u8; COMMAND_LEN];
        buf[0] = if self.tcp { PROTO_TCP } else { PROTO_UDP };
        buf[1] = (if self.client_tx { DIR_CLIENT_TX } else { 0 })
            | (if self.client_rx { DIR_CLIENT_RX } else { 0 });
        buf[2] = u8::from(self.zero_data);
        buf[3] = self.tcp_connection_count;
        buf[4..6].copy_from_slice(&self.tx_size.to_le_bytes());
        buf[6..8].copy_from_slice(&self.client_buffer.to_le_bytes());
        buf[8..12].copy_from_slice(&self.remote_tx_speed.to_le_bytes());
        buf[12..16].copy_from_slice(&self.local_tx_speed.to_le_bytes());
        buf
    }

    /// Lê o comando. Direção vazia ou protocolo desconhecido são recusados.
    pub(super) fn decode(buf: &[u8; COMMAND_LEN]) -> AppResult<Self> {
        let tcp = match buf[0] {
            PROTO_TCP => true,
            PROTO_UDP => false,
            other => return Err(protocol_error(&format!("protocolo {other:#04x}"))),
        };
        let direction = buf[1];
        if direction == 0 || direction & !(DIR_CLIENT_TX | DIR_CLIENT_RX) != 0 {
            return Err(protocol_error(&format!("direção {direction:#04x}")));
        }
        Ok(Self {
            tcp,
            client_tx: direction & DIR_CLIENT_TX != 0,
            client_rx: direction & DIR_CLIENT_RX != 0,
            zero_data: buf[2] != 0,
            tcp_connection_count: buf[3],
            tx_size: u16::from_le_bytes([buf[4], buf[5]]),
            client_buffer: u16::from_le_bytes([buf[6], buf[7]]),
            remote_tx_speed: u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]),
            local_tx_speed: u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]),
        })
    }
}

/// Mensagem de estado: `07`, sequência u32 **BE**, 3 bytes zerados, bytes
/// recebidos no intervalo u32 **LE** (assim no `btest-opensource`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Status {
    pub seq: u32,
    pub received: u32,
}

impl Status {
    pub(super) fn encode(&self) -> [u8; STATUS_LEN] {
        let mut buf = [0u8; STATUS_LEN];
        buf[0] = STATUS_ID;
        buf[1..5].copy_from_slice(&self.seq.to_be_bytes());
        buf[8..12].copy_from_slice(&self.received.to_le_bytes());
        buf
    }

    pub(super) fn decode(buf: &[u8; STATUS_LEN]) -> Option<Self> {
        (buf[0] == STATUS_ID).then(|| Self {
            seq: u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]),
            received: u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]),
        })
    }
}

/// Separa as mensagens de estado do fluxo de dados recebido.
///
/// Em TCP os dados e as mensagens de estado dividem a mesma conexão. Como
/// pedimos dados **zerados**, todo `07` fora de uma mensagem começa uma; os
/// demais bytes são dados. Com dados aleatórios (outro cliente) a separação
/// não é confiável, e por isso só serve para descontar o tamanho das mensagens
/// da contagem — a vazão nunca depende do que o par informa.
#[derive(Debug, Default)]
pub(super) struct StatusScanner {
    buf: [u8; STATUS_LEN],
    filled: usize,
    /// Mensagens completas encontradas.
    pub statuses: u64,
    /// Soma dos bytes que o par informou ter recebido.
    pub peer_received: u64,
}

impl StatusScanner {
    pub(super) fn feed(&mut self, data: &[u8]) {
        for &byte in data {
            if self.filled > 0 {
                self.buf[self.filled] = byte;
                self.filled += 1;
                if self.filled == STATUS_LEN {
                    self.filled = 0;
                    if let Some(status) = Status::decode(&self.buf) {
                        self.statuses += 1;
                        self.peer_received += u64::from(status.received);
                    }
                }
            } else if byte == STATUS_ID {
                self.buf[0] = byte;
                self.filled = 1;
            }
        }
    }

    /// Bytes de controle (mensagens de estado) vistos até aqui.
    pub(super) fn control_bytes(&self) -> u64 {
        self.statuses * STATUS_LEN as u64
    }
}

/// Hash da autenticação MD5: `md5(senha + md5(senha + desafio))`.
///
/// Vetor do `btest-opensource`: senha `test`, desafio
/// `ad32d6f94d28161625f2f390bb895637` → `3c968565bc0314f281a6da1571cf7255`.
pub(super) fn md5_auth_hash(password: &[u8], nonce: &[u8; NONCE_LEN]) -> [u8; 16] {
    let inner = Md5::new()
        .chain_update(password)
        .chain_update(nonce)
        .finalize();
    Md5::new()
        .chain_update(password)
        .chain_update(inner)
        .finalize()
        .into()
}

/// Resposta MD5 do cliente: 16 bytes de hash + usuário em 32 bytes com `\0`.
pub(super) fn md5_auth_reply(
    user: &str,
    password: &str,
    nonce: &[u8; NONCE_LEN],
) -> AppResult<[u8; AUTH_REPLY_LEN]> {
    let user = user.as_bytes();
    if user.len() >= USER_LEN {
        return Err(AppError::validation(format!(
            "O usuário do btest deve ter menos de {USER_LEN} bytes"
        )));
    }
    let mut reply = [0u8; AUTH_REPLY_LEN];
    reply[..16].copy_from_slice(&md5_auth_hash(password.as_bytes(), nonce));
    reply[16..16 + user.len()].copy_from_slice(user);
    Ok(reply)
}

pub(super) fn protocol_error(detail: &str) -> AppError {
    AppError::business_rule(format!("Resposta fora do protocolo btest: {detail}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
            .collect()
    }

    #[test]
    fn comandos_das_capturas_do_readme() {
        // "01:01:01:00:00:80:00:00:..." = transmit (cliente envia), TCP.
        let transmit = Command::tcp(true, false, None).encode();
        assert_eq!(transmit.to_vec(), hex("01010100008000000000000000000000"));
        let receive = Command::tcp(false, true, None).encode();
        assert_eq!(receive.to_vec(), hex("01020100008000000000000000000000"));
        let both = Command::tcp(true, true, None).encode();
        assert_eq!(both.to_vec(), hex("01030100008000000000000000000000"));
        // remote-tx-speed=4294967295 e local-tx-speed=1.
        let mut buf = [0u8; COMMAND_LEN];
        buf.copy_from_slice(&hex("010201000080000000000000ffffffff"));
        let cmd = Command::decode(&buf).expect("válido");
        assert_eq!((cmd.remote_tx_speed, cmd.local_tx_speed), (0, u32::MAX));
        buf.copy_from_slice(&hex("01020100008000000100000000000000"));
        let cmd = Command::decode(&buf).expect("válido");
        assert_eq!(cmd.remote_tx_speed, 1);
        assert!(cmd.client_rx && !cmd.client_tx && cmd.zero_data);
        buf.copy_from_slice(&hex("0102010a008000000000000000000000"));
        assert_eq!(
            Command::decode(&buf).expect("válido").tcp_connection_count,
            10
        );
    }

    #[test]
    fn comando_ida_e_volta_com_limite() {
        let cmd = Command::tcp(true, true, Some(50_000_000));
        let lido = Command::decode(&cmd.encode()).expect("válido");
        assert_eq!(lido, cmd);
        assert_eq!(lido.remote_tx_speed, 50_000_000);
        let saturado = Command::tcp(true, false, Some(10_000_000_000));
        assert_eq!(saturado.local_tx_speed, u32::MAX);
    }

    #[test]
    fn recusa_comando_invalido() {
        let mut buf = Command::tcp(true, false, None).encode();
        buf[1] = 0;
        assert!(Command::decode(&buf).is_err());
        buf[1] = 0x04;
        assert!(Command::decode(&buf).is_err());
        buf[1] = 0x01;
        buf[0] = 0x09;
        assert!(Command::decode(&buf).is_err());
        buf[0] = 0x00;
        assert!(!Command::decode(&buf).expect("UDP decodifica").tcp);
    }

    #[test]
    fn estado_da_captura() {
        // `07:00:00:00:01:00:00:00:36:6e:03:00` (README): seq 1, 224 822 bytes.
        let mut buf = [0u8; STATUS_LEN];
        buf.copy_from_slice(&hex("0700000001000000366e0300"));
        let status = Status::decode(&buf).expect("estado");
        assert_eq!(
            status,
            Status {
                seq: 1,
                received: 0x0003_6e36
            }
        );
        assert_eq!(status.encode(), buf);
        assert_eq!(Status::decode(&[0u8; STATUS_LEN]), None);
    }

    #[test]
    fn scanner_separa_estados_dos_dados() {
        let mut scanner = StatusScanner::default();
        let a = Status {
            seq: 1,
            received: 1_000,
        }
        .encode();
        let b = Status {
            seq: 2,
            received: 7,
        }
        .encode();
        let mut stream = vec![0u8; 100];
        stream.extend_from_slice(&a);
        stream.extend_from_slice(&[0u8; 50]);
        stream.extend_from_slice(&b);
        // Entregue em pedaços que cortam as mensagens ao meio.
        for chunk in stream.chunks(5) {
            scanner.feed(chunk);
        }
        assert_eq!(scanner.statuses, 2);
        assert_eq!(scanner.peer_received, 1_007);
        assert_eq!(scanner.control_bytes(), 24);
        // Byte desconhecido fora de mensagem é dado, não quebra a leitura.
        scanner.feed(&[0x01, 0x00, 0x00, 0x00]);
        assert_eq!(scanner.statuses, 2);
    }

    #[test]
    fn hash_md5_do_vetor_publicado() {
        let mut nonce = [0u8; NONCE_LEN];
        nonce.copy_from_slice(&hex("ad32d6f94d28161625f2f390bb895637"));
        assert_eq!(
            md5_auth_hash(b"test", &nonce).to_vec(),
            hex("3c968565bc0314f281a6da1571cf7255")
        );
    }

    #[test]
    fn resposta_md5_leva_usuario_com_nul() {
        let nonce = [7u8; NONCE_LEN];
        let reply = md5_auth_reply("test", "segredo", &nonce).expect("válido");
        assert_eq!(&reply[..16], &md5_auth_hash(b"segredo", &nonce));
        assert_eq!(&reply[16..20], b"test");
        assert!(reply[20..].iter().all(|b| *b == 0));
        assert!(md5_auth_reply(&"u".repeat(32), "x", &nonce).is_err());
    }
}
