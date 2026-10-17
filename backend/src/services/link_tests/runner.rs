//! A máquina de estados de um teste de enlace (ADR 015 §5).
//!
//! As pernas rodam **uma por vez**, em ordem; cada perna percorre as etapas
//! `prepare_a` → `prepare_b` → `serve` → `forward`/`reverse` → `stop`. O
//! `stop` roda **sempre** — sucesso, falha ou cancelamento — e o `handle` do
//! servidor que um equipamento subiu é gravado na perna **antes** de ser
//! usado: se o processo cair no meio, o boot o desfaz ([`super::janitor`]).
//!
//! Toda mudança de etapa é gravada e publicada (`link_test:updated`); as
//! amostras de 1 s saem como `link_test:sample` (no máximo duas por segundo
//! por sentido) e ficam na perna, com teto, para o `GET` de detalhe.

use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, Instant},
};

use chrono::Utc;
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

use super::{
    endpoint::{Endpoint, Role},
    env::LinkTestEnv,
    events,
    guard::Reservation,
    model::{
        as_db, LinkTestFlow, LinkTestLegKind, LinkTestSample, LinkTestSampleEvent, LinkTestSide,
        LinkTestSpec, LinkTestStage, LinkTestStageKey, LinkTestStatus, LinkTestStepStatus,
        LinkTestUserRef,
    },
    negotiate::{driver_direction, link_flow, link_rates, Negotiation},
    outcome,
    plan::PlannedLeg,
    repository, schedules, view,
};
use crate::{
    models::{link_test_legs, link_tests},
    services::{
        events::EventBus,
        network_tools::throughput::{
            btest::{self, ArmedSession, BtestAuth},
            driver::DriveRequest,
            is_cancelled, sample_channel, Flow, TestDirection, TestProtocol, ThroughputEngine,
            ThroughputSample, ThroughputSpec,
        },
        plugins::{
            invoke::{RunOutcome, RunStatus, ServiceRunRequest},
            runs::Origin,
            runtime::{ProgressFlow, ProgressSample},
        },
        shared::errors::{AppError, AppResult},
    },
};

/// Amostras guardadas por sentido de uma perna.
pub const MAX_SAMPLES_PER_FLOW: usize = 120;
/// Intervalo mínimo entre duas amostras publicadas do mesmo sentido.
const SAMPLE_PUBLISH_EVERY: Duration = Duration::from_millis(500);
/// Folga para o servidor armado fechar o relatório depois que o MikroTik
/// terminou (ou desistiu).
const ARMED_GRACE: Duration = Duration::from_secs(3);
/// Espera pelas últimas amostras depois do fim de uma medida.
const SAMPLE_DRAIN: Duration = Duration::from_secs(2);
/// Mensagem do teste cancelado.
pub const CANCELLED_TEXT: &str = "Cancelado pelo operador";

/// Limpeza pendente de uma perna: o servidor que um equipamento subiu e
/// como desfazê-lo. Gravada **antes** de o servidor ser usado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingCleanup {
    pub device_id: i64,
    pub plugin_slug: String,
    pub action: String,
    pub engine: ThroughputEngine,
    pub handle: String,
}

/// Usuário e senha de um teste btest com autenticação — gerados a cada
/// teste, nunca gravados (a senha vai como parâmetro `secret`).
#[derive(Clone)]
struct TestAuth {
    user: String,
    password: String,
}

impl TestAuth {
    fn generate() -> Self {
        let random = |len: usize| -> String {
            rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(len)
                .map(char::from)
                .collect()
        };
        Self {
            // Os plugins conferem: `^nm-test-[A-Za-z0-9_-]{1,40}$`.
            user: format!("nm-test-{}", random(12)),
            password: random(24),
        }
    }
}

/// Amostras de um teste: o que vai para a perna (com teto) e o que sai no
/// SSE (com ritmo).
struct SampleBook {
    test_id: i64,
    bus: Option<EventBus>,
    inner: Mutex<BookInner>,
}

#[derive(Default)]
struct BookInner {
    by_leg: HashMap<u32, Vec<LinkTestSample>>,
    published: HashMap<(u32, LinkTestFlow), Instant>,
}

impl SampleBook {
    fn push(&self, seq: u32, flow: LinkTestFlow, bps: f64, t_ms: u64) {
        let publish = {
            let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
            let samples = inner.by_leg.entry(seq).or_default();
            if samples.iter().filter(|sample| sample.flow == flow).count() < MAX_SAMPLES_PER_FLOW {
                samples.push(LinkTestSample { flow, bps, t_ms });
            }
            let now = Instant::now();
            let due = inner
                .published
                .get(&(seq, flow))
                .is_none_or(|last| now.duration_since(*last) >= SAMPLE_PUBLISH_EVERY);
            if due {
                inner.published.insert((seq, flow), now);
            }
            due
        };
        if publish {
            events::publish_sample(
                self.bus.as_ref(),
                LinkTestSampleEvent {
                    test_id: self.test_id,
                    leg_seq: seq,
                    flow,
                    bps,
                    t_ms,
                },
            );
        }
    }

    fn of(&self, seq: u32) -> Vec<LinkTestSample> {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .by_leg
            .get(&seq)
            .cloned()
            .unwrap_or_default()
    }
}

/// Repassa as amostras de um canal ao livro, já no sentido do enlace.
fn forward_samples<T, F>(
    mut rx: mpsc::UnboundedReceiver<T>,
    book: Arc<SampleBook>,
    seq: u32,
    map: F,
) -> JoinHandle<()>
where
    T: Send + 'static,
    F: Fn(T) -> Option<(LinkTestFlow, f64, u64)> + Send + 'static,
{
    tokio::spawn(async move {
        while let Some(sample) = rx.recv().await {
            if let Some((flow, bps, t_ms)) = map(sample) {
                book.push(seq, flow, bps, t_ms);
            }
        }
    })
}

/// Espera o repasse terminar (o canal fecha com o fim da medida), sem travar
/// se alguém segurar o emissor.
async fn drain(handle: JoinHandle<()>) {
    let abort = handle.abort_handle();
    if tokio::time::timeout(SAMPLE_DRAIN, handle).await.is_err() {
        abort.abort();
    }
}

/// Para onde quem dirige aponta.
#[derive(Debug, Clone)]
struct Target {
    host: String,
    port: u16,
}

impl Target {
    fn socket(&self) -> Result<SocketAddr, String> {
        self.host
            .trim()
            .parse::<IpAddr>()
            .map(|ip| SocketAddr::new(ip, self.port))
            .map_err(|_| format!("{} não é um endereço IP para o teste", self.host))
    }
}

/// O que a etapa `serve` deixou de pé.
enum Served {
    Nothing,
    Armed(ArmedSession),
    Device(PendingCleanup),
}

/// O que uma medida devolveu, do ponto de vista de quem dirige.
#[derive(Debug, Default, Clone, Copy)]
struct Measured {
    tx_bps: Option<f64>,
    rx_bps: Option<f64>,
    retransmits: Option<u64>,
}

/// O relatório que o `run` de um plugin devolve, lido com tolerância.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginReport {
    #[serde(default)]
    tx_bps: Option<f64>,
    #[serde(default)]
    rx_bps: Option<f64>,
    #[serde(default)]
    retransmits: Option<u64>,
}

impl From<PluginReport> for Measured {
    fn from(report: PluginReport) -> Self {
        Self {
            tx_bps: report.tx_bps,
            rx_bps: report.rx_bps,
            retransmits: report.retransmits,
        }
    }
}

/// Uma chamada de ação de plugin.
struct Call {
    device_id: i64,
    slug: String,
    action: String,
    params: Value,
    progress: Option<mpsc::UnboundedSender<ProgressSample>>,
    cancel: CancellationToken,
}

/// Falha de uma etapa: cancelada ou com motivo.
enum StageError {
    Cancelled,
    Failed(String),
}

impl StageError {
    fn from_outcome(outcome: &RunOutcome, what: &str) -> Self {
        match outcome.status {
            RunStatus::Cancelled => Self::Cancelled,
            _ => Self::Failed(
                outcome
                    .error
                    .clone()
                    .unwrap_or_else(|| format!("{what} terminou com falha")),
            ),
        }
    }

    fn from_error(error: &AppError) -> Self {
        if is_cancelled(error) {
            Self::Cancelled
        } else {
            Self::Failed(error.to_string())
        }
    }
}

/// O teste em execução.
pub struct Job {
    env: Arc<LinkTestEnv>,
    test: link_tests::Model,
    rows: Vec<link_test_legs::Model>,
    stages: Vec<Vec<LinkTestStage>>,
    plan: Vec<PlannedLeg>,
    spec: LinkTestSpec,
    requested_by: Option<LinkTestUserRef>,
    reservation: Reservation,
    book: Arc<SampleBook>,
}

impl Job {
    /// Monta o executor do teste gravado.
    #[must_use]
    pub fn new(
        env: Arc<LinkTestEnv>,
        test: link_tests::Model,
        rows: Vec<link_test_legs::Model>,
        plan: Vec<PlannedLeg>,
        spec: LinkTestSpec,
        requested_by: Option<LinkTestUserRef>,
        reservation: Reservation,
    ) -> Self {
        let stages = rows.iter().map(|_| LinkTestStage::all_pending()).collect();
        let book = Arc::new(SampleBook {
            test_id: test.id,
            bus: env.bus.clone(),
            inner: Mutex::default(),
        });
        Self {
            env,
            test,
            rows,
            stages,
            plan,
            spec,
            requested_by,
            reservation,
            book,
        }
    }

    fn cancelled(&self) -> bool {
        self.reservation.cancel.is_cancelled()
    }

    fn snapshot(&self) -> crate::services::link_tests::model::LinkTest {
        view::test(&self.test, &self.rows, self.requested_by.clone(), false)
    }

    async fn save_test(&mut self) {
        match repository::save_test(&self.env.db, &self.test).await {
            Ok(saved) => self.test = saved,
            Err(error) => {
                tracing::warn!(%error, test_id = self.test.id, "falha ao gravar o teste de enlace")
            }
        }
        events::publish_test(self.env.bus.as_ref(), &self.snapshot());
    }

    async fn persist(&mut self, idx: usize) {
        let seq = self.plan[idx].view.seq;
        let samples = self.book.of(seq);
        let row = &mut self.rows[idx];
        row.stages = json!(self.stages[idx]);
        row.samples = (!samples.is_empty()).then(|| json!(samples));
        match repository::save_leg(&self.env.db, row).await {
            Ok(saved) => *row = saved,
            Err(error) => {
                tracing::warn!(%error, test_id = self.test.id, seq, "falha ao gravar a perna do teste de enlace")
            }
        }
        events::publish_test(self.env.bus.as_ref(), &self.snapshot());
    }

    fn stage_mut(&mut self, idx: usize, key: LinkTestStageKey) -> &mut LinkTestStage {
        let stages = &mut self.stages[idx];
        let position = stages
            .iter()
            .position(|stage| stage.key == key)
            .unwrap_or_else(|| {
                stages.push(LinkTestStage::pending(key));
                stages.len() - 1
            });
        &mut stages[position]
    }

    async fn begin(&mut self, idx: usize, keys: &[LinkTestStageKey]) {
        let now = Utc::now().to_rfc3339();
        for key in keys {
            let stage = self.stage_mut(idx, *key);
            stage.status = LinkTestStepStatus::Running;
            stage.started_at = Some(now.clone());
        }
        self.persist(idx).await;
    }

    async fn end(
        &mut self,
        idx: usize,
        key: LinkTestStageKey,
        status: LinkTestStepStatus,
        message: Option<String>,
    ) {
        self.set(idx, key, status, message);
        self.persist(idx).await;
    }

    /// Fecha a etapa sem gravar (quem chama grava depois).
    fn set(
        &mut self,
        idx: usize,
        key: LinkTestStageKey,
        status: LinkTestStepStatus,
        message: Option<String>,
    ) {
        let now = Utc::now().to_rfc3339();
        let stage = self.stage_mut(idx, key);
        stage.status = status;
        stage.message = message;
        if stage.started_at.is_none() && status != LinkTestStepStatus::Skipped {
            stage.started_at = Some(now.clone());
        }
        stage.finished_at = Some(now);
    }

    async fn fail_stage(&mut self, idx: usize, key: LinkTestStageKey, error: StageError) {
        match error {
            StageError::Cancelled => {
                self.end(
                    idx,
                    key,
                    LinkTestStepStatus::Cancelled,
                    Some(CANCELLED_TEXT.to_owned()),
                )
                .await;
            }
            StageError::Failed(message) => {
                self.end(idx, key, LinkTestStepStatus::Failed, Some(message))
                    .await;
            }
        }
    }

    /// Pula (ou cancela) as etapas ainda pendentes da perna.
    fn close_pending(&mut self, idx: usize, status: LinkTestStepStatus, message: &str) {
        for stage in &mut self.stages[idx] {
            if !stage.status.is_final() {
                stage.status = status;
                stage.message = Some(message.to_owned());
                stage.finished_at = Some(Utc::now().to_rfc3339());
            }
        }
    }

    fn reason(&self, idx: usize, key: LinkTestStageKey) -> String {
        format!(
            "Teste de enlace #{}, perna {}: {}",
            self.test.id,
            self.plan[idx].view.seq,
            key.label()
        )
    }

    /// Chama a ação do plugin e grava o `runId` nas etapas assim que a
    /// execução existe.
    async fn invoke(
        &mut self,
        idx: usize,
        keys: &[LinkTestStageKey],
        call: Call,
    ) -> AppResult<RunOutcome> {
        let (started_tx, mut started_rx) = oneshot::channel();
        let plugins = Arc::clone(&self.env.plugins);
        let request = ServiceRunRequest {
            device_id: call.device_id,
            plugin_slug: call.slug,
            action: call.action,
            params: call.params,
            origin: Origin::LinkTest,
            user_id: self.test.user_id,
            reason: Some(self.reason(idx, keys[0])),
            progress: call.progress,
            cancel: Some(call.cancel),
        };
        let run = plugins.execute(request, Some(started_tx));
        tokio::pin!(run);
        let mut waiting = true;
        let result = loop {
            tokio::select! {
                result = &mut run => break result,
                id = &mut started_rx, if waiting => {
                    waiting = false;
                    if let Ok(id) = id {
                        for key in keys {
                            self.stage_mut(idx, *key).run_id = Some(id);
                        }
                        self.persist(idx).await;
                    }
                }
            }
        };
        if let Ok(outcome) = &result {
            for key in keys {
                self.stage_mut(idx, *key).run_id = Some(outcome.run_id);
            }
        }
        result
    }

    /// Executa o teste inteiro e libera a vaga no fim.
    pub async fn run(mut self) {
        self.test.status = as_db(&LinkTestStatus::Running);
        self.test.started_at = Some(Utc::now().into());
        self.save_test().await;
        self.record_schedule(LinkTestStatus::Running).await;
        for idx in 0..self.rows.len() {
            if self.cancelled() {
                self.close_pending(idx, LinkTestStepStatus::Cancelled, CANCELLED_TEXT);
                self.rows[idx].status = as_db(&LinkTestStepStatus::Cancelled);
                self.rows[idx].error = Some(CANCELLED_TEXT.to_owned());
                self.persist(idx).await;
                continue;
            }
            self.run_leg(idx).await;
        }
        self.finish().await;
    }

    async fn run_leg(&mut self, idx: usize) {
        let planned = self.plan[idx].clone();
        let Some(n) = planned
            .negotiation
            .clone()
            .filter(|_| planned.view.runnable)
        else {
            let reason = planned.view.issues.join("; ");
            self.close_pending(idx, LinkTestStepStatus::Skipped, &reason);
            self.rows[idx].status = as_db(&LinkTestStepStatus::Skipped);
            self.rows[idx].error = Some(reason);
            self.persist(idx).await;
            return;
        };
        self.rows[idx].status = as_db(&LinkTestStepStatus::Running);
        self.rows[idx].started_at = Some(Utc::now().into());
        self.persist(idx).await;

        let auth = n.require_auth.then(TestAuth::generate);
        let mut served = Served::Nothing;
        if self.prepare(idx, &planned, &n).await {
            if let Ok((now_served, target)) = self.serve(idx, &planned, &n, auth.as_ref()).await {
                served = now_served;
                self.measure(idx, &planned, &n, &mut served, &target, auth.as_ref())
                    .await;
            }
        }
        // Ninguém chegou às etapas seguintes: puladas (ou canceladas).
        let (status, message) = if self.cancelled() {
            (LinkTestStepStatus::Cancelled, CANCELLED_TEXT)
        } else {
            (
                LinkTestStepStatus::Skipped,
                "não rodou: uma etapa anterior falhou",
            )
        };
        for key in [
            LinkTestStageKey::PrepareA,
            LinkTestStageKey::PrepareB,
            LinkTestStageKey::Serve,
            LinkTestStageKey::Forward,
            LinkTestStageKey::Reverse,
        ] {
            if self.stage_mut(idx, key).status == LinkTestStepStatus::Pending {
                self.set(idx, key, status, Some(message.to_owned()));
            }
        }
        self.stop(idx, &n, served).await;

        let stages = &self.stages[idx];
        let status = outcome::leg_status(stages);
        let error = match status {
            LinkTestStepStatus::Cancelled => Some(CANCELLED_TEXT.to_owned()),
            LinkTestStepStatus::Failed => outcome::first_failure(stages),
            _ => None,
        };
        let row = &mut self.rows[idx];
        row.status = as_db(&status);
        row.error = error;
        row.finished_at = Some(Utc::now().into());
        self.persist(idx).await;
    }

    /// `prepare` de cada lado que declara a ação. `false` = a perna para.
    async fn prepare(&mut self, idx: usize, planned: &PlannedLeg, n: &Negotiation) -> bool {
        for side in [LinkTestSide::A, LinkTestSide::B] {
            let key = LinkTestStageKey::prepare_of(side);
            let endpoint = planned.endpoint(side);
            let action = endpoint
                .plugin()
                .and_then(|plugin| plugin.roles.actions.prepare.clone());
            let (Some(device_id), Some(plugin), Some(action)) =
                (endpoint.device_id(), endpoint.plugin(), action)
            else {
                self.end(
                    idx,
                    key,
                    LinkTestStepStatus::Skipped,
                    Some("nada a preparar".to_owned()),
                )
                .await;
                continue;
            };
            if self.cancelled() {
                return false;
            }
            self.begin(idx, &[key]).await;
            let role = if side == n.server { "serve" } else { "drive" };
            let call = Call {
                device_id,
                slug: plugin.slug.clone(),
                action,
                params: json!({ "engine": n.engine.as_str(), "role": role }),
                progress: None,
                cancel: self.reservation.cancel.clone(),
            };
            let result = self.invoke(idx, &[key], call).await;
            let error = match result {
                Ok(run) if run.status == RunStatus::Succeeded => {
                    let output = run.output.unwrap_or(Value::Null);
                    if output.get("ready").and_then(Value::as_bool) == Some(false) {
                        StageError::Failed(format!(
                            "{} não ficou pronto para o teste ({})",
                            endpoint.view.name,
                            n.engine.as_str()
                        ))
                    } else {
                        let installed: Vec<String> = output
                            .get("installed")
                            .and_then(Value::as_array)
                            .map(|list| {
                                list.iter()
                                    .filter_map(Value::as_str)
                                    .map(str::to_owned)
                                    .collect()
                            })
                            .unwrap_or_default();
                        let message = if installed.is_empty() {
                            "pronto".to_owned()
                        } else {
                            format!("instalado: {}", installed.join(", "))
                        };
                        self.end(idx, key, LinkTestStepStatus::Succeeded, Some(message))
                            .await;
                        continue;
                    }
                }
                Ok(run) => StageError::from_outcome(&run, "O preparo"),
                Err(error) => StageError::from_error(&error),
            };
            self.fail_stage(idx, key, error).await;
            return false;
        }
        true
    }

    /// Sobe o servidor: arma o btest da central ou chama o `serve` do
    /// equipamento (gravando o `handle` antes de seguir).
    async fn serve(
        &mut self,
        idx: usize,
        planned: &PlannedLeg,
        n: &Negotiation,
        auth: Option<&TestAuth>,
    ) -> Result<(Served, Target), ()> {
        let key = LinkTestStageKey::Serve;
        if self.cancelled() {
            return Err(());
        }
        self.begin(idx, &[key]).await;
        let server = planned.endpoint(n.server);
        let driver = planned.endpoint(n.driver());
        let result = match &server.role {
            Role::Central { .. } => self.arm_central(driver).await,
            Role::Device {
                device,
                plugin: Some(plugin),
            } => {
                let peer = match &driver.role {
                    Role::Device { .. } | Role::Agent { .. } => driver.view.ip.clone(),
                    Role::Central { .. } => self.env.central.toward(device).await.ok().flatten(),
                };
                let port = match n.engine {
                    ThroughputEngine::Iperf3 => self.reservation.port,
                    ThroughputEngine::Btest => btest::DEFAULT_PORT,
                };
                let mut params = json!({
                    "engine": n.engine.as_str(),
                    "port": port,
                    "peer": peer.unwrap_or_default(),
                    "requireAuth": n.require_auth,
                });
                if let Some(auth) = auth {
                    params["user"] = json!(auth.user);
                    params["password"] = json!(auth.password);
                }
                let serve_action = plugin.roles.actions.serve.clone().unwrap_or_default();
                let stop_action = plugin.roles.actions.stop.clone().unwrap_or_default();
                let call = Call {
                    device_id: device.id,
                    slug: plugin.slug.clone(),
                    action: serve_action,
                    params,
                    progress: None,
                    cancel: self.reservation.cancel.clone(),
                };
                match self.invoke(idx, &[key], call).await {
                    Ok(run) if run.status == RunStatus::Succeeded => {
                        let output = run.output.unwrap_or(Value::Null);
                        let port = output
                            .get("port")
                            .and_then(Value::as_u64)
                            .and_then(|port| u16::try_from(port).ok())
                            .unwrap_or(port);
                        let handle = match output.get("handle") {
                            Some(Value::String(handle)) => handle.clone(),
                            Some(Value::Null) | None => String::new(),
                            Some(other) => other.to_string(),
                        };
                        let cleanup = PendingCleanup {
                            device_id: device.id,
                            plugin_slug: plugin.slug.clone(),
                            action: stop_action,
                            engine: n.engine,
                            handle,
                        };
                        // Antes de usar o servidor: se o processo cair daqui
                        // em diante, o boot sabe o que desfazer.
                        self.rows[idx].cleanup = Some(json!(cleanup));
                        self.persist(idx).await;
                        Ok((
                            Served::Device(cleanup),
                            Target {
                                host: server.view.ip.clone().unwrap_or_default(),
                                port,
                            },
                            format!("servidor {} na porta {port}", n.engine.as_str()),
                        ))
                    }
                    Ok(run) => Err(StageError::from_outcome(&run, "O servidor")),
                    Err(error) => Err(StageError::from_error(&error)),
                }
            }
            _ => Err(StageError::Failed(format!(
                "{} não serve testes",
                server.view.name
            ))),
        };
        match result {
            Ok((served, target, message)) => {
                self.end(idx, key, LinkTestStepStatus::Succeeded, Some(message))
                    .await;
                Ok((served, target))
            }
            Err(error) => {
                self.fail_stage(idx, key, error).await;
                Err(())
            }
        }
    }

    /// Arma o servidor btest da central para o equipamento que vai discar.
    async fn arm_central(&self, driver: &Endpoint) -> Result<(Served, Target, String), StageError> {
        let (Some(peer), Some(device)) = (driver.ip(), driver.device()) else {
            return Err(StageError::Failed(format!(
                "{} precisa de um IP válido para a central aceitar a conexão btest",
                driver.view.name
            )));
        };
        let host = self
            .env
            .central
            .toward(device)
            .await
            .map_err(|error| StageError::Failed(error.to_string()))?
            .ok_or_else(|| {
                StageError::Failed(format!(
                    "Não há endereço deste servidor sugerido para {}: confira os endereços do \
                     servidor nas configurações",
                    driver.view.name
                ))
            })?;
        let session = self
            .env
            .btest
            .arm(peer, btest::DEFAULT_ARM_TTL)
            .map_err(|error| StageError::Failed(error.to_string()))?;
        Ok((
            Served::Armed(session),
            Target {
                host,
                port: self.env.btest.port(),
            },
            format!("servidor btest da central armado para {peer}"),
        ))
    }

    async fn measure(
        &mut self,
        idx: usize,
        planned: &PlannedLeg,
        n: &Negotiation,
        served: &mut Served,
        target: &Target,
        auth: Option<&TestAuth>,
    ) {
        let flows = self.spec.direction.flows();
        for flow in [LinkTestFlow::Forward, LinkTestFlow::Reverse] {
            if !flows.contains(&flow) {
                self.set(
                    idx,
                    LinkTestStageKey::of_flow(flow),
                    LinkTestStepStatus::Skipped,
                    Some("sentido não pedido".to_owned()),
                );
            }
        }
        // O btest mede os dois sentidos ao mesmo tempo, numa chamada só.
        if n.engine == ThroughputEngine::Btest && flows.len() == 2 {
            if !self.cancelled() {
                self.measure_call(
                    idx,
                    planned,
                    n,
                    served,
                    target,
                    auth,
                    TestDirection::Both,
                    &flows,
                )
                .await;
            }
            return;
        }
        for flow in flows {
            if self.cancelled() {
                return;
            }
            let direction = driver_direction(n.driver(), flow);
            self.measure_call(idx, planned, n, served, target, auth, direction, &[flow])
                .await;
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn measure_call(
        &mut self,
        idx: usize,
        planned: &PlannedLeg,
        n: &Negotiation,
        served: &mut Served,
        target: &Target,
        auth: Option<&TestAuth>,
        direction: TestDirection,
        flows: &[LinkTestFlow],
    ) {
        let keys: Vec<LinkTestStageKey> = flows
            .iter()
            .copied()
            .map(LinkTestStageKey::of_flow)
            .collect();
        self.begin(idx, &keys).await;
        let driver_side = n.driver();
        let driver = planned.endpoint(driver_side);
        let seq = planned.view.seq;
        let spec = ThroughputSpec {
            engine: n.engine,
            direction,
            protocol: TestProtocol::Tcp,
            duration_s: n.duration_s,
            // O btest fala com uma conexão TCP só.
            streams: if n.engine == ThroughputEngine::Btest {
                1
            } else {
                self.spec.streams
            },
            limit_mbps: self.spec.limit_mbps,
        };
        let result = match &driver.role {
            Role::Central { .. } | Role::Agent { .. } => {
                self.drive_natively(driver, &spec, target, auth, driver_side, seq)
                    .await
            }
            Role::Device {
                device,
                plugin: Some(plugin),
            } => {
                let mut params = json!({
                    "engine": n.engine.as_str(),
                    "target": target.host,
                    "port": target.port,
                    "direction": as_db(&direction),
                    "durationS": spec.duration_s,
                    "streams": spec.streams,
                });
                if let Some(limit) = spec.limit_mbps {
                    params["limitMbps"] = json!(limit);
                }
                if let Some(auth) = auth {
                    params["user"] = json!(auth.user);
                    params["password"] = json!(auth.password);
                }
                let mut call = Call {
                    device_id: device.id,
                    slug: plugin.slug.clone(),
                    action: plugin.roles.actions.run.clone().unwrap_or_default(),
                    params,
                    progress: None,
                    cancel: self.reservation.cancel.clone(),
                };
                if let Served::Armed(session) = served {
                    self.run_against_armed(idx, &keys, call, session, driver_side, seq)
                        .await
                } else {
                    let (progress_tx, progress_rx) = mpsc::unbounded_channel::<ProgressSample>();
                    call.progress = Some(progress_tx);
                    let forwarder = forward_samples(
                        progress_rx,
                        Arc::clone(&self.book),
                        seq,
                        move |sample: ProgressSample| {
                            let flow = match sample.flow {
                                ProgressFlow::Tx => Flow::Tx,
                                ProgressFlow::Rx => Flow::Rx,
                            };
                            Some((link_flow(driver_side, flow), sample.bps, sample.t_ms))
                        },
                    );
                    let result = self.invoke(idx, &keys, call).await;
                    drain(forwarder).await;
                    match result {
                        Ok(run) if run.status == RunStatus::Succeeded => {
                            Ok(parse_report(run.output))
                        }
                        Ok(run) => Err(StageError::from_outcome(&run, "A medida")),
                        Err(error) => Err(StageError::from_error(&error)),
                    }
                }
            }
            Role::Device { plugin: None, .. } => Err(StageError::Failed(format!(
                "{} não tem plugin de teste de enlace",
                driver.view.name
            ))),
        };
        match result {
            Ok(measured) => {
                let (forward, reverse) = link_rates(driver_side, measured.tx_bps, measured.rx_bps);
                if let Some(retransmits) = measured.retransmits {
                    let row = &mut self.rows[idx];
                    let total = row.retransmits.unwrap_or(0)
                        + i64::try_from(retransmits).unwrap_or(i64::MAX);
                    row.retransmits = Some(total);
                }
                for flow in flows {
                    let rate = match flow {
                        LinkTestFlow::Forward => forward,
                        LinkTestFlow::Reverse => reverse,
                    };
                    let key = LinkTestStageKey::of_flow(*flow);
                    match rate {
                        Some(bps) => {
                            match flow {
                                LinkTestFlow::Forward => self.rows[idx].forward_bps = Some(bps),
                                LinkTestFlow::Reverse => self.rows[idx].reverse_bps = Some(bps),
                            }
                            self.set(idx, key, LinkTestStepStatus::Succeeded, None);
                        }
                        None => self.set(
                            idx,
                            key,
                            LinkTestStepStatus::Failed,
                            Some("o motor não informou a vazão deste sentido".to_owned()),
                        ),
                    }
                }
                self.persist(idx).await;
            }
            Err(error) => {
                let (status, message) = match error {
                    StageError::Cancelled => {
                        (LinkTestStepStatus::Cancelled, CANCELLED_TEXT.to_owned())
                    }
                    StageError::Failed(message) => (LinkTestStepStatus::Failed, message),
                };
                for key in keys {
                    self.set(idx, key, status, Some(message.clone()));
                }
                self.persist(idx).await;
            }
        }
    }

    /// A central (ou um agente) dirige pelo motor nativo.
    async fn drive_natively(
        &self,
        driver: &Endpoint,
        spec: &ThroughputSpec,
        target: &Target,
        auth: Option<&TestAuth>,
        driver_side: LinkTestSide,
        seq: u32,
    ) -> Result<Measured, StageError> {
        let engine = match driver.role {
            Role::Agent { agent_id } => self
                .env
                .drivers
                .agent(agent_id)
                .map_err(|error| StageError::Failed(error.to_string()))?,
            _ => self.env.drivers.central(),
        };
        let request = DriveRequest {
            target: target.socket().map_err(StageError::Failed)?,
            spec: spec.clone(),
            auth: auth.map(|auth| BtestAuth {
                user: auth.user.clone(),
                password: auth.password.clone(),
            }),
        };
        let (sink, samples) = sample_channel();
        let forwarder = forward_samples(
            samples,
            Arc::clone(&self.book),
            seq,
            move |sample: ThroughputSample| {
                Some((link_flow(driver_side, sample.flow), sample.bps, sample.t_ms))
            },
        );
        let result = engine
            .drive(&request, &sink, &self.reservation.cancel)
            .await;
        drop(sink);
        drain(forwarder).await;
        result
            .map(|report| Measured {
                tx_bps: report.tx_bps,
                rx_bps: report.rx_bps,
                retransmits: report.retransmits,
            })
            .map_err(|error| StageError::from_error(&error))
    }

    /// O MikroTik dirige contra o servidor armado da central: a medida
    /// oficial é a do servidor (mesmo ponto de vista: o do cliente); a saída
    /// do plugin fica de reserva.
    async fn run_against_armed(
        &mut self,
        idx: usize,
        keys: &[LinkTestStageKey],
        call: Call,
        session: &mut ArmedSession,
        driver_side: LinkTestSide,
        seq: u32,
    ) -> Result<Measured, StageError> {
        let samples = std::mem::replace(&mut session.samples, mpsc::unbounded_channel().1);
        let forwarder = forward_samples(
            samples,
            Arc::clone(&self.book),
            seq,
            move |sample: ThroughputSample| {
                Some((link_flow(driver_side, sample.flow), sample.bps, sample.t_ms))
            },
        );
        let token = session.cancel_token();
        let plugin = async {
            let result = self.invoke(idx, keys, call).await;
            let succeeded = matches!(&result, Ok(run) if run.status == RunStatus::Succeeded);
            if succeeded {
                let token = token.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(ARMED_GRACE).await;
                    token.cancel();
                });
            } else {
                token.cancel();
            }
            result
        };
        let (plugin, server) = tokio::join!(plugin, session.outcome.wait());
        drain(forwarder).await;
        match (server, plugin) {
            (Ok(report), _) => Ok(Measured {
                tx_bps: report.tx_bps,
                rx_bps: report.rx_bps,
                retransmits: report.retransmits,
            }),
            (Err(_), Ok(run)) if run.status == RunStatus::Succeeded => Ok(parse_report(run.output)),
            (Err(_), Ok(run)) => Err(StageError::from_outcome(&run, "A medida")),
            (Err(_), Err(error)) => Err(StageError::from_error(&error)),
        }
    }

    /// Encerra o servidor — sempre, com um token próprio (o do teste pode
    /// já estar cancelado).
    async fn stop(&mut self, idx: usize, n: &Negotiation, served: Served) {
        let key = LinkTestStageKey::Stop;
        match served {
            Served::Nothing => {
                self.end(
                    idx,
                    key,
                    LinkTestStepStatus::Skipped,
                    Some("nada a encerrar".to_owned()),
                )
                .await;
            }
            Served::Armed(session) => {
                session.cancel();
                drop(session);
                self.end(
                    idx,
                    key,
                    LinkTestStepStatus::Succeeded,
                    Some("servidor btest da central desarmado".to_owned()),
                )
                .await;
            }
            Served::Device(cleanup) => {
                self.begin(idx, &[key]).await;
                let call = Call {
                    device_id: cleanup.device_id,
                    slug: cleanup.plugin_slug.clone(),
                    action: cleanup.action.clone(),
                    params: json!({ "engine": n.engine.as_str(), "handle": cleanup.handle }),
                    progress: None,
                    cancel: CancellationToken::new(),
                };
                let stopped = match self.invoke(idx, &[key], call).await {
                    Ok(run) if run.status == RunStatus::Succeeded => {
                        if run
                            .output
                            .as_ref()
                            .and_then(|output| output.get("stopped"))
                            .and_then(Value::as_bool)
                            == Some(false)
                        {
                            Err("o equipamento não confirmou o encerramento".to_owned())
                        } else {
                            Ok(())
                        }
                    }
                    Ok(run) => Err(run
                        .error
                        .unwrap_or_else(|| "o encerramento falhou".to_owned())),
                    Err(error) => Err(error.to_string()),
                };
                match stopped {
                    Ok(()) => {
                        self.rows[idx].cleanup = None;
                        self.end(
                            idx,
                            key,
                            LinkTestStepStatus::Succeeded,
                            Some("servidor encerrado".to_owned()),
                        )
                        .await;
                    }
                    Err(message) => {
                        self.end(
                            idx,
                            key,
                            LinkTestStepStatus::Failed,
                            Some(format!(
                                "{message}; a limpeza fica pendente e é refeita no próximo início \
                                 do servidor"
                            )),
                        )
                        .await;
                    }
                }
            }
        }
    }

    async fn finish(&mut self) {
        let statuses: Vec<LinkTestStepStatus> = self
            .rows
            .iter()
            .map(|row| {
                crate::services::link_tests::model::from_db(&row.status)
                    .unwrap_or(LinkTestStepStatus::Failed)
            })
            .collect();
        let status = outcome::test_status(&statuses, self.cancelled());
        let rates: Vec<outcome::LegRates> = self
            .rows
            .iter()
            .zip(&statuses)
            .map(|(row, status)| {
                (
                    u32::try_from(row.seq).unwrap_or_default(),
                    crate::services::link_tests::model::from_db(&row.kind)
                        .unwrap_or(LinkTestLegKind::Link),
                    *status,
                    row.forward_bps,
                    row.reverse_bps,
                )
            })
            .collect();
        self.test.status = as_db(&status);
        self.test.summary = outcome::summarize(&rates).map(|summary| json!(summary));
        self.test.error = match status {
            LinkTestStatus::Cancelled => Some(CANCELLED_TEXT.to_owned()),
            LinkTestStatus::Failed => self
                .rows
                .iter()
                .find_map(|row| row.error.clone())
                .or_else(|| Some("Nenhuma perna mediu o enlace".to_owned())),
            _ => None,
        };
        self.test.finished_at = Some(Utc::now().into());
        self.save_test().await;
        self.record_schedule(status).await;
    }

    /// O "último teste" do agendamento que disparou este.
    async fn record_schedule(&self, status: LinkTestStatus) {
        let Some(schedule_id) = self.test.schedule_id else {
            return;
        };
        if let Err(error) =
            schedules::record_result(&self.env, schedule_id, self.test.id, status).await
        {
            tracing::warn!(%error, schedule_id, "falha ao gravar o desfecho do agendamento");
        }
    }
}

/// O relatório do plugin, tolerante a campos ausentes.
fn parse_report(output: Option<Value>) -> Measured {
    output
        .and_then(|output| serde_json::from_value::<PluginReport>(output).ok())
        .map(Measured::from)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credencial_do_teste_e_aleatoria_e_mascarada() {
        let a = TestAuth::generate();
        let b = TestAuth::generate();
        assert!(a.user.starts_with("nm-test-"));
        assert_eq!(a.password.len(), 24);
        assert_ne!(a.password, b.password);
    }

    #[test]
    fn relatorio_do_plugin_e_lido_com_tolerancia() {
        let m = parse_report(Some(
            json!({ "engine": "iperf3", "txBps": 9.5e8, "samples": [] }),
        ));
        assert_eq!(m.tx_bps, Some(9.5e8));
        assert_eq!(m.rx_bps, None);
        assert_eq!(parse_report(Some(json!("texto"))).tx_bps, None);
        assert_eq!(parse_report(None).tx_bps, None);
    }

    #[test]
    fn livro_de_amostras_tem_teto_por_sentido() {
        let book = SampleBook {
            test_id: 1,
            bus: None,
            inner: Mutex::default(),
        };
        for t in 0..200u64 {
            book.push(1, LinkTestFlow::Forward, 1.0, t);
        }
        book.push(1, LinkTestFlow::Reverse, 2.0, 0);
        let samples = book.of(1);
        assert_eq!(
            samples
                .iter()
                .filter(|s| s.flow == LinkTestFlow::Forward)
                .count(),
            MAX_SAMPLES_PER_FLOW
        );
        assert_eq!(samples.len(), MAX_SAMPLES_PER_FLOW + 1);
        assert!(book.of(2).is_empty());
    }

    #[test]
    fn alvo_precisa_ser_ip() {
        let ok = Target {
            host: "127.0.0.1".into(),
            port: 5201,
        };
        assert_eq!(ok.socket().unwrap().port(), 5201);
        let ruim = Target {
            host: "servidor.local".into(),
            port: 1,
        };
        assert!(ruim.socket().is_err());
    }
}
