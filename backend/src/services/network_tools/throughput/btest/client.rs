//! Cliente btest nativo (agente → `bandwidth-server` do MikroTik).
//!
//! Fala TCP com uma conexão, sem autenticação ou com o MD5 do RouterOS
//! < 6.43. O EC-SRP5 (RouterOS ≥ 6.43 com autenticação ligada) é recusado
//! com erro próprio: o plugin do MikroTik desliga a autenticação durante a
//! janela do teste (ADR 015 §4).

use std::net::SocketAddr;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};
use tokio_util::sync::CancellationToken;

use super::{
    codec::{
        md5_auth_reply, protocol_error, Command, AUTH_EC_SRP5, AUTH_FAILED, AUTH_MD5, AUTH_OK,
        HELLO, NONCE_LEN, TCP_TX_SIZE,
    },
    duplex::{self, DuplexPlan},
    BtestAuth, AUTH_FAILED_MESSAGE, AUTH_REQUIRED_MESSAGE, EC_SRP5_MESSAGE,
};
use crate::services::{
    network_tools::throughput::{
        cancelled, Flow, SampleSink, ThroughputEngine, ThroughputReport, ThroughputSpec,
        HANDSHAKE_TIMEOUT,
    },
    shared::errors::{AppError, AppResult},
};

/// Executa um teste btest contra o servidor em `addr` (porta 2000).
///
/// `spec.engine` precisa ser [`ThroughputEngine::Btest`] e `spec.streams`, 1.
/// Com `direction = both` os dois sentidos correm **ao mesmo tempo** na mesma
/// conexão (é como o btest funciona). `auth` só é usado se o servidor pedir
/// o MD5. Cada segundo de medição vira uma amostra no `sink`.
///
/// # Errors
///
/// * [`AppError::Validation`] — especificação fora do contrato;
/// * [`AppError::ServiceUnavailable`] — servidor inalcançável ou mudo;
/// * [`AppError::BusinessRule`] — autenticação exigida sem credencial,
///   recusada, EC-SRP5 (ver [`super::is_ec_srp5_unsupported`]), resposta fora
///   do protocolo ou cancelamento.
pub async fn run_client(
    addr: SocketAddr,
    spec: &ThroughputSpec,
    auth: Option<&BtestAuth>,
    sink: &SampleSink,
    cancel: &CancellationToken,
) -> AppResult<ThroughputReport> {
    spec.validate_for(ThroughputEngine::Btest)?;
    if spec.streams != 1 {
        return Err(AppError::validation(
            "O cliente btest nativo usa uma conexão TCP: peça 1 fluxo",
        ));
    }
    let sends = spec.direction.sends();
    let receives = spec.direction.receives();
    let command = Command::tcp(sends, receives, spec.limit_bps());
    let stream = tokio::select! {
        () = cancel.cancelled() => return Err(cancelled()),
        stream = handshake(addr, &command, auth) => stream?,
    };
    let plan = DuplexPlan {
        send: sends,
        receive: receives,
        block: usize::from(TCP_TX_SIZE),
        pace_bps: spec.limit_bps(),
        max_duration: spec.duration(),
        sent_flow: Flow::Tx,
        received_flow: Flow::Rx,
    };
    let outcome = duplex::run(stream, &plan, sink, cancel).await?;
    let mut report = ThroughputReport::empty(ThroughputEngine::Btest);
    if sends {
        report.set_tx(outcome.sent, outcome.elapsed);
    }
    if receives {
        report.set_rx(outcome.received, outcome.elapsed);
    }
    report.samples = outcome.samples;
    Ok(report)
}

async fn handshake(
    addr: SocketAddr,
    command: &Command,
    auth: Option<&BtestAuth>,
) -> AppResult<TcpStream> {
    let silent =
        || AppError::service_unavailable(format!("O servidor btest em {addr} parou de responder"));
    let closed =
        |_| AppError::service_unavailable(format!("O servidor btest em {addr} fechou a conexão"));

    let mut stream = timeout(HANDSHAKE_TIMEOUT, TcpStream::connect(addr))
        .await
        .map_err(|_| silent())?
        .map_err(|error| {
            AppError::service_unavailable(format!(
                "Não foi possível conectar ao servidor btest em {addr}: {error}"
            ))
        })?;
    let _ = stream.set_nodelay(true);

    let mut word = [0u8; 4];
    timeout(HANDSHAKE_TIMEOUT, stream.read_exact(&mut word))
        .await
        .map_err(|_| silent())?
        .map_err(closed)?;
    if word != HELLO {
        return Err(protocol_error(&format!("hello inesperado {word:02x?}")));
    }
    timeout(HANDSHAKE_TIMEOUT, stream.write_all(&command.encode()))
        .await
        .map_err(|_| silent())?
        .map_err(closed)?;
    timeout(HANDSHAKE_TIMEOUT, stream.read_exact(&mut word))
        .await
        .map_err(|_| silent())?
        .map_err(closed)?;

    match word {
        AUTH_OK => Ok(stream),
        AUTH_MD5 => {
            let auth = auth.ok_or_else(|| AppError::business_rule(AUTH_REQUIRED_MESSAGE))?;
            let mut nonce = [0u8; NONCE_LEN];
            timeout(HANDSHAKE_TIMEOUT, stream.read_exact(&mut nonce))
                .await
                .map_err(|_| silent())?
                .map_err(closed)?;
            let reply = md5_auth_reply(&auth.user, &auth.password, &nonce)?;
            timeout(HANDSHAKE_TIMEOUT, stream.write_all(&reply))
                .await
                .map_err(|_| silent())?
                .map_err(closed)?;
            timeout(HANDSHAKE_TIMEOUT, stream.read_exact(&mut word))
                .await
                .map_err(|_| silent())?
                .map_err(closed)?;
            match word {
                AUTH_OK => Ok(stream),
                AUTH_FAILED => Err(AppError::business_rule(AUTH_FAILED_MESSAGE)),
                other => Err(protocol_error(&format!(
                    "resposta de autenticação {other:02x?}"
                ))),
            }
        }
        AUTH_EC_SRP5 => Err(AppError::business_rule(EC_SRP5_MESSAGE)),
        other => Err(protocol_error(&format!("resposta ao comando {other:02x?}"))),
    }
}
