//! Execução dos pedidos da central, no host do agente.
//!
//! É aqui que a política local é aplicada — a cada pedido, antes de qualquer
//! efeito (ADR 011). As operações usam o **mesmo** código da central:
//! `LocalEngine` para o Docker (compose incluído), `run_monitor_with`/
//! `scan_network_with` para as checagens, o `LocalTransport` para os
//! equipamentos e o `LocalDriver` para os testes de vazão.

use std::sync::Arc;

use async_trait::async_trait;
use serde::Serialize;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::services::{
    agents::{
        policy::Policy,
        protocol::{Command, DockerCall, ErrorCode, RemoteError},
    },
    discovery::service::scan_network_with,
    docker::{
        engine::ContainerAction,
        maintenance::{DockerMaintenance, Progress},
        protection::{is_current_container, is_protected_inspect},
        source::{DockerEngine, LocalEngine, LogChunk},
        DockerError,
    },
    monitoring::runner::{run_monitor_with, CheckDeps, RunOptions},
    plugins::transport::{local::LocalTransport, DeviceIoCall, DeviceTransport},
    probes::agent::failed_result,
};

use super::{
    targets::DeviceNetworks,
    throughput::{drive_request, ThroughputRunner},
};

/// Contrato do executor: a sessão do agente só conhece isto.
#[async_trait]
pub trait CommandExecutor: Send + Sync {
    /// Executa o pedido. Linhas intermediárias (progresso, log) vão em
    /// `chunks`; o valor final é a resposta.
    async fn execute(
        &self,
        command: Command,
        chunks: mpsc::UnboundedSender<Value>,
        cancel: CancellationToken,
    ) -> Result<Value, RemoteError>;
}

fn to_value<T: Serialize>(value: T) -> Result<Value, RemoteError> {
    serde_json::to_value(value)
        .map_err(|error| RemoteError::new(ErrorCode::Internal, error.to_string()))
}

fn docker<T: Serialize>(result: Result<T, DockerError>) -> Result<Value, RemoteError> {
    result.map_err(RemoteError::from).and_then(to_value)
}

/// Linhas de progresso de texto viram chunks de string.
fn progress_to_chunks(
    chunks: mpsc::UnboundedSender<Value>,
) -> (Progress, tokio::task::JoinHandle<()>) {
    let (progress, mut lines) = mpsc::unbounded_channel::<String>();
    let forward = tokio::spawn(async move {
        while let Some(line) = lines.recv().await {
            if chunks.send(Value::String(line)).is_err() {
                break;
            }
        }
    });
    (progress, forward)
}

pub struct LocalExecutor {
    policy: Policy,
    engine: Arc<LocalEngine>,
    deps: CheckDeps,
    /// Alvos de equipamento aceitos (`AGENT_DEVICE_CIDRS`).
    networks: DeviceNetworks,
    /// Transporte fixo de equipamento (testes); `None` = um `LocalTransport`
    /// por chamada.
    device: Option<Arc<dyn DeviceTransport>>,
    throughput: ThroughputRunner,
}

impl LocalExecutor {
    #[must_use]
    pub fn new(policy: Policy, deps: CheckDeps) -> Self {
        Self {
            policy,
            engine: Arc::new(LocalEngine),
            deps,
            networks: DeviceNetworks::from_env(),
            device: None,
            throughput: ThroughputRunner::default(),
        }
    }

    /// Troca as redes aceitas como alvo (o padrão vem do ambiente).
    #[must_use]
    pub fn with_device_networks(mut self, networks: DeviceNetworks) -> Self {
        self.networks = networks;
        self
    }

    /// Fixa o transporte de equipamento.
    #[must_use]
    pub fn with_device_transport(mut self, device: Arc<dyn DeviceTransport>) -> Self {
        self.device = Some(device);
        self
    }

    /// Uma operação de equipamento. Cada linha da saída vai num chunk de
    /// texto enquanto o comando roda; a resposta é a completa.
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

    /// Nenhuma mutação atinge o container que executa o próprio agente nem
    /// um container protegido da instalação (a central, ao lado do agente
    /// local): derrubá-los cortaria a central do host sem volta (ADR 014).
    async fn guard(&self, id: &str) -> Result<(), RemoteError> {
        let inspected = self
            .engine
            .inspect_container_raw(id)
            .await
            .map_err(RemoteError::from)?;
        let full_id = inspected.get("Id").and_then(Value::as_str).unwrap_or(id);
        if is_current_container(full_id) {
            return Err(RemoteError::new(
                ErrorCode::Forbidden,
                "O agente não executa ações sobre o próprio container",
            ));
        }
        if is_protected_inspect(&inspected) {
            return Err(RemoteError::new(
                ErrorCode::Forbidden,
                "Container protegido da instalação NetMonitor: a ação derrubaria a central",
            ));
        }
        Ok(())
    }

    /// Operação longa: o progresso vira chunks de texto até o fim.
    async fn long<T, F>(
        chunks: mpsc::UnboundedSender<Value>,
        operation: impl FnOnce(Progress) -> F,
    ) -> Result<Value, RemoteError>
    where
        T: Serialize,
        F: std::future::Future<Output = Result<T, DockerError>>,
    {
        let (progress, forward) = progress_to_chunks(chunks);
        let result = operation(progress).await;
        let _ = forward.await;
        docker(result)
    }

    async fn follow(
        &self,
        id: &str,
        tail: &str,
        chunks: mpsc::UnboundedSender<Value>,
        cancel: CancellationToken,
    ) -> Result<Value, RemoteError> {
        let (logs, mut received) = mpsc::unbounded_channel::<LogChunk>();
        let forward = tokio::spawn(async move {
            while let Some(chunk) = received.recv().await {
                let Ok(value) = serde_json::to_value(chunk) else {
                    continue;
                };
                if chunks.send(value).is_err() {
                    break;
                }
            }
        });
        let result = self.engine.follow_logs(id, tail, logs, cancel).await;
        let _ = forward.await;
        docker(result.map(|()| Value::Null))
    }

    #[allow(clippy::too_many_lines)]
    async fn docker(
        &self,
        call: DockerCall,
        chunks: mpsc::UnboundedSender<Value>,
        cancel: CancellationToken,
    ) -> Result<Value, RemoteError> {
        let engine = self.engine.as_ref();
        match call {
            DockerCall::Status => docker(engine.status_raw().await),
            DockerCall::ListContainers => docker(engine.list_containers_raw().await),
            DockerCall::InspectContainer { id } => docker(engine.inspect_container_raw(&id).await),
            DockerCall::ContainerAction { id, action } => {
                if action != ContainerAction::Start {
                    self.guard(&id).await?;
                }
                docker(engine.container_action(&id, action).await)
            }
            DockerCall::Logs {
                id,
                filters,
                max_lines,
            } => docker(
                engine
                    .container_logs_raw(&id, &filters, max_lines.min(10_000))
                    .await,
            ),
            DockerCall::FollowLogs { id, tail } => self.follow(&id, &tail, chunks, cancel).await,
            DockerCall::ClearLogs { id } => docker(engine.clear_logs(&id).await),
            DockerCall::Metrics => to_value(engine.metrics().await),
            DockerCall::ListVolumes => docker(engine.list_volumes_raw().await),
            DockerCall::InspectVolume { name } => docker(engine.inspect_volume_raw(&name).await),
            DockerCall::RemoveVolume { name, force } => {
                docker(engine.remove_volume(&name, force).await)
            }
            DockerCall::ListNetworks => docker(engine.list_networks_raw().await),
            DockerCall::InspectNetwork { id } => docker(engine.inspect_network_raw(&id).await),
            DockerCall::CreateNetwork { name, driver } => {
                docker(engine.create_network(&name, &driver).await)
            }
            DockerCall::RemoveNetwork { id } => docker(engine.remove_network(&id).await),
            DockerCall::ConnectNetwork { network, container } => {
                docker(engine.connect_network(&network, &container).await)
            }
            DockerCall::DisconnectNetwork {
                network,
                container,
                force,
            } => {
                self.guard(&container).await?;
                docker(engine.disconnect_network(&network, &container, force).await)
            }
            DockerCall::ListImages => docker(engine.list_images_raw().await),
            DockerCall::InspectImage { id } => docker(engine.inspect_image_raw(&id).await),
            DockerCall::RemoveImage { id, force } => docker(engine.remove_image(&id, force).await),
            DockerCall::PruneImages => docker(engine.prune_images_raw().await),
            DockerCall::PullImage { image } => {
                Self::long(chunks, |progress| async move {
                    engine.pull_image(&image, progress).await
                })
                .await
            }
            DockerCall::UpdateContainer { id } => {
                self.guard(&id).await?;
                Self::long(chunks, |progress| async move {
                    engine.update_container(&id, progress).await
                })
                .await
            }
            DockerCall::Compose { request } => {
                Self::long(chunks, |progress| async move {
                    engine.compose(&request, progress).await
                })
                .await
            }
        }
    }
}

#[async_trait]
impl CommandExecutor for LocalExecutor {
    async fn execute(
        &self,
        command: Command,
        chunks: mpsc::UnboundedSender<Value>,
        cancel: CancellationToken,
    ) -> Result<Value, RemoteError> {
        let permission = command.permission();
        if !self.policy.allows(permission) {
            return Err(RemoteError::new(
                ErrorCode::Forbidden,
                format!("A política deste host não libera a permissão '{permission}'"),
            ));
        }
        match command {
            Command::Docker { call } => self.docker(call, chunks, cancel).await,
            Command::Monitor { task } => {
                let result = run_monitor_with(
                    &self.deps,
                    &task.task_type,
                    &task.payload,
                    RunOptions {
                        timeout_ms: u64::try_from(task.timeout_ms).ok(),
                    },
                )
                .await
                .unwrap_or_else(|error| failed_result(&error.to_string()));
                to_value(result)
            }
            Command::Discovery { task } => {
                let hosts = scan_network_with(&self.deps, &task.cidr, cancel)
                    .await
                    .map_err(|error| RemoteError::new(ErrorCode::Internal, error.to_string()))?;
                to_value(hosts)
            }
            // O modo ao vivo é da sessão, não do executor.
            Command::SetLive { .. } => Ok(Value::Null),
            Command::DeviceIo { call } => self.device_io(&call, chunks).await,
            Command::Throughput {
                target,
                port,
                spec,
                auth,
            } => {
                self.throughput
                    .run(
                        &self.networks,
                        drive_request(target, port, spec, auth),
                        chunks,
                        cancel,
                    )
                    .await
            }
            // A ponte precisa dos quadros binários do canal: é da sessão.
            Command::DatabaseTunnel { .. } | Command::VolumeExport { .. } => Err(RemoteError::new(
                ErrorCode::Unsupported,
                "pontes são atendidas pela sessão do agente",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::services::{
        agents::{policy::Permission, protocol::DockerCall},
        network_tools::throughput::{
            iperf3::fake_server::{Behavior, FakeServer},
            Flow, TestDirection, TestProtocol, ThroughputEngine, ThroughputReport,
            ThroughputSample, ThroughputSpec, CANCELLED_MESSAGE,
        },
        plugins::transport::{self as device_io, DeviceIoReply, LineSink},
        probes::dispatcher::ProbeTask,
        shared::errors::AppResult,
    };

    fn vazao() -> Policy {
        Policy::from_permissions([Permission::Throughput])
    }

    fn iperf3(target: std::net::SocketAddr, duration_s: u8) -> Command {
        Command::Throughput {
            target: target.ip(),
            port: target.port(),
            spec: ThroughputSpec {
                engine: ThroughputEngine::Iperf3,
                direction: TestDirection::Send,
                protocol: TestProtocol::Tcp,
                duration_s,
                streams: 1,
                limit_mbps: Some(200),
            },
            auth: None,
        }
    }

    #[tokio::test]
    async fn vazao_manda_amostras_em_chunk_e_o_relatorio_na_resposta() {
        let server = FakeServer::start(Behavior::Normal).await;
        let executor = LocalExecutor::new(vazao(), CheckDeps::default());
        let (chunks, mut received) = mpsc::unbounded_channel();
        let value = tokio::time::timeout(
            Duration::from_secs(5),
            executor.execute(iperf3(server.addr, 1), chunks, CancellationToken::new()),
        )
        .await
        .expect("dentro de 5 s")
        .expect("relatório");
        let report: ThroughputReport = serde_json::from_value(value).expect("relatório");
        assert_eq!(report.engine, ThroughputEngine::Iperf3);
        assert!(report.tx_bytes > 0);
        let mut samples = Vec::new();
        while let Ok(chunk) = received.try_recv() {
            samples.push(serde_json::from_value::<ThroughputSample>(chunk).expect("amostra"));
        }
        assert!(!samples.is_empty(), "amostras em chunk");
        assert!(samples.iter().all(|sample| sample.flow == Flow::Tx));
        assert_eq!(samples, report.samples, "as mesmas do relatório");
    }

    #[tokio::test]
    async fn vazao_cancelada_para_e_libera_a_vaga_e_um_teste_por_vez() {
        let server = FakeServer::start(Behavior::Normal).await;
        let executor = Arc::new(LocalExecutor::new(vazao(), CheckDeps::default()));
        let cancel = CancellationToken::new();
        let (chunks, mut received) = mpsc::unbounded_channel();
        let first = {
            let executor = executor.clone();
            let cancel = cancel.clone();
            let addr = server.addr;
            tokio::spawn(async move { executor.execute(iperf3(addr, 3), chunks, cancel).await })
        };
        // A primeira amostra prova que o teste está rodando.
        tokio::time::timeout(Duration::from_secs(5), received.recv())
            .await
            .expect("amostra dentro de 5 s")
            .expect("amostra");

        let (chunks, _) = mpsc::unbounded_channel();
        let busy = executor
            .execute(iperf3(server.addr, 1), chunks, CancellationToken::new())
            .await
            .expect_err("ocupado");
        assert_eq!(busy.code, ErrorCode::Conflict);

        cancel.cancel();
        let error = tokio::time::timeout(Duration::from_secs(5), first)
            .await
            .expect("parou dentro de 5 s")
            .expect("tarefa")
            .expect_err("cancelado");
        assert_eq!(error.message, CANCELLED_MESSAGE);
        assert!(server.client_terminated().await, "o servidor foi avisado");

        let (chunks, _) = mpsc::unbounded_channel();
        let again = tokio::time::timeout(
            Duration::from_secs(5),
            executor.execute(iperf3(server.addr, 1), chunks, CancellationToken::new()),
        )
        .await
        .expect("dentro de 5 s");
        assert!(again.is_ok(), "a vaga voltou: {again:?}");
    }

    #[tokio::test]
    async fn vazao_exige_permissao_e_alvo_permitido() {
        let local: std::net::SocketAddr = "127.0.0.1:5201".parse().unwrap();
        let negado = LocalExecutor::new(Policy::default(), CheckDeps::default());
        let (chunks, _) = mpsc::unbounded_channel();
        let error = negado
            .execute(iperf3(local, 1), chunks, CancellationToken::new())
            .await
            .expect_err("sem throughput");
        assert_eq!(error.code, ErrorCode::Forbidden);

        let liberado = LocalExecutor::new(vazao(), CheckDeps::default());
        let (chunks, _) = mpsc::unbounded_channel();
        let error = liberado
            .execute(
                iperf3("8.8.8.8:5201".parse().unwrap(), 1),
                chunks,
                CancellationToken::new(),
            )
            .await
            .expect_err("IP público");
        assert_eq!(error.code, ErrorCode::Forbidden);

        let restrito = LocalExecutor::new(vazao(), CheckDeps::default())
            .with_device_networks(DeviceNetworks::parse("192.168.10.0/24"));
        let (chunks, _) = mpsc::unbounded_channel();
        let error = restrito
            .execute(iperf3(local, 1), chunks, CancellationToken::new())
            .await
            .expect_err("fora de AGENT_DEVICE_CIDRS");
        assert_eq!(error.code, ErrorCode::Forbidden);

        let (chunks, _) = mpsc::unbounded_channel();
        let error = liberado
            .execute(iperf3(local, 0), chunks, CancellationToken::new())
            .await
            .expect_err("duração fora do contrato");
        assert_eq!(error.code, ErrorCode::Validation);
    }

    /// Equipamento falso que entrega a saída aos pedaços.
    struct StreamingDevice;

    #[async_trait]
    impl DeviceTransport for StreamingDevice {
        async fn execute(&self, _call: &DeviceIoCall) -> AppResult<DeviceIoReply> {
            Ok(DeviceIoReply::Exec {
                stdout: "tx: 1Mbps\ntx: 2Mbps\n".into(),
                stderr: String::new(),
                exit_code: Some(0),
            })
        }

        async fn execute_streaming(
            &self,
            call: &DeviceIoCall,
            on_line: LineSink<'_>,
        ) -> AppResult<DeviceIoReply> {
            on_line("tx: 1Mbps");
            tokio::task::yield_now().await;
            on_line("tx: 2Mbps");
            self.execute(call).await
        }

        fn origin(&self) -> String {
            "teste".into()
        }
    }

    #[tokio::test]
    async fn saida_do_equipamento_vai_em_chunk_e_inteira_na_resposta() {
        let executor = LocalExecutor::new(
            Policy::from_permissions([Permission::DeviceIo]),
            CheckDeps::default(),
        )
        .with_device_transport(Arc::new(StreamingDevice));
        let (chunks, mut received) = mpsc::unbounded_channel();
        let value = executor
            .execute(
                Command::DeviceIo {
                    call: DeviceIoCall::SshExec {
                        endpoint: device_io::Endpoint {
                            host: "192.168.88.1".into(),
                            port: 22,
                        },
                        login: device_io::Login {
                            username: "admin".into(),
                            password: None,
                        },
                        command: "/tool bandwidth-test".into(),
                        timeout_ms: 1_000,
                    },
                },
                chunks,
                CancellationToken::new(),
            )
            .await
            .expect("resposta");
        assert_eq!(received.try_recv().ok(), Some(Value::from("tx: 1Mbps")));
        assert_eq!(received.try_recv().ok(), Some(Value::from("tx: 2Mbps")));
        assert!(received.try_recv().is_err(), "só as linhas da saída");
        let reply: DeviceIoReply = serde_json::from_value(value).expect("resposta");
        let DeviceIoReply::Exec { stdout, .. } = reply else {
            panic!("exec");
        };
        assert_eq!(
            stdout, "tx: 1Mbps\ntx: 2Mbps\n",
            "a resposta continua inteira"
        );
    }

    #[tokio::test]
    async fn politica_local_nega_antes_de_tocar_o_docker() {
        let executor = LocalExecutor::new(
            Policy::from_permissions([Permission::Read]),
            CheckDeps::default(),
        );
        let (chunks, _) = mpsc::unbounded_channel();
        let error = executor
            .execute(
                Command::Docker {
                    call: DockerCall::UpdateContainer { id: "web".into() },
                },
                chunks,
                CancellationToken::new(),
            )
            .await
            .expect_err("negado");
        assert_eq!(error.code, ErrorCode::Forbidden);
    }

    #[tokio::test]
    async fn acesso_a_equipamento_exige_permissao_e_rede_privada() {
        let call = |host: &str| Command::DeviceIo {
            call: DeviceIoCall::HttpRequest {
                endpoint: device_io::Endpoint {
                    host: host.into(),
                    port: 1,
                },
                https: false,
                request: device_io::HttpRequest {
                    method: "GET".into(),
                    path: "/".into(),
                    headers: vec![],
                    body: None,
                    basic_auth: None,
                },
                timeout_ms: 1_000,
            },
        };
        let negado = LocalExecutor::new(Policy::default(), CheckDeps::default());
        let (chunks, _) = mpsc::unbounded_channel();
        let error = negado
            .execute(call("127.0.0.1"), chunks, CancellationToken::new())
            .await
            .expect_err("sem device_io");
        assert_eq!(error.code, ErrorCode::Forbidden);

        let liberado = LocalExecutor::new(
            Policy::from_permissions([Permission::DeviceIo]),
            CheckDeps::default(),
        );
        let (chunks, _) = mpsc::unbounded_channel();
        let error = liberado
            .execute(call("8.8.8.8"), chunks, CancellationToken::new())
            .await
            .expect_err("IP público");
        assert_eq!(error.code, ErrorCode::Forbidden);
    }

    #[tokio::test]
    async fn monitor_executa_o_mesmo_checker_da_central() {
        let executor = LocalExecutor::new(Policy::default(), CheckDeps::default());
        let (chunks, _) = mpsc::unbounded_channel();
        let value = executor
            .execute(
                Command::Monitor {
                    task: ProbeTask {
                        id: "task-1".into(),
                        monitor_id: 1,
                        task_type: "tcp".into(),
                        timeout_ms: 1_000,
                        payload: serde_json::json!({ "host": "127.0.0.1", "port": 1 }),
                    },
                },
                chunks,
                CancellationToken::new(),
            )
            .await
            .expect("resultado");
        assert!(value.get("status").is_some());
    }

    #[tokio::test]
    async fn falha_de_execucao_vira_observacao_down() {
        let executor = LocalExecutor::new(Policy::default(), CheckDeps::default());
        let (chunks, _) = mpsc::unbounded_channel();
        let value = executor
            .execute(
                Command::Monitor {
                    task: ProbeTask {
                        id: "task-2".into(),
                        monitor_id: 2,
                        task_type: "inexistente".into(),
                        timeout_ms: 1_000,
                        payload: serde_json::json!({}),
                    },
                },
                chunks,
                CancellationToken::new(),
            )
            .await
            .expect("observação");
        assert_eq!(value["status"], "down");
    }
}
