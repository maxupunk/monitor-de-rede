//! O agente remoto como origem de teste de vazão (ADR 015 §6), com a central e
//! o agente ligados em memória: a `AgentSession` da central e o
//! `agent_runtime::session::run` do agente com o executor de verdade.

use tokio::{sync::mpsc, task::JoinHandle};

use std::{sync::Arc, time::Duration};

use agent_protocol::{
    policy::{Permission, Policy},
    Body, Envelope, ErrorCode, RemoteError, Welcome, WireFrame,
};
use backend::services::agent_runtime::{
    executor::LocalExecutor,
    outbox::Outbox,
    session::{self as agent_session, NoLiveSource, SessionDeps},
    tunnel::LocalTunnelOpener,
};
use netmonitor_core::{
    agents::{
        hub::AgentHub,
        session::{testing::hello, AgentSession},
        targets::DeviceNetworks,
    },
    monitoring::CheckDeps,
    shared::errors::AppError,
};
use netmonitor_network::throughput::{
    agent::{from_hub, request_timeout, AgentDriver},
    driver::{DriveRequest, ThroughputDriver},
    iperf3::fake_server::{Behavior, FakeServer},
    is_cancelled, sample_channel, Flow, TestDirection, TestProtocol, ThroughputEngine,
    ThroughputSpec,
};
use tokio_util::sync::CancellationToken;

/// Central e agente ligados em memória: a `AgentSession` da central e o
/// `agent_runtime::session::run` do agente com o executor de verdade.
struct Linked {
    session: Arc<AgentSession>,
    tasks: Vec<JoinHandle<()>>,
}

impl Drop for Linked {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

async fn linked(permissions: &[Permission]) -> Linked {
    linked_with(permissions, DeviceNetworks::default()).await
}

async fn linked_with(permissions: &[Permission], networks: DeviceNetworks) -> Linked {
    let policy = Policy::from_permissions(permissions.iter().copied());
    let (central_out, mut to_wire) = mpsc::channel::<Envelope>(64);
    let (to_agent, agent_in) = mpsc::channel::<WireFrame>(64);
    let (agent_out, mut from_agent) = mpsc::channel::<WireFrame>(64);
    let session = Arc::new(AgentSession::new(
        7,
        "filial",
        hello(policy.permissions()),
        central_out,
    ));
    let deps = SessionDeps {
        executor: Arc::new(LocalExecutor::new(
            policy.clone(),
            CheckDeps::default(),
            backend::registries::agent_command_handlers(&CheckDeps::default(), networks),
        )),
        outbox: Arc::new(Outbox::in_memory(10)),
        live: Arc::new(NoLiveSource),
        tunnels: Arc::new(LocalTunnelOpener::new(policy.clone(), Vec::new())),
    };
    let agent_hello = hello(policy.permissions());
    let agent = tokio::spawn(async move {
        let _ = agent_session::run(deps, agent_hello, agent_in, agent_out).await;
    });
    assert!(matches!(from_agent.recv().await, Some(WireFrame::Text(_))));
    let welcome = Envelope::new(Body::Welcome(Welcome {
        probe_id: 7,
        name: "filial".into(),
        live_interval_ms: 0,
    }));
    to_agent
        .send(WireFrame::Text(welcome.encode().unwrap()))
        .await
        .unwrap();
    let outbound = tokio::spawn(async move {
        while let Some(envelope) = to_wire.recv().await {
            let text = envelope.encode().unwrap();
            if to_agent.send(WireFrame::Text(text)).await.is_err() {
                break;
            }
        }
    });
    let inbound = {
        let session = session.clone();
        tokio::spawn(async move {
            while let Some(frame) = from_agent.recv().await {
                if let WireFrame::Text(text) = frame {
                    session.handle(Envelope::decode(&text).unwrap());
                }
            }
        })
    };
    Linked {
        session,
        tasks: vec![agent, outbound, inbound],
    }
}

fn request(target: std::net::SocketAddr, duration_s: u8) -> DriveRequest {
    DriveRequest {
        target,
        spec: ThroughputSpec {
            engine: ThroughputEngine::Iperf3,
            direction: TestDirection::Receive,
            protocol: TestProtocol::Tcp,
            duration_s,
            streams: 1,
            limit_mbps: None,
        },
        auth: None,
    }
}

#[tokio::test]
async fn teste_pelo_agente_traz_amostras_e_relatorio() {
    let server = FakeServer::start(Behavior::Normal).await;
    let link = linked(&[Permission::Throughput]).await;
    let driver = AgentDriver::new(link.session.clone()).expect("throughput");
    assert_eq!(driver.origin(), "agente filial");
    let (sink, mut samples) = sample_channel();
    let report = tokio::time::timeout(
        Duration::from_secs(5),
        driver.drive(&request(server.addr, 1), &sink, &CancellationToken::new()),
    )
    .await
    .expect("dentro de 5 s")
    .expect("relatório");
    assert!(server.last_test().await.reverse, "o agente recebeu");
    assert!(report.rx_bytes > 0);
    assert_eq!(report.retransmits, Some(7));
    let first = samples.try_recv().expect("amostra no sink");
    assert_eq!(first.flow, Flow::Rx);
    assert_eq!(Some(&first), report.samples.first());
    assert_eq!(link.session.pending_count(), 0);
}

#[tokio::test]
async fn cancelar_na_central_para_o_teste_no_agente() {
    let server = FakeServer::start(Behavior::Normal).await;
    let link = linked(&[Permission::Throughput]).await;
    let driver = Arc::new(AgentDriver::new(link.session.clone()).expect("throughput"));
    let cancel = CancellationToken::new();
    let (sink, mut samples) = sample_channel();
    let running = {
        let driver = driver.clone();
        let cancel = cancel.clone();
        let request = request(server.addr, 3);
        tokio::spawn(async move { driver.drive(&request, &sink, &cancel).await })
    };
    tokio::time::timeout(Duration::from_secs(5), samples.recv())
        .await
        .expect("amostra dentro de 5 s")
        .expect("amostra");
    cancel.cancel();
    let error = tokio::time::timeout(Duration::from_secs(5), running)
        .await
        .expect("parou dentro de 5 s")
        .expect("tarefa")
        .expect_err("cancelado");
    assert!(is_cancelled(&error), "{error}");
    assert!(
        server.client_terminated().await,
        "o Cancel chegou ao agente e ele encerrou o teste"
    );
}

#[tokio::test]
async fn queda_do_agente_vira_indisponibilidade_clara() {
    let server = FakeServer::start(Behavior::Normal).await;
    let link = linked(&[Permission::Throughput]).await;
    let driver = Arc::new(AgentDriver::new(link.session.clone()).expect("throughput"));
    let (sink, mut samples) = sample_channel();
    let running = {
        let driver = driver.clone();
        let request = request(server.addr, 3);
        tokio::spawn(async move {
            driver
                .drive(&request, &sink, &CancellationToken::new())
                .await
        })
    };
    tokio::time::timeout(Duration::from_secs(5), samples.recv())
        .await
        .expect("amostra dentro de 5 s")
        .expect("amostra");
    link.session.close();
    let error = tokio::time::timeout(Duration::from_secs(5), running)
        .await
        .expect("dentro de 5 s")
        .expect("tarefa")
        .expect_err("desconectado");
    assert!(matches!(error, AppError::ServiceUnavailable(_)));
    assert!(error.to_string().contains("se desconectou"), "{error}");
}

#[tokio::test]
async fn sem_permissao_ou_alvo_publico_nada_e_enviado() {
    let link = linked(&[Permission::Read]).await;
    let error = AgentDriver::new(link.session.clone())
        .err()
        .expect("sem throughput");
    let message = error.to_string();
    assert!(
        message.contains("`throughput`") && message.contains("AGENT_ALLOW"),
        "{message}"
    );

    let link = linked(&[Permission::Throughput]).await;
    let driver = AgentDriver::new(link.session.clone()).expect("throughput");
    let (sink, _) = sample_channel();
    let error = driver
        .drive(
            &request("8.8.8.8:5201".parse().unwrap(), 1),
            &sink,
            &CancellationToken::new(),
        )
        .await
        .expect_err("público");
    assert!(matches!(error, AppError::Validation(_)));
    assert_eq!(link.session.pending_count(), 0);
}

#[tokio::test]
async fn recusa_do_agente_volta_com_a_mensagem_dele() {
    // 127.0.0.1 passa na central, mas o host só aceita 192.168.10.0/24.
    let link = linked_with(
        &[Permission::Throughput],
        DeviceNetworks::parse("192.168.10.0/24"),
    )
    .await;
    let driver = AgentDriver::new(link.session.clone()).expect("throughput");
    let (sink, _) = sample_channel();
    let error = driver
        .drive(
            &request("127.0.0.1:5201".parse().unwrap(), 1),
            &sink,
            &CancellationToken::new(),
        )
        .await
        .expect_err("fora de AGENT_DEVICE_CIDRS");
    assert!(matches!(error, AppError::BusinessRule(_)));
    assert!(error.to_string().contains("fora das redes"), "{error}");

    let busy = driver.map_error(RemoteError::new(ErrorCode::Conflict, "ocupado"));
    assert!(matches!(busy, AppError::Conflict(_)));
    let engine = driver.map_error(RemoteError::new(ErrorCode::Engine, "EC-SRP5"));
    assert!(matches!(engine, AppError::BusinessRule(ref msg) if msg == "EC-SRP5"));
    let timeout = driver.map_error(RemoteError::timeout());
    assert!(timeout.to_string().contains("no prazo"), "{timeout}");
}

#[tokio::test]
async fn agente_desconectado_nao_vira_origem() {
    let hub = AgentHub::default();
    let error = from_hub(&hub, 9).err().expect("desconectado");
    assert!(matches!(error, AppError::ServiceUnavailable(_)));

    let link = linked(&[Permission::Throughput]).await;
    hub.register(link.session.clone());
    assert_eq!(
        from_hub(&hub, 7).expect("conectado").origin(),
        "agente filial"
    );
}

#[test]
fn prazo_cobre_as_duas_fases() {
    let mut both = request("10.0.0.1:5201".parse().unwrap(), 5);
    both.spec.direction = TestDirection::Both;
    assert_eq!(request_timeout(&both), Duration::from_secs(2 * 20 + 10));
    let one = request("10.0.0.1:5201".parse().unwrap(), 5);
    assert_eq!(request_timeout(&one), Duration::from_secs(20 + 10));
}
