//! Orquestração do discovery: etapas isoladas, cancelamento cooperativo e
//! persistência do cache de resultados da execução.

use chrono::Utc;
use loco_rs::app::AppContext;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio_util::sync::CancellationToken;

use crate::{
    models::discovery_results,
    services::{
        discovery::{
            cidr_range::expand_cidr,
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
    let hosts = expand_cidr(cidr, 1024)?;
    reporter.phase("icmp", 0, hosts.len());
    let ping = PingClient::from_context(ctx)?;
    let icmp_hosts = icmp::scan(&ping, &hosts, reporter).await?;
    reporter.phase("discovery", icmp_hosts.len(), hosts.len());
    // Os três coletores de descoberta são independentes e best-effort.
    let (arp_hosts, mdns_hosts, ssdp_hosts) =
        tokio::join!(arp::scan(&hosts), mdns::scan(), ssdp::scan());
    let mut merged = merge_hosts([icmp_hosts, arp_hosts, mdns_hosts, ssdp_hosts]);
    reporter.hosts(&merged);
    if !cancel.is_cancelled() {
        merged = merge_hosts([ports::enrich(merged, cancel.clone(), reporter).await]);
        reporter.hosts(&merged);
    }
    if cancel.is_cancelled() {
        return Err(AppError::BusinessRule("Varredura cancelada.".into()));
    }
    reporter.phase("snmp", 0, merged.len());
    Ok(merge_hosts([snmp::enrich(merged, reporter).await]))
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
