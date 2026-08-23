//! Sondagem TCP de porta única compartilhada por monitores e scanners.
//!
//! A classificação segue a semântica observável de um TCP connect: conexão
//! aceita prova porta aberta; `ConnectionRefused` prova que o host respondeu e
//! a porta está fechada; silêncio permanece filtrado/inconclusivo.

use std::{io, time::Duration};

use serde::Serialize;
use tokio::{
    net::{TcpStream, ToSocketAddrs},
    time::timeout,
};

/// Estado observável de uma tentativa TCP, sem inferir além da evidência.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TcpProbeState {
    Open,
    Closed,
    Filtered,
    Unreachable,
    Error,
}

impl TcpProbeState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Closed => "closed",
            Self::Filtered => "filtered",
            Self::Unreachable => "unreachable",
            Self::Error => "error",
        }
    }

    /// `open` e `closed` exigem resposta do alvo e, portanto, provam alcance.
    #[must_use]
    pub const fn proves_reachability(self) -> bool {
        matches!(self, Self::Open | Self::Closed)
    }

    #[must_use]
    pub const fn retryable(self) -> bool {
        matches!(self, Self::Filtered | Self::Unreachable)
    }
}

/// Observação de uma única tentativa de conexão.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TcpProbeObservation {
    pub state: TcpProbeState,
    pub latency_ms: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Executa uma única tentativa TCP e descarta a conexão assim que classificada.
pub async fn probe_tcp<A: ToSocketAddrs>(
    target: A,
    timeout_duration: Duration,
) -> TcpProbeObservation {
    let started = std::time::Instant::now();
    let outcome = timeout(timeout_duration, TcpStream::connect(target)).await;
    let (state, error) = classify_tcp_outcome(outcome);
    TcpProbeObservation {
        state,
        latency_ms: millis(started.elapsed()),
        error,
    }
}

fn classify_tcp_outcome(
    outcome: Result<io::Result<TcpStream>, tokio::time::error::Elapsed>,
) -> (TcpProbeState, Option<String>) {
    match outcome {
        Ok(Ok(_)) => (TcpProbeState::Open, None),
        Ok(Err(error)) if error.kind() == io::ErrorKind::ConnectionRefused => {
            (TcpProbeState::Closed, None)
        }
        Ok(Err(error))
            if matches!(
                error.kind(),
                io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
            ) =>
        {
            (TcpProbeState::Filtered, Some(error.to_string()))
        }
        Ok(Err(error))
            if matches!(
                error.kind(),
                io::ErrorKind::HostUnreachable | io::ErrorKind::NetworkUnreachable
            ) =>
        {
            (TcpProbeState::Unreachable, Some(error.to_string()))
        }
        Ok(Err(error)) => (TcpProbeState::Error, Some(error.to_string())),
        Err(error) => (TcpProbeState::Filtered, Some(error.to_string())),
    }
}

fn millis(duration: Duration) -> f64 {
    (duration.as_secs_f64() * 1_000.0 * 1_000.0).round() / 1_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn conexao_aceita_prova_que_o_host_respondeu() {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        let observation = probe_tcp(address, Duration::from_secs(1)).await;
        assert_eq!(observation.state, TcpProbeState::Open);
        assert!(observation.state.proves_reachability());
    }

    #[test]
    fn recusa_de_conexao_tambem_prova_que_o_host_respondeu() {
        let outcome = Ok(Err(io::Error::from(io::ErrorKind::ConnectionRefused)));
        let (state, error) = classify_tcp_outcome(outcome);
        assert_eq!(state, TcpProbeState::Closed);
        assert!(state.proves_reachability());
        assert!(error.is_none());
    }
}
