//! Formato de fio do iperf3 3.x (canal de controle), sem E/S de teste.
//!
//! Conferido contra `esnet/iperf` (`src/iperf_api.h`, `iperf_api.c`,
//! `iperf_client_api.c`):
//!
//! * o cliente abre o controle e manda o **cookie** — 36 caracteres de
//!   `[a-z2-7]` mais o `\0`, 37 bytes (`COOKIE_SIZE`);
//! * o servidor dirige a máquina de estados com **um byte com sinal** por
//!   mensagem; o cliente só escreve `TEST_END`, `IPERF_DONE` e
//!   `CLIENT_TERMINATE`;
//! * parâmetros e resultados vão em JSON precedido do tamanho em 4 bytes
//!   big-endian (`JSON_write`/`JSON_read`);
//! * `SERVER_ERROR` vem seguido de dois inteiros de 32 bits big-endian
//!   (`i_errno` e `errno`);
//! * os fluxos recebem ids 1, 3, 4, 5… (`iperf_add_stream`, mantido assim por
//!   compatibilidade entre versões) — os resultados citam esses ids.

use rand::Rng;
use serde_json::{json, Value};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::services::shared::errors::{AppError, AppResult};

/// Tamanho do cookie, `\0` incluso.
pub(super) const COOKIE_SIZE: usize = 37;
/// Bloco padrão de leitura/escrita do iperf3 em TCP (`DEFAULT_TCP_BLKSIZE`).
pub(super) const DEFAULT_TCP_BLKSIZE: usize = 128 * 1024;
/// Teto do JSON de resultados aceito do servidor.
const MAX_JSON_LEN: u32 = 1024 * 1024;
const COOKIE_ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz234567";

/// Estados do protocolo (`iperf_api.h`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum State {
    TestStart,
    TestRunning,
    TestEnd,
    ParamExchange,
    CreateStreams,
    ServerTerminate,
    ClientTerminate,
    ExchangeResults,
    DisplayResults,
    IperfStart,
    IperfDone,
    AccessDenied,
    ServerError,
}

impl State {
    pub(super) const fn code(self) -> i8 {
        match self {
            Self::TestStart => 1,
            Self::TestRunning => 2,
            Self::TestEnd => 4,
            Self::ParamExchange => 9,
            Self::CreateStreams => 10,
            Self::ServerTerminate => 11,
            Self::ClientTerminate => 12,
            Self::ExchangeResults => 13,
            Self::DisplayResults => 14,
            Self::IperfStart => 15,
            Self::IperfDone => 16,
            Self::AccessDenied => -1,
            Self::ServerError => -2,
        }
    }

    pub(super) fn from_code(code: i8) -> Option<Self> {
        Some(match code {
            1 => Self::TestStart,
            2 => Self::TestRunning,
            4 => Self::TestEnd,
            9 => Self::ParamExchange,
            10 => Self::CreateStreams,
            11 => Self::ServerTerminate,
            12 => Self::ClientTerminate,
            13 => Self::ExchangeResults,
            14 => Self::DisplayResults,
            15 => Self::IperfStart,
            16 => Self::IperfDone,
            -1 => Self::AccessDenied,
            -2 => Self::ServerError,
            _ => return None,
        })
    }
}

/// Cookie novo: identifica o teste no controle e em cada fluxo de dados.
pub(super) fn make_cookie() -> [u8; COOKIE_SIZE] {
    let mut rng = rand::thread_rng();
    let mut cookie = [0u8; COOKIE_SIZE];
    for byte in cookie.iter_mut().take(COOKIE_SIZE - 1) {
        *byte = COOKIE_ALPHABET[rng.gen_range(0..COOKIE_ALPHABET.len())];
    }
    cookie
}

/// Id do fluxo de índice `index` (0-based), como o iperf3 numera.
pub(super) fn stream_id(index: usize) -> u64 {
    if index == 0 {
        1
    } else {
        index as u64 + 2
    }
}

/// Parâmetros do teste (`send_parameters`). `bandwidth` é **por fluxo** no
/// iperf3; o teto total é dividido entre eles.
pub(super) fn params_json(
    duration_s: u8,
    streams: u8,
    reverse: bool,
    limit_bps: Option<u64>,
) -> Value {
    let mut params = json!({
        "tcp": true,
        "omit": 0,
        "time": duration_s,
        "num": 0,
        "blockcount": 0,
        "parallel": streams,
        "len": DEFAULT_TCP_BLKSIZE,
        "pacing_timer": 1000,
        "client_version": concat!("netmonitor-", env!("CARGO_PKG_VERSION")),
    });
    if reverse {
        params["reverse"] = Value::Bool(true);
    }
    if let Some(total) = limit_bps {
        let per_stream = (total / u64::from(streams.max(1))).max(1);
        params["bandwidth"] = json!(per_stream);
    }
    params
}

/// Resultados do cliente (`send_results`). Não medimos CPU nem
/// retransmissões do lado de cá; o servidor só exige os campos presentes.
pub(super) fn results_json(per_stream_bytes: &[u64], sender: bool, elapsed_secs: f64) -> Value {
    let streams: Vec<Value> = per_stream_bytes
        .iter()
        .enumerate()
        .map(|(index, bytes)| {
            json!({
                "id": stream_id(index),
                "bytes": bytes,
                "retransmits": -1,
                "jitter": 0,
                "errors": 0,
                "omitted_errors": 0,
                "packets": 0,
                "omitted_packets": 0,
                "start_time": 0,
                "end_time": elapsed_secs,
            })
        })
        .collect();
    json!({
        "cpu_util_total": 0,
        "cpu_util_user": 0,
        "cpu_util_system": 0,
        "sender_has_retransmits": if sender { 0 } else { -1 },
        "streams": streams,
    })
}

/// O que interessa dos resultados do servidor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ServerResults {
    /// Soma dos bytes dos fluxos (recebidos ou enviados, conforme o papel).
    pub bytes: u64,
    /// Retransmissões, quando o servidor foi quem enviou e as mediu.
    pub retransmits: Option<u64>,
}

/// Lê os resultados do servidor (`get_results`).
pub(super) fn parse_server_results(value: &Value) -> AppResult<ServerResults> {
    let invalid = || malformed("resultados sem a lista de fluxos");
    let streams = value
        .get("streams")
        .and_then(Value::as_array)
        .ok_or_else(invalid)?;
    let sender_has_retransmits =
        value.get("sender_has_retransmits").and_then(Value::as_i64) == Some(1);
    let mut bytes = 0u64;
    let mut retransmits = 0u64;
    for stream in streams {
        let stream_bytes = stream
            .get("bytes")
            .and_then(as_non_negative)
            .ok_or_else(|| malformed("fluxo sem contagem de bytes"))?;
        bytes = bytes.saturating_add(stream_bytes);
        if let Some(r) = stream.get("retransmits").and_then(as_non_negative) {
            retransmits = retransmits.saturating_add(r);
        }
    }
    Ok(ServerResults {
        bytes,
        retransmits: sender_has_retransmits.then_some(retransmits),
    })
}

/// Número JSON não negativo (o iperf3 grava inteiros como `double`).
fn as_non_negative(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| {
        value
            .as_f64()
            .filter(|v| *v >= 0.0 && v.is_finite())
            .map(|v| v as u64)
    })
}

pub(super) fn malformed(detail: &str) -> AppError {
    AppError::business_rule(format!("Resposta inválida do servidor iperf3: {detail}"))
}

pub(super) async fn read_state<R: AsyncRead + Unpin>(reader: &mut R) -> AppResult<State> {
    let code = reader.read_i8().await.map_err(|_| control_closed())?;
    State::from_code(code).ok_or_else(|| malformed(&format!("estado desconhecido {code}")))
}

pub(super) async fn write_state<W: AsyncWrite + Unpin>(
    writer: &mut W,
    state: State,
) -> AppResult<()> {
    writer
        .write_i8(state.code())
        .await
        .map_err(|_| control_closed())
}

/// `SERVER_ERROR`: lê `i_errno` e `errno` e monta o erro.
pub(super) async fn read_server_error<R: AsyncRead + Unpin>(reader: &mut R) -> AppError {
    let codes = async {
        let i_errno = reader.read_i32().await?;
        let errno = reader.read_i32().await?;
        Ok::<_, std::io::Error>((i_errno, errno))
    };
    match codes.await {
        Ok((i_errno, errno)) => AppError::business_rule(format!(
            "O servidor iperf3 relatou erro (código {i_errno}, errno {errno})"
        )),
        Err(_) => malformed("erro do servidor sem código"),
    }
}

pub(super) async fn write_json<W: AsyncWrite + Unpin>(
    writer: &mut W,
    value: &Value,
) -> AppResult<()> {
    let body = serde_json::to_vec(value).map_err(|e| AppError::Internal(e.into()))?;
    let len = u32::try_from(body.len()).map_err(|_| malformed("JSON grande demais"))?;
    writer.write_u32(len).await.map_err(|_| control_closed())?;
    writer.write_all(&body).await.map_err(|_| control_closed())
}

pub(super) async fn read_json<R: AsyncRead + Unpin>(reader: &mut R) -> AppResult<Value> {
    let len = reader.read_u32().await.map_err(|_| control_closed())?;
    if len == 0 || len > MAX_JSON_LEN {
        return Err(malformed(&format!("JSON com tamanho {len}")));
    }
    let mut body = vec![0u8; len as usize];
    reader
        .read_exact(&mut body)
        .await
        .map_err(|_| control_closed())?;
    serde_json::from_slice(&body).map_err(|_| malformed("JSON ilegível"))
}

pub(super) fn control_closed() -> AppError {
    AppError::service_unavailable("O servidor iperf3 encerrou a conexão de controle")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estados_ida_e_volta() {
        for code in [1i8, 2, 4, 9, 10, 11, 12, 13, 14, 15, 16, -1, -2] {
            let state = State::from_code(code).expect("estado conhecido");
            assert_eq!(state.code(), code);
        }
        assert_eq!(State::from_code(3), None);
        assert_eq!(State::from_code(0), None);
        assert_eq!(State::AccessDenied.code() as u8, 0xFF);
    }

    #[test]
    fn cookie_tem_36_caracteres_e_nul() {
        let cookie = make_cookie();
        assert_eq!(cookie.len(), 37);
        assert_eq!(cookie[36], 0);
        assert!(cookie[..36].iter().all(|b| COOKIE_ALPHABET.contains(b)));
        assert_ne!(make_cookie(), cookie);
    }

    #[test]
    fn ids_dos_fluxos_seguem_o_iperf3() {
        let ids: Vec<u64> = (0..4).map(stream_id).collect();
        assert_eq!(ids, vec![1, 3, 4, 5]);
    }

    #[test]
    fn parametros_basicos() {
        let p = params_json(5, 2, false, None);
        assert_eq!(p["tcp"], true);
        assert_eq!(p["time"], 5);
        assert_eq!(p["parallel"], 2);
        assert_eq!(p["len"], 131_072);
        assert!(p.get("reverse").is_none());
        assert!(p.get("bandwidth").is_none());
        let r = params_json(3, 4, true, Some(100_000_000));
        assert_eq!(r["reverse"], true);
        assert_eq!(r["bandwidth"], 25_000_000);
    }

    #[test]
    fn resultados_do_cliente() {
        let r = results_json(&[10, 20], true, 1.5);
        assert_eq!(r["sender_has_retransmits"], 0);
        assert_eq!(r["streams"][0]["id"], 1);
        assert_eq!(r["streams"][1]["id"], 3);
        assert_eq!(r["streams"][1]["bytes"], 20);
        assert_eq!(r["streams"][1]["end_time"], 1.5);
        assert_eq!(results_json(&[1], false, 1.0)["sender_has_retransmits"], -1);
    }

    #[test]
    fn le_resultados_do_servidor() {
        let v = json!({
            "sender_has_retransmits": 1,
            "streams": [
                {"id": 1, "bytes": 1000.0, "retransmits": 3},
                {"id": 3, "bytes": 500, "retransmits": 2}
            ]
        });
        assert_eq!(
            parse_server_results(&v).expect("válido"),
            ServerResults {
                bytes: 1_500,
                retransmits: Some(5)
            }
        );
        let receptor = json!({
            "sender_has_retransmits": -1,
            "streams": [{"id": 1, "bytes": 42, "retransmits": -1}]
        });
        assert_eq!(
            parse_server_results(&receptor).expect("válido"),
            ServerResults {
                bytes: 42,
                retransmits: None
            }
        );
    }

    #[test]
    fn recusa_resultados_malformados() {
        assert!(parse_server_results(&json!({})).is_err());
        assert!(parse_server_results(&json!({"streams": [{"id": 1}]})).is_err());
        assert!(parse_server_results(&json!({"streams": [{"bytes": -5}]})).is_err());
    }

    #[tokio::test]
    async fn json_com_prefixo_de_tamanho() {
        let mut buf = Vec::new();
        write_json(&mut buf, &json!({"a": 1}))
            .await
            .expect("escreve");
        assert_eq!(&buf[..4], &[0, 0, 0, 7]);
        let lido = read_json(&mut buf.as_slice()).await.expect("lê");
        assert_eq!(lido, json!({"a": 1}));
        let vazio: &[u8] = &[0, 0, 0, 0];
        assert!(read_json(&mut &vazio[..]).await.is_err());
        let lixo: &[u8] = &[0, 0, 0, 2, b'{', b'x'];
        assert!(read_json(&mut &lixo[..]).await.is_err());
    }
}
