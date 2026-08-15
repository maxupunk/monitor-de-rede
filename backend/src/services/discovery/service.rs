//! Orquestração do discovery: etapas isoladas, cancelamento cooperativo e
//! persistência do cache de resultados da execução.

use chrono::Utc;
use loco_rs::app::AppContext;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, RwLock};
use tokio_util::sync::CancellationToken;

use crate::{
    models::{discovery_results, discovery_runs},
    services::{
        discovery::{
            cidr_range::{expand_cidr_batch, parse_cidr_range, MAX_SCAN_HOSTS},
            merger::{merge_hosts, DiscoveredHost},
            progress::{ScanEvent, ScanReporter},
            scanners::{arp, icmp, mdns, ports, snmp, ssdp},
        },
        monitoring::checkers::ping::PingClient,
        shared::errors::{AppError, AppResult},
    },
};

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
            phase: "idle".into(),
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

#[derive(Clone)]
pub struct ScanSessionService {
    state: Arc<RwLock<ScanSessionState>>,
    updates: broadcast::Sender<ScanSessionState>,
    cancel: Arc<RwLock<Option<CancellationToken>>>,
}
impl ScanSessionService {
    #[must_use]
    pub fn create() -> Self {
        // Uma varredura publica dezenas de atualizações por fase; um buffer
        // curto faria o assinante do SSE ficar para trás logo no sweep ICMP.
        let (updates, _) = broadcast::channel(256);
        Self {
            state: Arc::new(RwLock::new(ScanSessionState::default())),
            updates,
            cancel: Arc::new(RwLock::new(None)),
        }
    }
    pub fn from_context(ctx: &AppContext) -> AppResult<Self> {
        ctx.shared_store.get::<Self>().ok_or_else(|| {
            AppError::Internal(anyhow::anyhow!("Sessão de discovery não inicializada"))
        })
    }
    pub fn subscribe(&self) -> broadcast::Receiver<ScanSessionState> {
        self.updates.subscribe()
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
            phase: "icmp".into(),
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
        if state.status == "running" {
            state.status = "cancelled".into();
            state.finished_at = Some(Utc::now().to_rfc3339());
            state.logs.push("Varredura cancelada.".into());
            self.publish(&state);
        }
    }
    pub async fn wait_for_probe(&self) {
        let mut state = self.state.write().await;
        state.status = "pending".into();
        state.phase = "probe".into();
        state
            .logs
            .push("Aguardando execução pelo probe remoto.".into());
        self.publish(&state);
    }
    pub async fn remote_started(&self, run_id: i64) {
        let mut state = self.state.write().await;
        if state.run_id == Some(run_id) && state.status == "pending" {
            state.status = "running".into();
            state.phase = "probe".into();
            state.logs.push("Probe remoto iniciou a varredura.".into());
            self.publish(&state);
        }
    }
    pub async fn progress(&self, phase: &str, current: usize, total: usize) {
        let mut state = self.state.write().await;
        state.phase = phase.into();
        state.progress_current = current;
        state.progress_total = total;
        self.publish(&state);
    }
    pub async fn hosts(&self, hosts: &[DiscoveredHost]) {
        let mut state = self.state.write().await;
        state.hosts = hosts.to_vec();
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
            state.phase = "idle".into();
            let completed = state.status == "completed";
            state.logs.push(if completed {
                "Varredura finalizada.".into()
            } else {
                "Varredura encerrada.".into()
            });
            self.publish(&state);
        }
        *self.cancel.write().await = None;
    }
    fn publish(&self, state: &ScanSessionState) {
        let _ = self.updates.send(state.clone());
    }
}

pub async fn run_discovery(
    ctx: &AppContext,
    cidr: &str,
    run_id: i64,
    cancel: CancellationToken,
) -> AppResult<Vec<DiscoveredHost>> {
    let session = ScanSessionService::from_context(ctx)?;
    let (reporter, events) = ScanReporter::channel();
    let pump = tokio::spawn(pump_events(session.clone(), events));
    let outcome = scan_phases(ctx, cidr, cancel, &reporter).await;
    // Derrubar o repórter fecha o canal e encerra o pump: só depois disso o
    // estado publicado é o final, e não uma atualização atrasada de fase.
    drop(reporter);
    let _ = pump.await;

    let merged = outcome?;
    session.hosts(&merged).await;
    persist_results(&ctx.db, run_id, &merged).await?;
    Ok(merged)
}

/// Execução sem persistência usada pelo agente remoto. Os mesmos scanners e
/// limites são reutilizados; somente o servidor central grava a run.
pub async fn scan_network(
    ctx: &AppContext,
    cidr: &str,
    cancel: CancellationToken,
) -> AppResult<Vec<DiscoveredHost>> {
    scan_phases(ctx, cidr, cancel, &ScanReporter::silent()).await
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
    Ok(())
}

/// Aplica na sessão o que os scanners relatam, na ordem em que chega.
async fn pump_events(
    session: ScanSessionService,
    mut events: tokio::sync::mpsc::UnboundedReceiver<ScanEvent>,
) {
    while let Some(event) = events.recv().await {
        match event {
            ScanEvent::Progress {
                phase,
                current,
                total,
            } => session.progress(phase, current, total).await,
            ScanEvent::Hosts(hosts) => session.hosts(&hosts).await,
        }
    }
}

async fn scan_phases(
    ctx: &AppContext,
    cidr: &str,
    cancel: CancellationToken,
    reporter: &ScanReporter,
) -> AppResult<Vec<DiscoveredHost>> {
    let range = parse_cidr_range(cidr)?;
    let total = range.usable_hosts as usize;
    reporter.phase("discovery", 0, total);
    let ping = PingClient::from_context(ctx)?;
    // Multicast pertence à interface, não a um lote do CIDR: executá-lo uma vez
    // evita respostas duplicadas em faixas grandes.
    let (mdns_hosts, ssdp_hosts) = tokio::join!(mdns::scan(), ssdp::scan());
    let mut merged = merge_hosts([mdns_hosts, ssdp_hosts]);
    let mut offset = 0_u32;

    while offset < range.usable_hosts {
        let batch_started = Instant::now();
        if cancel.is_cancelled() {
            return Err(AppError::BusinessRule("Varredura cancelada.".into()));
        }
        let addresses = expand_cidr_batch(cidr, offset, MAX_SCAN_HOSTS as usize)?;
        if addresses.is_empty() {
            break;
        }
        let completed = (offset as usize + addresses.len()).min(total);

        reporter.phase("icmp", offset as usize, total);
        let phase_started = Instant::now();
        let icmp_hosts =
            icmp::scan(&ping, &addresses, cancel.clone(), &ScanReporter::silent()).await?;
        tracing::info!(
            phase = "icmp",
            cidr,
            offset,
            tested = addresses.len(),
            found = icmp_hosts.len(),
            duration_ms = phase_started.elapsed().as_millis(),
            "fase de discovery concluída"
        );
        reporter.progress("icmp", completed, total);

        reporter.phase("discovery", offset as usize, total);
        let phase_started = Instant::now();
        let arp_hosts = arp::scan(&addresses).await;
        tracing::info!(
            phase = "neighbors",
            cidr,
            offset,
            found = arp_hosts.len(),
            duration_ms = phase_started.elapsed().as_millis(),
            "fase de discovery concluída"
        );
        merged = merge_hosts([merged, icmp_hosts, arp_hosts]);

        // Equivalente seguro ao `-Pn`: portas-chave são testadas em todo IP,
        // mesmo quando ICMP e ARP não produziram resposta.
        reporter.phase("ports", offset as usize, total);
        let candidates: Vec<_> = addresses
            .iter()
            .map(|ip| DiscoveredHost {
                ip_address: ip.to_string(),
                data: serde_json::json!({ "scanner": "tcp-connect" }),
                ..Default::default()
            })
            .collect();
        let phase_started = Instant::now();
        let mut port_hosts =
            ports::enrich(candidates.clone(), cancel.clone(), &ScanReporter::silent()).await;
        port_hosts.retain(|host| !host.open_ports.is_empty());
        let port_host_count = port_hosts.len();
        merged = merge_hosts([merged, port_hosts]);
        reporter.progress("ports", completed, total);
        tracing::info!(
            phase = "ports",
            cidr,
            offset,
            found = port_host_count,
            duration_ms = phase_started.elapsed().as_millis(),
            "fase de discovery concluída"
        );

        reporter.phase("snmp", offset as usize, total);
        let phase_started = Instant::now();
        let mut snmp_hosts =
            snmp::enrich(candidates, cancel.clone(), &ScanReporter::silent()).await;
        snmp_hosts.retain(|host| host.data.get("snmp").is_some());
        let snmp_host_count = snmp_hosts.len();
        merged = merge_hosts([merged, snmp_hosts]);
        reporter.progress("snmp", completed, total);
        reporter.hosts(&merged);
        tracing::info!(
            phase = "snmp",
            cidr,
            offset,
            found = snmp_host_count,
            duration_ms = phase_started.elapsed().as_millis(),
            batch_duration_ms = batch_started.elapsed().as_millis(),
            "fase de discovery concluída"
        );

        offset = offset.saturating_add(addresses.len() as u32);
    }

    Ok(merge_hosts([merged]))
}

async fn persist_results(
    db: &sea_orm::DatabaseConnection,
    run_id: i64,
    hosts: &[DiscoveredHost],
) -> AppResult<()> {
    // discovery_results é o cache da última execução desta run; reexecuções não
    // acumulam entradas antigas que já não existem na rede.
    discovery_results::Entity::delete_many()
        .filter(crate::models::_entities::discovery_results::Column::DiscoveryRunId.eq(run_id))
        .exec(db)
        .await?;
    for host in hosts {
        let now = Utc::now();
        discovery_results::ActiveModel {
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
        }
        .insert(db)
        .await?;
    }
    Ok(())
}
