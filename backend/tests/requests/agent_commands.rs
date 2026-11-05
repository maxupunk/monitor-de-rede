//! O executor do agente com os comandos da área Rede (ADR 017,
//! `AgentCommandHandler`): amostras e saída do equipamento em chunk, um teste
//! de vazão por vez. Moram aqui porque juntam o executor (pacote raiz) aos
//! handlers e aos servidores de mentira do crate `network`.

use std::{sync::Arc, time::Duration};

use agent_protocol::{
    device_io::{self, DeviceIoCall, DeviceIoReply},
    policy::{Permission, Policy},
    throughput::{TestDirection, TestProtocol, ThroughputEngine, ThroughputSpec},
    Command, ErrorCode,
};
use async_trait::async_trait;
use backend::services::agent_runtime::executor::{CommandExecutor, LocalExecutor};
use netmonitor_core::{
    agents::{command::AgentCommandHandlers, targets::DeviceNetworks},
    monitoring::CheckDeps,
    shared::errors::AppResult,
};
use netmonitor_network::{
    agent_commands,
    plugins::transport::{DeviceTransport, LineSink},
    throughput::{
        iperf3::fake_server::{Behavior, FakeServer},
        Flow, ThroughputReport, ThroughputSample, CANCELLED_MESSAGE,
    },
};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

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

/// Equipamento falso que entrega a saída aos pedaços.
struct StreamingDevice;

#[async_trait]
impl DeviceTransport for StreamingDevice {
    async fn execute(&self, _call: &DeviceIoCall) -> AppResult<DeviceIoReply> {
        Ok(DeviceIoReply::Exec {
            stdout: "tx: 1Mbps\ntx: 2Mbps\n".into(),
            stderr: String::new(),
            exit_code: Some(0),
            host_key: None,
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
    .with_handlers(AgentCommandHandlers::new(agent_commands::handlers(
        CheckDeps::default(),
        DeviceNetworks::default(),
        Some(Arc::new(StreamingDevice)),
    )));
    let (chunks, mut received) = mpsc::unbounded_channel();
    let value = executor
        .execute(
            Command::DeviceIo {
                call: DeviceIoCall::SshExec {
                    endpoint: device_io::Endpoint {
                        host: "192.168.88.1".into(),
                        port: 22,
                    },
                    login: device_io::Login::with_password("admin", None),
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
