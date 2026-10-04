//! Orquestração do discovery: a sessão ao vivo, o ciclo de vida de uma
//! execução e as consultas do histórico.
//!
//! A sessão chega à tela pelo barramento global (`discovery:scan`, snapshot
//! do [`EventBus`]) — quem abre a página depois recebe o estado atual logo ao
//! conectar, sem endpoint SSE próprio nem hidratação por HTTP (AGENTS §9).

use chrono::{Duration as ChronoDuration, Utc};
use loco_rs::app::AppContext;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use serde::Serialize;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use crate::{
    models::{
        _entities::{
            discovery_results::Column as ResultColumn, discovery_runs::Column as RunColumn,
        },
        discovery_results, discovery_runs, networks,
    },
    services::{
        discovery::{
            cidr_range::parse_cidr_range,
            merger::DiscoveredHost,
            pipeline::{self, ScanTarget},
            progress::{phase, ScanEvent, ScanReporter},
        },
        events::EventBus,
        monitoring::{checkers::ping::PingClient, runner::CheckDeps},
        shared::errors::{AppError, AppResult},
    },
};

/// Evento de snapshot da sessão no barramento global.
pub const SCAN_EVENT: &str = "discovery:scan";
const SCAN_SNAPSHOT_KEY: &str = "session";
/// Intervalo mínimo entre quadros publicados durante a varredura: a tela anda
/// suave e o barramento não recebe uma rajada por host respondido.
const PUBLISH_INTERVAL: Duration = Duration::from_millis(200);
/// Linhas de registro guardadas na sessão.
const MAX_LOGS: usize = 40;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSessionState {
    pub run_id: Option<i64>,
    pub network_id: Option<i64>,
    pub status: String,
    pub phase: String,
    pub progress_current: usize,
    pub progress_total: usize,
    pub hosts: Vec<DiscoveredHost>,
    pub logs: Vec<String>,
    pub error: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}
impl Default for ScanSessionState {
    fn default() -> Self {
        Self {
            run_id: None,
            network_id: None,
            status: "idle".into(),
            phase: phase::IDLE.into(),
            progress_current: 0,
            progress_total: 0,
            hosts: vec![],
            logs: vec![],
            error: None,
            started_at: None,
            finished_at: None,
        }
    }
}

impl ScanSessionState {
    fn is_active(&self) -> bool {
        matches!(self.status.as_str(), "running" | "pending")
    }

    fn log(&mut self, line: impl Into<String>) {
        self.logs.push(line.into());
        if self.logs.len() > MAX_LOGS {
            let excess = self.logs.len() - MAX_LOGS;
            self.logs.drain(..excess);
        }
    }
}

#[derive(Clone)]
pub struct ScanSessionService {
    state: Arc<RwLock<ScanSessionState>>,
    bus: EventBus,
    cancel: Arc<RwLock<Option<CancellationToken>>>,
}
impl ScanSessionService {
    #[must_use]
    pub fn create(bus: EventBus) -> Self {
        let service = Self {
            state: Arc::new(RwLock::new(ScanSessionState::default())),
            bus,
            cancel: Arc::new(RwLock::new(None)),
        };
        service.publish(&ScanSessionState::default());
        service
    }
    pub fn from_context(ctx: &AppContext) -> AppResult<Self> {
        ctx.shared_store.get::<Self>().ok_or_else(|| {
            AppError::Internal(anyhow::anyhow!("Sessão de discovery não inicializada"))
        })
    }
    pub async fn state(&self) -> ScanSessionState {
        self.state.read().await.clone()
    }
    pub async fn start(&self, run_id: i64, network_id: i64) -> CancellationToken {
        let token = CancellationToken::new();
        *self.cancel.write().await = Some(token.clone());
        let mut state = self.state.write().await;
        *state = ScanSessionState {
            run_id: Some(run_id),
            network_id: Some(network_id),
            status: "running".into(),
            phase: phase::SWEEP.into(),
            started_at: Some(Utc::now().to_rfc3339()),
            logs: vec!["Varredura iniciada.".into()],
            ..Default::default()
        };
        self.publish(&state);
        token
    }
    pub async fn cancel(&self) {
        if let Some(token) = self.cancel.write().await.take() {
            token.cancel();
        }
        let mut state = self.state.write().await;
        if state.is_active() {
            state.status = "cancelled".into();
            state.phase = phase::IDLE.into();
            state.finished_at = Some(Utc::now().to_rfc3339());
            state.log("Varredura cancelada.");
            self.publish(&state);
        }
    }
    pub async fn wait_for_probe(&self) {
        let mut state = self.state.write().await;
        state.status = "pending".into();
        state.phase = phase::PROBE.into();
        state.log("Aguardando execução pelo probe remoto.");
        self.publish(&state);
    }
    pub async fn remote_started(&self, run_id: i64) {
        let mut state = self.state.write().await;
        if state.run_id == Some(run_id) && state.status == "pending" {
            state.status = "running".into();
            state.phase = phase::PROBE.into();
            state.log("Probe remoto iniciou a varredura.");
            self.publish(&state);
        }
    }
    pub async fn hosts(&self, hosts: &[DiscoveredHost]) {
        let mut state = self.state.write().await;
        state.hosts = hosts.to_vec();
        self.publish(&state);
    }
    /// Aplica vários eventos dos scanners e publica um quadro só.
    async fn apply(&self, events: Vec<ScanEvent>) {
        let mut state = self.state.write().await;
        // Evento atrasado de uma varredura cancelada não ressuscita a sessão.
        if state.status != "running" {
            return;
        }
        for event in events {
            match event {
                ScanEvent::Progress {
                    phase,
                    current,
                    total,
                } => {
                    state.phase = phase.into();
                    state.progress_current = current;
                    state.progress_total = total;
                }
                ScanEvent::Hosts(hosts) => state.hosts = hosts,
                ScanEvent::Log(line) => state.log(line),
            }
        }
        self.publish(&state);
    }
    /// Acrescenta um dado a um host da execução ao vivo e republica — o
    /// palpite do Laya chega depois da varredura. Outra execução em curso
    /// não é tocada.
    pub async fn annotate_host(
        &self,
        run_id: i64,
        ip_address: &str,
        key: &str,
        value: serde_json::Value,
    ) {
        let mut state = self.state.write().await;
        if state.run_id != Some(run_id) {
            return;
        }
        let Some(host) = state
            .hosts
            .iter_mut()
            .find(|host| host.ip_address == ip_address)
        else {
            return;
        };
        if !host.data.is_object() {
            host.data = serde_json::json!({});
        }
        host.data[key] = value;
        self.publish(&state);
    }
    pub async fn finish(&self, error: Option<String>) {
        {
            let mut state = self.state.write().await;
            state.status = if error.is_some() {
                "failed"
            } else if state.status == "cancelled" {
                "cancelled"
            } else {
                "completed"
            }
            .into();
            state.error = error;
            state.finished_at = Some(Utc::now().to_rfc3339());
            state.phase = phase::IDLE.into();
            if state.status == "completed" {
                state.progress_current = state.progress_total;
                let found = state.hosts.len();
                state.log(format!(
                    "Varredura finalizada: {found} dispositivo(s) encontrado(s)."
                ));
            } else {
                state.log("Varredura encerrada.");
            }
            self.publish(&state);
        }
        *self.cancel.write().await = None;
    }
    fn publish(&self, state: &ScanSessionState) {
        if let Ok(payload) = serde_json::to_value(state) {
            self.bus
                .publish_snapshot(SCAN_EVENT, SCAN_SNAPSHOT_KEY, payload);
        }
    }
}

/// Resposta de `POST /api/discovery/scan`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanStarted {
    pub run_id: i64,
    pub status: &'static str,
    pub usable_hosts: u32,
    pub truncated: bool,
    pub execution: &'static str,
}

/// Inicia a varredura de uma rede: local, em segundo plano, ou entregue ao
/// probe remoto dono da rede.
///
/// # Errors
///
/// Rede inexistente, faixa não varredurável ou outra varredura em andamento.
pub async fn start_scan(ctx: &AppContext, network_id: i64) -> AppResult<ScanStarted> {
    let network = networks::Entity::find_by_id(network_id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Rede não encontrada"))?;
    let range = parse_cidr_range(&network.cidr).map_err(|_| {
        AppError::business_rule(format!(
            "A rede \"{}\" não tem uma faixa CIDR varredurável (valor atual: \"{}\").",
            network.name, network.cidr
        ))
    })?;
    let session = ScanSessionService::from_context(ctx)?;
    // A sessão ao vivo é uma só: uma segunda varredura sobrescreveria a tela
    // da primeira, que seguiria rodando sem dono.
    if session.state().await.is_active() {
        return Err(AppError::business_rule(
            "Já existe uma varredura em andamento — aguarde ou cancele para iniciar outra.",
        ));
    }
    let remote = network.probe_id.is_some();
    let run = discovery_runs::ActiveModel {
        network_id: Set(network.id),
        probe_id: Set(network.probe_id),
        status: Set(if remote { "pending" } else { "running" }.into()),
        started_at: Set(Utc::now().into()),
        configuration: Set(Some(serde_json::json!({
            "cidr": network.cidr,
            "usableHosts": range.usable_hosts,
            "truncated": range.truncated,
        }))),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await?;
    let cancel = session.start(run.id, network.id).await;
    if remote {
        session.wait_for_probe().await;
    } else {
        let ctx = ctx.clone();
        let target = ScanTarget::new(network.cidr.clone()).with_gateway(network.gateway.as_deref());
        tokio::spawn(async move {
            if let Err(error) = execute_run(&ctx, run.id, network.id, &target, cancel).await {
                tracing::warn!(%error, run_id = run.id, "falha ao concluir a varredura");
            }
        });
    }
    Ok(ScanStarted {
        run_id: run.id,
        status: if remote { "pending" } else { "running" },
        usable_hosts: range.usable_hosts,
        truncated: range.truncated,
        execution: if remote { "probe" } else { "local" },
    })
}

/// Roda uma execução local já marcada `running` até o fim: grava o status,
/// encerra a sessão e publica o evento durável da varredura. É o único lugar
/// que conhece esse ciclo — o botão "Escanear" e o agendador passam por aqui.
///
/// # Errors
///
/// Só falhas ao gravar o status; o erro da varredura vira status `failed`.
pub async fn execute_run(
    ctx: &AppContext,
    run_id: i64,
    network_id: i64,
    target: &ScanTarget,
    cancel: CancellationToken,
) -> AppResult<()> {
    let session = ScanSessionService::from_context(ctx)?;
    publish_run_event(
        ctx,
        "discovery:started",
        serde_json::json!({
            "runId": run_id,
            "networkId": network_id,
            "cidr": target.cidr,
            "message": format!("Varredura de {} iniciada", target.cidr),
        }),
    )
    .await;
    let outcome = run_discovery(ctx, target, run_id, cancel.clone()).await;
    let (status, error) = match &outcome {
        Ok(_) => ("completed", None),
        // Cancelamento não é falha: a run interrompida pelo operador precisa
        // aparecer como `cancelled` na lista (§7.7).
        Err(_) if cancel.is_cancelled() => ("cancelled", None),
        Err(error) => ("failed", Some(error.to_string())),
    };
    discovery_runs::ActiveModel {
        id: Set(run_id),
        status: Set(status.into()),
        finished_at: Set(Some(Utc::now().into())),
        error: Set(error.clone()),
        ..Default::default()
    }
    .update(&ctx.db)
    .await?;
    session.finish(error.clone()).await;

    let (event, message) = match (&outcome, status) {
        (Ok(hosts), _) => (
            "discovery:completed",
            format!(
                "Varredura de {} concluída: {} dispositivo(s)",
                target.cidr,
                hosts.len()
            ),
        ),
        (_, "cancelled") => (
            "discovery:cancelled",
            format!("Varredura de {} cancelada", target.cidr),
        ),
        _ => (
            "discovery:failed",
            format!(
                "Varredura de {} falhou: {}",
                target.cidr,
                error.as_deref().unwrap_or("erro desconhecido")
            ),
        ),
    };
    publish_run_event(
        ctx,
        event,
        serde_json::json!({
            "runId": run_id,
            "networkId": network_id,
            "cidr": target.cidr,
            "devicesFound": outcome.as_ref().map_or(0, Vec::len),
            "error": error,
            "message": message,
        }),
    )
    .await;
    Ok(())
}

/// Evento durável no histórico de eventos. Falhar aqui não pode derrubar a
/// varredura: o resultado já está gravado.
async fn publish_run_event(ctx: &AppContext, event: &str, payload: serde_json::Value) {
    if let Ok(bus) = EventBus::from_context(ctx) {
        if let Err(error) = bus.publish(&ctx.db, event, payload).await {
            tracing::debug!(%error, event, "evento de discovery não publicado");
        }
    }
}

/// Cancela a varredura ao vivo e marca a run.
///
/// # Errors
///
/// Falha de banco ao marcar a run.
pub async fn cancel_scan(ctx: &AppContext) -> AppResult<()> {
    let session = ScanSessionService::from_context(ctx)?;
    let run_id = session.state().await.run_id;
    session.cancel().await;
    if let Some(run_id) = run_id {
        discovery_runs::Entity::update_many()
            .col_expr(
                RunColumn::Status,
                sea_orm::sea_query::Expr::value("cancelled"),
            )
            .col_expr(
                RunColumn::FinishedAt,
                sea_orm::sea_query::Expr::value(Some(Utc::now())),
            )
            .filter(RunColumn::Id.eq(run_id))
            .filter(RunColumn::Status.is_in(["pending", "running"]))
            .exec(&ctx.db)
            .await?;
    }
    Ok(())
}

pub async fn run_discovery(
    ctx: &AppContext,
    target: &ScanTarget,
    run_id: i64,
    cancel: CancellationToken,
) -> AppResult<Vec<DiscoveredHost>> {
    let session = ScanSessionService::from_context(ctx)?;
    let (reporter, events) = ScanReporter::channel();
    let pump = tokio::spawn(pump_events(session.clone(), events));
    let outcome = match PingClient::from_context(ctx) {
        Ok(ping) => pipeline::scan(&ping, target, cancel, &reporter).await,
        Err(error) => Err(error),
    };
    // Derrubar o repórter fecha o canal e encerra o pump: só depois disso o
    // estado publicado é o final, e não uma atualização atrasada de fase.
    drop(reporter);
    let _ = pump.await;

    let merged = outcome?;
    session.hosts(&merged).await;
    persist_results(&ctx.db, run_id, &merged).await?;
    super::laya_identity::spawn(ctx, run_id, merged.clone());

    // Auditoria de conflitos/clones de IP e MAC após a varredura
    if let Ok(conflicts) = super::conflicts::analyze_network_conflicts(&ctx.db).await {
        if !conflicts.is_empty() {
            let _ = super::conflicts::alert_on_conflicts(ctx, &conflicts).await;
        }
    }

    Ok(merged)
}

/// Execução sem persistência usada pelo agente remoto. Os mesmos scanners e
/// limites são reutilizados; somente o servidor central grava a run.
pub async fn scan_network(
    ctx: &AppContext,
    cidr: &str,
    cancel: CancellationToken,
) -> AppResult<Vec<DiscoveredHost>> {
    scan_network_with(&CheckDeps::from_context(ctx), cidr, cancel).await
}

/// Mesma varredura de [`scan_network`], para quem não tem `AppContext` (agente).
pub async fn scan_network_with(
    deps: &CheckDeps,
    cidr: &str,
    cancel: CancellationToken,
) -> AppResult<Vec<DiscoveredHost>> {
    pipeline::scan(
        &deps.ping_client()?,
        &ScanTarget::new(cidr),
        cancel,
        &ScanReporter::silent(),
    )
    .await
}

/// Valida e finaliza o resultado devolvido por um probe. O vínculo da run com
/// o probe impede um agente de gravar dados na execução de outro.
pub async fn complete_remote_discovery(
    ctx: &AppContext,
    probe_id: i64,
    run_id: i64,
    hosts: &[DiscoveredHost],
    error: Option<&str>,
) -> AppResult<()> {
    let run = discovery_runs::Entity::find_by_id(run_id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Execução de discovery não encontrada"))?;
    if run.probe_id != Some(probe_id) {
        return Err(AppError::unauthorized(
            "A execução de discovery pertence a outro probe",
        ));
    }
    if run.status == "cancelled" || run.status == "completed" {
        return Ok(());
    }

    if error.is_none() {
        persist_results(&ctx.db, run_id, hosts).await?;
        super::laya_identity::spawn(ctx, run_id, hosts.to_vec());
    }
    discovery_runs::ActiveModel {
        id: Set(run_id),
        status: Set(if error.is_some() {
            "failed"
        } else {
            "completed"
        }
        .into()),
        finished_at: Set(Some(Utc::now().into())),
        error: Set(error.map(ToString::to_string)),
        ..Default::default()
    }
    .update(&ctx.db)
    .await?;

    let session = ScanSessionService::from_context(ctx)?;
    if session.state().await.run_id == Some(run_id) {
        if error.is_none() {
            session.hosts(hosts).await;
        }
        session.finish(error.map(ToString::to_string)).await;
    }
    publish_run_event(
        ctx,
        if error.is_some() {
            "discovery:failed"
        } else {
            "discovery:completed"
        },
        serde_json::json!({
            "runId": run_id,
            "networkId": run.network_id,
            "devicesFound": hosts.len(),
            "error": error,
            "message": error.map_or_else(
                || format!("Varredura remota concluída: {} dispositivo(s)", hosts.len()),
                |error| format!("Varredura remota falhou: {error}"),
            ),
        }),
    )
    .await;
    Ok(())
}

/// Aplica na sessão o que os scanners relatam, em quadros de no máximo
/// [`PUBLISH_INTERVAL`]: os eventos que chegam no intervalo viram um quadro só.
async fn pump_events(
    session: ScanSessionService,
    mut events: tokio::sync::mpsc::UnboundedReceiver<ScanEvent>,
) {
    while let Some(first) = events.recv().await {
        let mut batch = vec![first];
        while let Ok(next) = events.try_recv() {
            batch.push(next);
        }
        session.apply(batch).await;
        tokio::time::sleep(PUBLISH_INTERVAL).await;
    }
}

async fn persist_results(
    db: &sea_orm::DatabaseConnection,
    run_id: i64,
    hosts: &[DiscoveredHost],
) -> AppResult<()> {
    // discovery_results é o cache da última execução desta run; reexecuções não
    // acumulam entradas antigas que já não existem na rede.
    discovery_results::Entity::delete_many()
        .filter(ResultColumn::DiscoveryRunId.eq(run_id))
        .exec(db)
        .await?;
    let now = Utc::now();
    let rows: Vec<_> = hosts
        .iter()
        .map(|host| discovery_results::ActiveModel {
            discovery_run_id: Set(run_id),
            ip_address: Set(host.ip_address.clone()),
            mac_address: Set(host.mac_address.clone()),
            hostname: Set(host.hostname.clone()),
            mdns_name: Set(host.mdns_name.clone()),
            vendor: Set(host.vendor.clone()),
            device_type: Set(host.device_type.clone()),
            confidence: Set(host.confidence),
            data: Set(Some(
                serde_json::json!({ "openPorts": host.open_ports, "details": host.data }),
            )),
            first_seen_at: Set(now.into()),
            last_seen_at: Set(now.into()),
            ..Default::default()
        })
        .collect();
    // Em lotes: o SQLite limita as variáveis por comando.
    for chunk in rows.chunks(50) {
        discovery_results::Entity::insert_many(chunk.to_vec())
            .exec(db)
            .await?;
    }
    Ok(())
}

/// Uma linha do histórico de varreduras.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSummary {
    pub id: i64,
    pub network_id: i64,
    pub network_name: Option<String>,
    pub probe_id: Option<i64>,
    pub status: String,
    pub devices_found: i64,
    pub cidr: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub error: Option<String>,
}

/// Histórico de varreduras, da mais recente para a mais antiga, com a
/// contagem de resultados numa consulta agrupada (não uma por run).
///
/// # Errors
///
/// Falha de banco.
pub async fn list_runs<C: ConnectionTrait>(db: &C) -> AppResult<Vec<RunSummary>> {
    let runs = discovery_runs::Entity::find()
        .order_by_desc(RunColumn::Id)
        .all(db)
        .await?;
    let counts: HashMap<i64, i64> = discovery_results::Entity::find()
        .select_only()
        .column(ResultColumn::DiscoveryRunId)
        .column_as(ResultColumn::Id.count(), "found")
        .group_by(ResultColumn::DiscoveryRunId)
        .into_tuple::<(i64, i64)>()
        .all(db)
        .await?
        .into_iter()
        .collect();
    let names: HashMap<i64, String> = networks::Entity::find()
        .all(db)
        .await?
        .into_iter()
        .map(|network| (network.id, network.name))
        .collect();
    Ok(runs
        .into_iter()
        .map(|run| RunSummary {
            id: run.id,
            network_id: run.network_id,
            network_name: names.get(&run.network_id).cloned(),
            probe_id: run.probe_id,
            status: run.status,
            devices_found: counts.get(&run.id).copied().unwrap_or(0),
            cidr: run
                .configuration
                .as_ref()
                .and_then(|value| value.get("cidr"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
            started_at: run.started_at.to_rfc3339(),
            finished_at: run.finished_at.map(|value| value.to_rfc3339()),
            error: run.error,
        })
        .collect())
}

/// Uma execução com os resultados gravados.
///
/// # Errors
///
/// [`AppError::NotFound`] quando a run não existe.
pub async fn run_details<C: ConnectionTrait>(db: &C, id: i64) -> AppResult<serde_json::Value> {
    let run = discovery_runs::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::not_found("Execução de discovery não encontrada"))?;
    let results = discovery_results::Entity::find()
        .filter(ResultColumn::DiscoveryRunId.eq(id))
        .all(db)
        .await?;
    Ok(serde_json::json!({
        "id": run.id,
        "networkId": run.network_id,
        "status": run.status,
        "configuration": run.configuration,
        "results": results,
    }))
}

/// Apaga as runs criadas há mais de `days` dias (mínimo 1).
///
/// # Errors
///
/// Falha de banco.
pub async fn cleanup_runs<C: ConnectionTrait>(db: &C, days: i64) -> AppResult<u64> {
    let result = discovery_runs::Entity::delete_many()
        .filter(RunColumn::CreatedAt.lt(Utc::now() - ChronoDuration::days(days.max(1))))
        .exec(db)
        .await?;
    Ok(result.rows_affected)
}
