//! Os comandos da área Rede no host do agente (ADR 017,
//! `AgentCommandHandler`): a descoberta (`Command::Discovery`), o acesso a
//! equipamento dos plugins (`Command::DeviceIo`) e o teste de vazão
//! (`Command::Throughput`). O executor do agente aplica a política local
//! (`AGENT_ALLOW`) antes e só então entrega o comando ao handler.

use std::sync::Arc;

use agent_protocol::{Command, ErrorCode, RemoteError};
use async_trait::async_trait;
use netmonitor_core::{
    agents::{command::AgentCommandHandler, targets::DeviceNetworks},
    monitoring::CheckDeps,
};
use serde::Serialize;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{
    discovery::service::scan_network_with,
    plugins::transport::{local::LocalTransport, DeviceIoCall, DeviceTransport},
    throughput::agent_runner::{drive_request, ThroughputRunner},
};

fn to_value<T: Serialize>(value: T) -> Result<Value, RemoteError> {
    serde_json::to_value(value)
        .map_err(|error| RemoteError::new(ErrorCode::Internal, error.to_string()))
}

/// Os handlers desta área. `device` fixa o transporte de equipamento (testes);
/// `None` = um `LocalTransport` por chamada.
#[must_use]
pub fn handlers(
    deps: CheckDeps,
    networks: DeviceNetworks,
    device: Option<Arc<dyn DeviceTransport>>,
) -> Vec<Arc<dyn AgentCommandHandler>> {
    vec![
        Arc::new(DiscoveryCommand { deps }),
        Arc::new(DeviceIoCommand {
            networks: networks.clone(),
            device,
        }),
        Arc::new(ThroughputCommand {
            networks,
            runner: ThroughputRunner::default(),
        }),
    ]
}

/// A varredura de uma rede a partir do agente, com os mesmos checkers da
/// central (`scan_network_with`).
pub struct DiscoveryCommand {
    deps: CheckDeps,
}

#[async_trait]
impl AgentCommandHandler for DiscoveryCommand {
    fn handles(&self, command: &Command) -> bool {
        matches!(command, Command::Discovery { .. })
    }

    async fn execute(
        &self,
        command: Command,
        _chunks: mpsc::UnboundedSender<Value>,
        cancel: CancellationToken,
    ) -> Result<Value, RemoteError> {
        let Command::Discovery { task } = command else {
            return Err(unexpected());
        };
        let hosts = scan_network_with(&self.deps, &task.cidr, cancel)
            .await
            .map_err(|error| RemoteError::new(ErrorCode::Internal, error.to_string()))?;
        to_value(hosts)
    }
}

/// Uma operação de equipamento. Cada linha da saída vai num chunk de texto
/// enquanto o comando roda; a resposta é a completa.
pub struct DeviceIoCommand {
    /// Alvos de equipamento aceitos (`AGENT_DEVICE_CIDRS`).
    networks: DeviceNetworks,
    device: Option<Arc<dyn DeviceTransport>>,
}

impl DeviceIoCommand {
    async fn device_io(
        &self,
        call: &DeviceIoCall,
        chunks: mpsc::UnboundedSender<Value>,
    ) -> Result<Value, RemoteError> {
        let ip = call
            .endpoint()
            .ip()
            .map_err(|error| RemoteError::new(ErrorCode::Validation, error))?;
        self.networks.check(ip)?;
        let transport: Arc<dyn DeviceTransport> = match &self.device {
            Some(device) => device.clone(),
            None => Arc::new(
                LocalTransport::new("agente")
                    .map_err(|error| RemoteError::new(ErrorCode::Internal, error.to_string()))?,
            ),
        };
        let reply = transport
            .execute_streaming(call, &|line: &str| {
                let _ = chunks.send(Value::String(line.to_owned()));
            })
            .await
            .map_err(|error| RemoteError::new(ErrorCode::Unavailable, error.to_string()))?;
        to_value(reply)
    }
}

#[async_trait]
impl AgentCommandHandler for DeviceIoCommand {
    fn handles(&self, command: &Command) -> bool {
        matches!(command, Command::DeviceIo { .. })
    }

    async fn execute(
        &self,
        command: Command,
        chunks: mpsc::UnboundedSender<Value>,
        _cancel: CancellationToken,
    ) -> Result<Value, RemoteError> {
        let Command::DeviceIo { call } = command else {
            return Err(unexpected());
        };
        self.device_io(&call, chunks).await
    }
}

/// O teste de vazão dirigido pelo agente (um por vez).
pub struct ThroughputCommand {
    networks: DeviceNetworks,
    runner: ThroughputRunner,
}

#[async_trait]
impl AgentCommandHandler for ThroughputCommand {
    fn handles(&self, command: &Command) -> bool {
        matches!(command, Command::Throughput { .. })
    }

    async fn execute(
        &self,
        command: Command,
        chunks: mpsc::UnboundedSender<Value>,
        cancel: CancellationToken,
    ) -> Result<Value, RemoteError> {
        let Command::Throughput {
            target,
            port,
            spec,
            auth,
        } = command
        else {
            return Err(unexpected());
        };
        self.runner
            .run(
                &self.networks,
                drive_request(target, port, spec, auth),
                chunks,
                cancel,
            )
            .await
    }
}

fn unexpected() -> RemoteError {
    RemoteError::new(ErrorCode::Internal, "comando entregue ao handler errado")
}
