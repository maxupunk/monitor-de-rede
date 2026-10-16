//! Quem **dirige** um teste nativo (o lado cliente) — a central ou um agente.
//!
//! O orquestrador do teste de enlace (ADR 015 §5) não sabe de onde o teste
//! parte: pede a um [`ThroughputDriver`] e recebe as amostras e o relatório.
//! [`LocalDriver`] dirige deste processo; o agente remoto implementa o mesmo
//! contrato pelo `Command::Throughput` (ADR 015 §6).

use std::net::SocketAddr;

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use super::{btest, iperf3, SampleSink, ThroughputEngine, ThroughputReport, ThroughputSpec};
use crate::services::shared::errors::AppResult;

/// Um teste a dirigir: alvo, especificação e, para o btest, a credencial do
/// teste (gerada pelo orquestrador; `None` = servidor sem autenticação).
#[derive(Debug, Clone)]
pub struct DriveRequest {
    pub target: SocketAddr,
    pub spec: ThroughputSpec,
    pub auth: Option<btest::BtestAuth>,
}

/// O contrato que o orquestrador conhece.
#[async_trait]
pub trait ThroughputDriver: Send + Sync {
    /// Dirige o teste até o fim, mandando uma amostra por segundo ao `sink`.
    ///
    /// # Errors
    ///
    /// Os mesmos dos motores: alvo inalcançável, protocolo violado, servidor
    /// ocupado, cancelamento.
    async fn drive(
        &self,
        request: &DriveRequest,
        sink: &SampleSink,
        cancel: &CancellationToken,
    ) -> AppResult<ThroughputReport>;

    /// De onde o teste parte, para a tela ("central", "agente Filial").
    fn origin(&self) -> String;
}

/// Dirige a partir deste processo.
#[derive(Debug, Clone, Copy, Default)]
pub struct LocalDriver;

#[async_trait]
impl ThroughputDriver for LocalDriver {
    async fn drive(
        &self,
        request: &DriveRequest,
        sink: &SampleSink,
        cancel: &CancellationToken,
    ) -> AppResult<ThroughputReport> {
        match request.spec.engine {
            ThroughputEngine::Iperf3 => {
                iperf3::run_client(request.target, &request.spec, sink, cancel).await
            }
            ThroughputEngine::Btest => {
                btest::run_client(
                    request.target,
                    &request.spec,
                    request.auth.as_ref(),
                    sink,
                    cancel,
                )
                .await
            }
        }
    }

    fn origin(&self) -> String {
        "central".to_owned()
    }
}
