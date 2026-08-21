//! Execução de rotinas periódicas de manutenção, tráfego VPN, outbox de notificações e retenção.

use chrono::Utc;
use loco_rs::app::AppContext;

use super::cadence::{
    is_due, DATA_PRUNE_INTERVAL_SECONDS, ROLLUP_INTERVAL_SECONDS, VPN_STATUS_INTERVAL_SECONDS,
    VPN_TRAFFIC_INTERVAL_SECONDS,
};
use crate::services::{
    alerts::hysteresis, maintenance::data_pruner, monitoring::rollup, notifications::outbox,
    shared::errors::AppResult, syslog, vpn::traffic_recorder,
};

/// Sincroniza o status (10s) e histórico (30s) do tráfego dos túneis VPN.
pub async fn sync_vpn_traffic_if_due(ctx: &AppContext) -> AppResult<()> {
    let now = Utc::now();
    if is_due("vpn_traffic", VPN_TRAFFIC_INTERVAL_SECONDS, now) {
        traffic_recorder::record_all(ctx).await?;
        is_due("vpn_status", VPN_STATUS_INTERVAL_SECONDS, now);
        return Ok(());
    }
    if is_due("vpn_status", VPN_STATUS_INTERVAL_SECONDS, now) {
        traffic_recorder::sync_all(ctx).await?;
    }
    Ok(())
}

/// Despacha notificações pendentes no outbox respeitando agrupamento e higiene.
pub async fn dispatch_notifications(ctx: &AppContext) -> AppResult<()> {
    let stats = outbox::dispatch_pending(ctx).await?;
    if stats.total() > 0 {
        tracing::debug!(
            entregues = stats.delivered,
            agrupadas = stats.consolidated,
            suprimidas = stats.suppressed,
            "notificações despachadas"
        );
    }
    Ok(())
}

/// Executa a purga periódica de dados antigos, retenção de logs e varredura de histerese ociosa.
pub async fn run_data_pruner_if_due(ctx: &AppContext) -> AppResult<()> {
    if !is_due("data_pruner", DATA_PRUNE_INTERVAL_SECONDS, Utc::now()) {
        return Ok(());
    }
    let stats = data_pruner::prune_all(&ctx.db).await?;
    if stats.total() > 0 {
        tracing::info!(
            outbox = stats.outbox_deleted,
            resultados = stats.results_deleted,
            metricas = stats.metrics_deleted,
            descoberta = stats.discovery_deleted,
            alertas = stats.alert_events_deleted,
            notificacoes = stats.notifications_deleted,
            "purga de dados antigos executada"
        );
    }
    if let Ok(logs) = syslog::LogsDb::from_context(ctx) {
        match syslog::retention::prune(logs.connection()).await {
            Ok(stats) if stats.total() > 0 => tracing::info!(
                por_idade = stats.by_age,
                por_tamanho = stats.by_size,
                bytes = stats.bytes_after,
                "purga do banco de logs executada"
            ),
            Ok(_) => {}
            Err(error) => tracing::warn!(%error, "falha ao purgar o banco de logs"),
        }
    }
    let esquecidas = hysteresis::sweep(
        Utc::now(),
        chrono::Duration::hours(hysteresis::IDLE_TTL_HOURS),
    );
    if esquecidas > 0 {
        tracing::debug!(esquecidas, "contagens de histerese ociosas descartadas");
    }
    Ok(())
}

/// Executa o rollup horário dos resultados brutos quando vence a cadência.
///
/// A tarefa roda no mesmo ciclo do scheduler, mas só age a cada hora (ou quando
/// `ROLLUP_INTERVAL_SECONDS` for menor em ambiente de teste). O fechamento do
/// bucket exclui a hora em curso, garantindo que a agregação seja completa.
pub async fn rollup_monitor_results_if_due(ctx: &AppContext) -> AppResult<()> {
    let now = Utc::now();
    if !is_due("monitor_rollup", ROLLUP_INTERVAL_SECONDS, now) {
        return Ok(());
    }
    let stats = rollup::rollup_monitor_results(&ctx.db, now).await?;
    if stats.total() > 0 {
        tracing::info!(
            buckets = stats.buckets_upserted,
            agregadas = stats.rows_aggregated,
            apagadas = stats.rows_deleted,
            "rollup de resultados de monitoramento executado"
        );
    }
    Ok(())
}
