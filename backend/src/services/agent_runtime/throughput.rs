//! Teste de vazão dirigido pelo agente (`Command::Throughput`, ADR 015 §6).
//!
//! O agente é sempre o **cliente**: disca até o servidor iperf3/btest do
//! equipamento, nunca abre porta. Roda o mesmo [`LocalDriver`] da central;
//! cada amostra de 1 s vira um `Chunk` e o relatório é a resposta. Um teste
//! por agente de cada vez — dois ao mesmo tempo dividiriam o enlace e as duas
//! medidas sairiam erradas.

use std::net::{IpAddr, SocketAddr};

use serde_json::Value;
use tokio::sync::{mpsc, Semaphore};
use tokio_util::sync::CancellationToken;

use super::targets::DeviceNetworks;
use crate::services::{
    agents::protocol::{ErrorCode, RemoteError, ThroughputAuth},
    network_tools::throughput::{
        driver::{DriveRequest, LocalDriver, ThroughputDriver},
        sample_channel, ThroughputSample, ThroughputSpec,
    },
};

/// Mensagem do segundo teste pedido enquanto um roda.
pub const BUSY_MESSAGE: &str =
    "Este agente já está executando um teste de vazão; aguarde o fim dele";

pub struct ThroughputRunner {
    slot: Semaphore,
}

impl Default for ThroughputRunner {
    fn default() -> Self {
        Self {
            slot: Semaphore::new(1),
        }
    }
}

impl ThroughputRunner {
    /// Valida alvo e especificação, ocupa a vaga e dirige o teste até o fim
    /// (ou até `cancel`).
    ///
    /// # Errors
    ///
    /// `Forbidden` para alvo fora das redes do host, `Validation` para
    /// especificação fora do contrato, `Conflict` com outro teste em curso e
    /// os erros do motor (ver [`RemoteError::from`] para `AppError`).
    pub async fn run(
        &self,
        networks: &DeviceNetworks,
        request: DriveRequest,
        chunks: mpsc::UnboundedSender<Value>,
        cancel: CancellationToken,
    ) -> Result<Value, RemoteError> {
        networks.check(request.target.ip())?;
        request.spec.validate().map_err(RemoteError::from)?;
        let _slot = self
            .slot
            .try_acquire()
            .map_err(|_| RemoteError::new(ErrorCode::Conflict, BUSY_MESSAGE))?;

        let (sink, mut samples) = sample_channel();
        let forward = |sample: ThroughputSample| {
            if let Ok(value) = serde_json::to_value(sample) {
                let _ = chunks.send(value);
            }
        };
        let driver = LocalDriver;
        let drive = driver.drive(&request, &sink, &cancel);
        tokio::pin!(drive);
        let result = loop {
            tokio::select! {
                biased;
                Some(sample) = samples.recv() => forward(sample),
                result = &mut drive => break result,
            }
        };
        while let Ok(sample) = samples.try_recv() {
            forward(sample);
        }
        let report = result.map_err(RemoteError::from)?;
        serde_json::to_value(report)
            .map_err(|error| RemoteError::new(ErrorCode::Internal, error.to_string()))
    }
}

/// O pedido do motor a partir dos campos do comando.
#[must_use]
pub fn drive_request(
    target: IpAddr,
    port: u16,
    spec: ThroughputSpec,
    auth: Option<ThroughputAuth>,
) -> DriveRequest {
    DriveRequest {
        target: SocketAddr::new(target, port),
        spec,
        auth: auth.map(Into::into),
    }
}
