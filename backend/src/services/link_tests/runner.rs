//! A máquina de estados de um teste de enlace (ADR 015 §5).
//!
//! As pernas rodam **uma por vez**, em ordem; cada perna percorre as etapas
//! `prepare_a` → `prepare_b` → `serve` → `forward`/`reverse` → `stop`. A
//! primeira etapa de cada lado só **confere** (`check`): instalar o que falta
//! é um passo que o operador aceita no plano, nunca uma surpresa aqui. O
//! `stop` roda **sempre** — sucesso, falha ou cancelamento — e o `handle` do
//! servidor que um equipamento subiu é gravado na perna **antes** de ser
//! usado: se o processo cair no meio, o boot o desfaz ([`super::janitor`]).
//!
//! Toda mudança de etapa é gravada e publicada (`link_test:updated`); as
//! amostras de 1 s saem como `link_test:sample` (no máximo duas por segundo
//! por sentido) e ficam na perna, com teto, para o `GET` de detalhe. Enquanto
//! a perna mede, CPU e memória das pontas e dos equipamentos do caminho são
//! lidos a cada segundo ([`super::health`]) no mesmo relógio da vazão e saem
//! como `link_test:health`.

use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, Instant},
};

use chrono::Utc;
use rand::{distributions::Alphanumeric, Rng};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
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
    health::{HealthSampler, HealthTarget, LinkTestHealthEvent, MAX_HEALTH_PER_DEVICE},
    model::{
        as_db, LinkTestFlow, LinkTestHealthSample, LinkTestLegKind, LinkTestRole, LinkTestSample,
        LinkTestSampleEvent, LinkTestSide, LinkTestSpec, LinkTestStage, LinkTestStageKey,
        LinkTestStatus, LinkTestStepStatus, LinkTestUserRef,
    },
    negotiate::{driver_direction, link_flow, link_rates, Negotiation},
    outcome,
    plan::PlannedLeg,
    readiness::{self, Readiness},
    repository, schedules, view,
};
use crate::{
    models::{devices, link_test_legs, link_tests},
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
    health: HashMap<u32, Vec<LinkTestHealthSample>>,
    /// Início da medição de cada perna: o relógio comum de vazão e CPU.
    clocks: HashMap<u32, Instant>,
    published: HashMap<(u32, LinkTestFlow), Instant>,
}

fn millis_since(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)
}

impl SampleBook {
    fn lock(&self) -> std::sync::MutexGuard<'_, BookInner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Zera o relógio da perna (o início da medição).
    fn start_clock(&self, seq: u32) {
        self.lock().clocks.entry(seq).or_insert_with(Instant::now);
    }

    /// Milissegundos desde o início da medição da perna.
    fn clock(&self, seq: u32) -> Option<u64> {
        self.lock().clocks.get(&seq).copied().map(millis_since)
    }

    fn push(&self, seq: u32, flow: LinkTestFlow, bps: f64, t_ms: u64) {
        let at_ms = self.clock(seq);
        let publish = {
            let mut inner = self.lock();
            let samples = inner.by_leg.entry(seq).or_default();
            if samples.iter().filter(|sample| sample.flow == flow).count() < MAX_SAMPLES_PER_FLOW {
                samples.push(LinkTestSample {
                    flow,
                    bps,
                    t_ms,
                    at_ms,
                });
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
                    at_ms,
                },
            );
        }
    }

    fn of(&self, seq: u32) -> Vec<LinkTestSample> {
        self.lock().by_leg.get(&seq).cloned().unwrap_or_default()
    }

    /// Uma leitura de CPU/memória: guardada (com teto por equipamento) e
    /// publicada — uma por segundo por equipamento, sem ritmo a aplicar.
    fn push_health(&self, seq: u32, sample: LinkTestHealthSample) {
        {
            let mut inner = self.lock();
            let list = inner.health.entry(seq).or_default();
            let of_device = list
                .iter()
                .filter(|kept| kept.device_id == sample.device_id)
                .count();
            if of_device < MAX_HEALTH_PER_DEVICE {
                list.push(sample);
            }
        }
        events::publish_health(
            self.bus.as_ref(),
            LinkTestHealthEvent {
                test_id: self.test_id,
                leg_seq: seq,
                sample,
            },
        );
    }

    fn health_of(&self, seq: u32) -> Vec<LinkTestHealthSample> {
        self.lock().health.get(&seq).cloned().unwrap_or_default()
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

impl Measured {
    /// Nenhum sentido passou um bit: o motor "terminou" sem medir (o RouterOS
    /// imprime médias zeradas quando nem conecta).
    fn is_empty(&self) -> bool {
        ![self.tx_bps, self.rx_bps]
            .into_iter()
            .flatten()
            .any(|bps| bps > 0.0)
    }
}

/// Parâmetros do `serve` de um plugin (contrato do ADR 015 §3).
fn serve_params(
    engine: ThroughputEngine,
    port: u16,
    peer: &str,
    require_auth: bool,
    auth: Option<&TestAuth>,
) -> Value {
    let mut params = json!({
        "engine": engine.as_str(),
        "port": port,
        "peer": peer,
        "requireAuth": require_auth,
    });
    with_auth(&mut params, auth);
    params
}

/// Parâmetros do `run` de um plugin (contrato do ADR 015 §3).
fn run_params(spec: &ThroughputSpec, target: &Target, auth: Option<&TestAuth>) -> Value {
    let mut params = json!({
        "engine": spec.engine.as_str(),
        "target": target.host,
        "port": target.port,
        "direction": as_db(&spec.direction),
        "durationS": spec.duration_s,
        "streams": spec.streams,
    });
    if let Some(limit) = spec.limit_mbps {
        params["limitMbps"] = json!(limit);
    }
    with_auth(&mut params, auth);
    params
}

fn with_auth(params: &mut Value, auth: Option<&TestAuth>) {
    if let Some(auth) = auth {
        params["user"] = json!(auth.user);
        params["password"] = json!(auth.password);
    }
}

/// Medida de um `run` de plugin que terminou bem — zerada é falha.
fn measured_from_run(output: Option<Value>) -> Result<Measured, StageError> {
    let measured = parse_report(output);
    if measured.is_empty() {
        Err(StageError::Failed(
            "o equipamento terminou o teste sem medir nada (nenhum byte passou)".to_owned(),
        ))
    } else {
        Ok(measured)
    }
}

/// O equipamento armado não conectou, mas outro IP bateu na porta do btest
/// durante a janela.
fn nat_message(expected: std::net::IpAddr, stray: std::net::IpAddr) -> String {
    format!(
        "O equipamento não conectou ao servidor btest da central como {expected}: a conexão \
         chegou de {stray}. Há NAT entre ele e a central — cadastre o IP pelo qual a central o \
         vê ({stray}) ou rode o teste sem NAT no caminho"
    )
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
        let health = self.book.health_of(seq);
        let row = &mut self.rows[idx];
        row.stages = json!(self.stages[idx]);
        row.samples = (!samples.is_empty()).then(|| json!(samples));
        row.health = (!health.is_empty()).then(|| json!(health));
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

    /// A primeira etapa de cada lado. Com `check`, só **confere** — faltando
    /// algo, a perna para com a frase do plugin (instalar é o passo aceito no
    /// plano). Plugin sem `check` (o contrato de antes) roda o `prepare`.
    /// `false` = a perna para.
    async fn prepare(&mut self, idx: usize, planned: &PlannedLeg, n: &Negotiation) -> bool {
        for side in [LinkTestSide::A, LinkTestSide::B] {
            let key = LinkTestStageKey::prepare_of(side);
            let endpoint = planned.endpoint(side);
            let step = endpoint.plugin().and_then(|plugin| {
                let actions = &plugin.roles.actions;
                actions
                    .check
                    .clone()
                    .map(|action| (action, true))
                    .or_else(|| actions.prepare.clone().map(|action| (action, false)))
            });
            let (Some(device_id), Some(plugin), Some((action, checks))) =
                (endpoint.device_id(), endpoint.plugin(), step)
            else {
                self.end(
                    idx,
                    key,
                    LinkTestStepStatus::Skipped,
                    Some("nada a conferir".to_owned()),
                )
                .await;
                continue;
            };
            if self.cancelled() {
                return false;
            }
            self.begin(idx, &[key]).await;
            let call = Call {
                device_id,
                slug: plugin.slug.clone(),
                action,
                params: readiness::params(n.engine, LinkTestRole::of(side == n.server)),
                progress: None,
                cancel: self.reservation.cancel.clone(),
            };
            let installs = plugin.roles.actions.prepare.is_some();
            let verdict = match self.invoke(idx, &[key], call).await {
                Ok(run) if run.status == RunStatus::Succeeded => {
                    let output = run.output.unwrap_or(Value::Null);
                    if checks {
                        checked(&endpoint.view.name, n.engine, installs, &output)
                    } else {
                        prepared(&endpoint.view.name, n.engine, &output)
                    }
                }
                Ok(run) => Err(StageError::from_outcome(
                    &run,
                    if checks {
                        "A conferência"
                    } else {
                        "O preparo"
                    },
                )),
                Err(error) => Err(StageError::from_error(&error)),
            };
            match verdict {
                Ok(message) => {
                    self.end(idx, key, LinkTestStepStatus::Succeeded, Some(message))
                        .await;
                }
                Err(error) => {
                    self.fail_stage(idx, key, error).await;
                    return false;
                }
            }
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
                let params = serve_params(
                    n.engine,
                    port,
                    &peer.unwrap_or_default(),
                    n.require_auth,
                    auth,
                );
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
        let seq = planned.view.seq;
        self.book.start_clock(seq);
        let health = self.start_health(planned, seq).await;
        // Um sentido por vez, em todo motor. O btest até aceita os dois ao
        // mesmo tempo, mas num RouterOS real o "both" em TCP não devolve o
        // envio (tx 0) — e um sentido de cada vez mede melhor roteador de CPU
        // fraca, sem um sentido roubar CPU do outro.
        for (index, flow) in flows.into_iter().enumerate() {
            if self.cancelled() {
                break;
            }
            // A sessão armada da central é de uso único: o segundo sentido
            // arma outra para o mesmo equipamento.
            if index > 0 {
                if let Err(message) = self.rearm(served) {
                    self.set(
                        idx,
                        LinkTestStageKey::of_flow(flow),
                        LinkTestStepStatus::Failed,
                        Some(message),
                    );
                    continue;
                }
            }
            let direction = driver_direction(n.driver(), flow);
            self.measure_call(idx, planned, n, served, target, auth, direction, &[flow])
                .await;
        }
        health.stop().await;
    }

    /// Começa a ler CPU/memória das pontas e dos equipamentos do caminho
    /// (os que têm como ser lidos — SNMP ligado, ou a central).
    async fn start_health(&self, planned: &PlannedLeg, seq: u32) -> HealthSampler {
        let via = if planned.view.via_device_ids.is_empty() {
            Vec::new()
        } else {
            devices::Entity::find()
                .filter(devices::Column::Id.is_in(planned.view.via_device_ids.clone()))
                .all(&self.env.db)
                .await
                .unwrap_or_default()
        };
        let mut targets: Vec<(i64, HealthTarget<'_>)> = Vec::new();
        for endpoint in [&planned.a, &planned.b] {
            if let (Some(device), Some(target)) =
                (endpoint.device(), HealthTarget::of_endpoint(endpoint))
            {
                targets.push((device.id, target));
            }
        }
        targets.extend(
            via.iter()
                .map(|device| (device.id, HealthTarget::Device(device))),
        );
        let mut readers = Vec::new();
        for (device_id, target) in targets {
            if readers.iter().any(|(id, _)| *id == device_id) {
                continue;
            }
            if let Some(reader) = self.env.health.reader_for(target).await {
                readers.push((device_id, reader));
            }
        }
        let clock = Arc::clone(&self.book);
        let sink = Arc::clone(&self.book);
        HealthSampler::start(
            readers,
            move || clock.clock(seq).unwrap_or_default(),
            Arc::new(move |sample| sink.push_health(seq, sample)),
        )
    }

    /// Troca a sessão btest já usada por uma nova, para o mesmo IP. Outros
    /// servidores (plugin) atendem várias medidas e ficam como estão.
    fn rearm(&self, served: &mut Served) -> Result<(), String> {
        let Served::Armed(previous) = served else {
            return Ok(());
        };
        let peer = previous.peer;
        // Desarma antes: o mesmo IP não pode ter duas sessões armadas.
        *served = Served::Nothing;
        let session = self
            .env
            .btest
            .arm(peer, btest::DEFAULT_ARM_TTL)
            .map_err(|error| error.to_string())?;
        *served = Served::Armed(session);
        Ok(())
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
                let params = run_params(&spec, target, auth);
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
                            measured_from_run(run.output)
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
        if server.is_err() {
            // Ninguém conectou ao servidor armado. Se uma conexão chegou de
            // outro IP na janela, é o equipamento atrás de NAT: dizer isso
            // vale mais que qualquer número que o plugin tenha impresso.
            if let Some(stray) = session.stray_peer() {
                return Err(StageError::Failed(nat_message(session.peer, stray)));
            }
        }
        match (server, plugin) {
            (Ok(report), _) => Ok(Measured {
                tx_bps: report.tx_bps,
                rx_bps: report.rx_bps,
                retransmits: report.retransmits,
            }),
            (Err(error), Ok(run)) if run.status == RunStatus::Succeeded => {
                measured_from_run(run.output).map_err(|_| StageError::from_error(&error))
            }
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
/// O desfecho do `check` na etapa: pronto segue; faltando, a perna para com
/// a frase do plugin e, quando o plugin instala, o caminho para resolver.
fn checked(
    device_name: &str,
    engine: ThroughputEngine,
    installs: bool,
    output: &Value,
) -> Result<String, StageError> {
    let readiness = Readiness::from_output(Some(output))
        .map_err(|error| StageError::Failed(format!("{device_name}: {error}")))?;
    if readiness.ready {
        return Ok("pronto".to_owned());
    }
    let reason = readiness.reason(device_name, engine);
    Err(StageError::Failed(if readiness.can_install && installs {
        format!("{reason} — aceite a instalação no plano do teste")
    } else {
        reason
    }))
}

/// O desfecho do `prepare` de um plugin sem `check` (pode ter instalado).
fn prepared(
    device_name: &str,
    engine: ThroughputEngine,
    output: &Value,
) -> Result<String, StageError> {
    if output.get("ready").and_then(Value::as_bool) == Some(false) {
        // O plugin sabe por que não ficou pronto (sem internet, sem espaço,
        // pacote fora do repositório): essa é a parte útil da mensagem.
        let reason = output
            .get("message")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty());
        return Err(StageError::Failed(match reason {
            Some(reason) => format!(
                "{device_name} não ficou pronto para o teste ({}): {reason}",
                engine.as_str()
            ),
            None => format!(
                "{device_name} não ficou pronto para o teste ({})",
                engine.as_str()
            ),
        }));
    }
    let installed: Vec<&str> = output
        .get("installed")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    Ok(if installed.is_empty() {
        "pronto".to_owned()
    } else {
        format!("instalado: {}", installed.join(", "))
    })
}

fn parse_report(output: Option<Value>) -> Measured {
    output
        .and_then(|output| serde_json::from_value::<PluginReport>(output).ok())
        .map(Measured::from)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Contrato entre o orquestrador e os plugins embutidos: o que o runner
    /// manda no `serve` e no `run` — inclusive a credencial gerada a cada
    /// teste — passa na validação de parâmetros de cada plugin. Um padrão
    /// errado num plugin (já houve um `pattern` com um valor de exemplo)
    /// derruba o teste só no equipamento real; aqui ele quebra antes.
    #[test]
    fn parametros_do_orquestrador_passam_nos_plugins_embutidos() {
        use crate::services::plugins::{builtin, params};
        let auth = TestAuth::generate();
        let mut checked = 0;
        for package in builtin::packages() {
            let manifest = &package.manifest;
            let Some(roles) = &manifest.throughput else {
                continue;
            };
            let schema = |id: &Option<String>| {
                let id = id.as_deref()?;
                manifest
                    .actions
                    .iter()
                    .find(|action| action.id == id)
                    .map(|action| action.params.clone())
            };
            for serve in &roles.serve {
                let port = match serve.engine {
                    ThroughputEngine::Iperf3 => 5_201,
                    ThroughputEngine::Btest => btest::DEFAULT_PORT,
                };
                let modes: &[bool] = if serve.engine == ThroughputEngine::Btest {
                    &[true, false]
                } else {
                    &[false]
                };
                for &require_auth in modes {
                    let auth = require_auth.then_some(&auth);
                    let sent = serve_params(serve.engine, port, "10.0.0.2", require_auth, auth);
                    let declared = schema(&roles.actions.serve).expect("serve declarado");
                    params::validate(declared.as_ref(), &sent).unwrap_or_else(|errors| {
                        panic!("{} serve {sent}: {errors:?}", manifest.slug)
                    });
                    checked += 1;
                }
            }
            for drive in &roles.drive {
                for direction in [TestDirection::Send, TestDirection::Receive] {
                    let spec = ThroughputSpec {
                        engine: drive.engine,
                        direction,
                        protocol: TestProtocol::Tcp,
                        duration_s: 5,
                        streams: 1,
                        limit_mbps: Some(100),
                    };
                    let target = Target {
                        host: "10.0.0.1".to_owned(),
                        port: 2_000,
                    };
                    // Como na negociação: credencial só no btest com cliente
                    // que autentica.
                    let with_auth = drive.engine == ThroughputEngine::Btest && drive.auth;
                    let sent = run_params(&spec, &target, with_auth.then_some(&auth));
                    let declared = schema(&roles.actions.run).expect("run declarado");
                    params::validate(declared.as_ref(), &sent).unwrap_or_else(|errors| {
                        panic!("{} run {sent}: {errors:?}", manifest.slug)
                    });
                    checked += 1;
                }
            }
            // Conferir e instalar recebem `{ engine, role }` em cada papel.
            let papers = roles
                .serve
                .iter()
                .map(|serve| (serve.engine, LinkTestRole::Serve))
                .chain(
                    roles
                        .drive
                        .iter()
                        .map(|drive| (drive.engine, LinkTestRole::Drive)),
                );
            for (engine, role) in papers {
                let sent = readiness::params(engine, role);
                for (name, action) in [
                    ("check", &roles.actions.check),
                    ("prepare", &roles.actions.prepare),
                ] {
                    let Some(declared) = schema(action) else {
                        continue;
                    };
                    params::validate(declared.as_ref(), &sent).unwrap_or_else(|errors| {
                        panic!("{} {name} {sent}: {errors:?}", manifest.slug)
                    });
                    checked += 1;
                }
            }
        }
        assert!(checked > 0, "nenhum plugin embutido com papel de teste");
    }

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
