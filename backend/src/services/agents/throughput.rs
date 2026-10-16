//! Um agente remoto como origem de teste de vazão (ADR 015 §6).
//!
//! [`AgentDriver`] implementa o mesmo [`ThroughputDriver`] do `LocalDriver`:
//! o orquestrador não sabe de onde o teste parte. O pedido vai como
//! `Command::Throughput` pelo canal que o agente mantém — a central nunca o
//! chama, e o agente só **dirige** (cliente), nunca serve. As amostras chegam
//! em `Chunk` e vão para o `SampleSink`; cancelar manda `Cancel`.
//!
//! O agente decide: `throughput` precisa estar no `AGENT_ALLOW` do host e o
//! alvo, numa rede que ele aceite. As checagens daqui só evitam um pedido que
//! seria recusado.

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use loco_rs::app::AppContext;
use tokio_util::sync::CancellationToken;

use super::{
    hub::AgentHub,
    policy::Permission,
    protocol::{Command, ErrorCode, RemoteError},
    session::AgentSession,
};
use crate::services::{
    network_tools::throughput::{
        cancelled,
        driver::{DriveRequest, ThroughputDriver},
        SampleSink, ThroughputReport, ThroughputSample, DEADLINE_MARGIN,
    },
    plugins::transport::is_reachable_target,
    shared::errors::{AppError, AppResult},
};

/// Folga sobre o prazo dos motores para a volta pelo canal.
const GRACE: Duration = Duration::from_secs(10);

/// Dirige o teste a partir de um agente conectado.
pub struct AgentDriver {
    session: Arc<AgentSession>,
}

impl AgentDriver {
    /// # Errors
    ///
    /// O agente não anunciou `throughput` na política.
    pub fn new(session: Arc<AgentSession>) -> AppResult<Self> {
        if !session.allows(Permission::Throughput) {
            return Err(not_allowed(&session.name));
        }
        Ok(Self { session })
    }

    /// Erro do agente com a mensagem que a tela mostra. Regra de negócio do
    /// motor volta com o texto exato (EC-SRP5 é reconhecido por ele).
    fn map_error(&self, error: RemoteError) -> AppError {
        let name = &self.session.name;
        match error.code {
            ErrorCode::Disconnected => AppError::service_unavailable(format!(
                "O agente {name} se desconectou durante o teste de vazão"
            )),
            ErrorCode::Timeout => AppError::service_unavailable(format!(
                "O agente {name} não concluiu o teste de vazão no prazo"
            )),
            ErrorCode::Forbidden if !self.session.allows(Permission::Throughput) => {
                not_allowed(name)
            }
            ErrorCode::Forbidden => {
                AppError::business_rule(format!("O agente {name} recusou o teste: {error}"))
            }
            ErrorCode::Unavailable => AppError::service_unavailable(error.message),
            ErrorCode::Conflict => AppError::conflict(error.message),
            ErrorCode::Validation => AppError::validation(error.message),
            ErrorCode::NotFound => AppError::not_found(error.message),
            ErrorCode::Engine => AppError::business_rule(error.message),
            ErrorCode::Disabled | ErrorCode::Unsupported | ErrorCode::Internal => {
                AppError::business_rule(format!("Agente {name}: {error}"))
            }
        }
    }
}

fn not_allowed(name: &str) -> AppError {
    AppError::business_rule(format!(
        "O agente {name} não permite testes de vazão: adicione `throughput` ao AGENT_ALLOW \
         daquele host"
    ))
}

/// Prazo do pedido inteiro: cada sentido do iperf3 é uma fase com o próprio
/// prazo (`duração + margem`), mais a volta pelo canal.
fn request_timeout(request: &DriveRequest) -> Duration {
    let direction = request.spec.direction;
    let phases = u32::from(direction.sends()) + u32::from(direction.receives());
    (request.spec.duration() + DEADLINE_MARGIN) * phases.max(1) + GRACE
}

#[async_trait]
impl ThroughputDriver for AgentDriver {
    async fn drive(
        &self,
        request: &DriveRequest,
        sink: &SampleSink,
        cancel: &CancellationToken,
    ) -> AppResult<ThroughputReport> {
        request.spec.validate()?;
        let ip = request.target.ip();
        if !is_reachable_target(ip) {
            return Err(AppError::validation(format!(
                "Um agente só testa até equipamentos de rede privada: {ip} é endereço público"
            )));
        }
        let result = self
            .session
            .stream(
                Command::from(request),
                request_timeout(request),
                cancel,
                |chunk| {
                    if let Ok(sample) = serde_json::from_value::<ThroughputSample>(chunk) {
                        let _ = sink.send(sample);
                    }
                },
            )
            .await;
        if cancel.is_cancelled() {
            return Err(cancelled());
        }
        let value = result.map_err(|error| self.map_error(error))?;
        serde_json::from_value(value).map_err(|error| {
            AppError::Internal(anyhow::anyhow!("relatório inválido do agente: {error}"))
        })
    }

    fn origin(&self) -> String {
        format!("agente {}", self.session.name)
    }
}

/// O agente `probe_id` como origem de teste de vazão.
///
/// # Errors
///
/// Agente desconectado (`service_unavailable`) ou sem `throughput` no
/// `AGENT_ALLOW`.
pub fn agent_driver(ctx: &AppContext, probe_id: i64) -> AppResult<Arc<dyn ThroughputDriver>> {
    let hub = AgentHub::from_context(ctx).map_err(|_| disconnected(probe_id))?;
    from_hub(&hub, probe_id)
}

fn from_hub(hub: &AgentHub, probe_id: i64) -> AppResult<Arc<dyn ThroughputDriver>> {
    let session = hub
        .get(probe_id)
        .filter(|session| !session.is_closed())
        .ok_or_else(|| disconnected(probe_id))?;
    Ok(Arc::new(AgentDriver::new(session)?))
}

fn disconnected(probe_id: i64) -> AppError {
    AppError::service_unavailable(format!(
        "O agente #{probe_id} não está conectado à central: o teste de vazão parte dele"
    ))
}

#[cfg(test)]
mod tests {
    use tokio::{sync::mpsc, task::JoinHandle};

    use super::*;
    use crate::services::{
        agent_runtime::{
            executor::LocalExecutor,
            outbox::Outbox,
            session::{self as agent_session, NoLiveSource, SessionDeps},
            targets::DeviceNetworks,
            tunnel::LocalTunnelOpener,
        },
        agents::{
            policy::Policy,
            protocol::{Body, Envelope, Welcome, WireFrame},
            session::tests::hello,
        },
        monitoring::runner::CheckDeps,
        network_tools::throughput::{
            iperf3::fake_server::{Behavior, FakeServer},
            is_cancelled, sample_channel, Flow, TestDirection, TestProtocol, ThroughputEngine,
            ThroughputSpec,
        },
    };

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
            executor: Arc::new(
                LocalExecutor::new(policy.clone(), CheckDeps::default())
                    .with_device_networks(networks),
            ),
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
}
