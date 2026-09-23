//! O que a central faz com os eventos espontâneos de um agente.
//!
//! Snapshots Docker viram eventos SSE com a chave do host (e ficam guardados
//! para quem assinar depois). Rollups vão para o histórico. Resultados que
//! ficaram presos no buffer offline do agente entram pelo mesmo `receiver` do
//! protocolo HTTP — com a mesma checagem de que o monitor é deste probe.

use chrono::{DateTime, Duration, Utc};
use loco_rs::app::AppContext;

use crate::services::{
    docker::hosts::HostKey,
    events::EventBus,
    probes::receiver::{self, ProbeResultPayload},
    telemetry::{
        rollup::{bucket_of, MetricsRollup},
        store,
    },
};

use super::protocol::AgentEvent;

/// Relógio do agente adiantado além disto é descartado a favor do relógio da
/// central: um minuto "no futuro" ficaria no topo do gráfico para sempre.
const MAX_CLOCK_SKEW_MINUTES: i64 = 5;

/// Balde confiável: o do agente, salvo quando está no futuro.
#[must_use]
pub fn sanitize_bucket(bucket: DateTime<Utc>, now: DateTime<Utc>) -> DateTime<Utc> {
    if bucket > now + Duration::minutes(MAX_CLOCK_SKEW_MINUTES) {
        bucket_of(now)
    } else {
        bucket
    }
}

pub async fn handle(ctx: &AppContext, probe_id: i64, event: AgentEvent) {
    let host_key = HostKey::agent(probe_id).to_string();
    match event {
        AgentEvent::DockerLive { mut snapshot } => {
            snapshot.host_key.clone_from(&host_key);
            publish(
                ctx,
                "docker:snapshot",
                &host_key,
                serde_json::to_value(snapshot),
            );
        }
        AgentEvent::DockerInventory { mut snapshot } => {
            snapshot.host_key.clone_from(&host_key);
            publish(
                ctx,
                "docker:inventory",
                &host_key,
                serde_json::to_value(snapshot),
            );
        }
        AgentEvent::MetricsRollup { rollup } => {
            save_rollup(ctx, &host_key, *rollup).await;
        }
        AgentEvent::MonitorResult {
            monitor_id,
            task_id,
            result,
        } => {
            let payload = ProbeResultPayload {
                monitor_id,
                task_id,
                result: *result,
            };
            if let Err(error) = receiver::receive_batch_results(ctx, probe_id, &[payload]).await {
                tracing::warn!(%error, probe_id, "falha ao processar resultado reenviado pelo agente");
            }
        }
        AgentEvent::DiscoveryResult { result } => {
            if let Err(error) = receiver::receive_discovery_results(ctx, probe_id, &[*result]).await
            {
                tracing::warn!(%error, probe_id, "falha ao processar discovery reenviado pelo agente");
            }
        }
    }
}

async fn save_rollup(ctx: &AppContext, host_key: &str, mut rollup: MetricsRollup) {
    rollup.bucket_at = sanitize_bucket(rollup.bucket_at, Utc::now());
    if let Err(error) = store::save(&ctx.db, host_key, &rollup).await {
        tracing::warn!(%error, host = host_key, "falha ao gravar rollup do agente");
    }
}

fn publish(
    ctx: &AppContext,
    event_type: &str,
    host_key: &str,
    payload: Result<serde_json::Value, serde_json::Error>,
) {
    let (Ok(bus), Ok(payload)) = (EventBus::from_context(ctx), payload) else {
        return;
    };
    bus.publish_snapshot(event_type, host_key, payload);
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn relogio_adiantado_cai_no_minuto_da_central() {
        let now = Utc
            .with_ymd_and_hms(2026, 9, 22, 12, 0, 30)
            .single()
            .expect("agora");
        let past = now - Duration::hours(3);
        assert_eq!(
            sanitize_bucket(past, now),
            past,
            "buffer atrasado é legítimo"
        );
        assert_eq!(
            sanitize_bucket(now + Duration::minutes(2), now),
            now + Duration::minutes(2)
        );
        assert_eq!(
            sanitize_bucket(now + Duration::hours(1), now),
            bucket_of(now)
        );
    }
}
