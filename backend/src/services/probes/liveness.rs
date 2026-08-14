//! Vida dos probes (§8.11).
//!
//! O heartbeat marca `online`, mas nada marcava o caminho de volta: um agente
//! derrubado continuava aparecendo como `online` para sempre, e as tarefas
//! despachadas para ele sumiam sem deixar rastro. Aqui a verdade vem de
//! `last_seen_at`, não do campo `status`.

use chrono::Utc;
use loco_rs::app::AppContext;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

use crate::{
    models::probes,
    services::{events::EventBus, shared::errors::AppResult},
};

/// Silêncio tolerado antes de dar o probe como fora do ar.
///
/// O agente bate a cada `PROBE_INTERVAL_MS` (5 s por padrão), então a folga
/// cobre com sobra tanto o ritmo padrão quanto intervalos bem mais largos.
pub const PROBE_OFFLINE_AFTER_SECONDS: i64 = 90;

/// Status considerados "em serviço" — só eles viram `offline` no watchdog.
const ACTIVE_STATUSES: [&str; 2] = ["online", "busy"];

pub const STATUS_OFFLINE: &str = "offline";
pub const STATUS_ONLINE: &str = "online";

/// Um probe só recebe tarefa se realmente estiver batendo o heartbeat.
#[must_use]
pub fn is_probe_alive(probe: Option<&probes::Model>) -> bool {
    let Some(probe) = probe else {
        return false;
    };
    if probe.status == probes::STATUS_REVOKED {
        return false;
    }
    probe.last_seen_at.is_some_and(|seen| {
        (Utc::now() - seen.with_timezone(&Utc)).num_seconds() <= PROBE_OFFLINE_AFTER_SECONDS
    })
}

/// Payload de `probe:status` — o frontend (`stores/probes.ts`) lê estes nomes.
#[must_use]
pub fn status_payload(probe: &probes::Model) -> serde_json::Value {
    serde_json::json!({
        "id": probe.id,
        "probeId": probe.id,
        "name": probe.name,
        "status": probe.status,
        "version": probe.version,
        "lastSeenAt": probe.last_seen_at.map(|value| value.to_rfc3339()),
    })
}

/// Marca como `offline` os probes que pararam de bater o heartbeat e publica a
/// transição.
///
/// É o que faz a tela de probes contar a verdade e explicar por que os
/// monitores daquele agente pararam. Devolve quantos mudaram de estado.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn mark_stale_probes_offline(ctx: &AppContext) -> AppResult<u64> {
    let candidates = probes::Entity::find()
        .filter(probes::Column::Status.is_in(ACTIVE_STATUSES))
        .all(&ctx.db)
        .await?;
    let mut changed = 0;

    for probe in candidates {
        if is_probe_alive(Some(&probe)) {
            continue;
        }
        let mut active: probes::ActiveModel = probe.into();
        active.status = Set(STATUS_OFFLINE.into());
        let saved = active.update(&ctx.db).await?;
        changed += 1;

        if let Ok(bus) = EventBus::from_context(ctx) {
            if let Err(error) = bus
                .publish(&ctx.db, "probe:status", status_payload(&saved))
                .await
            {
                tracing::warn!(%error, probe_id = saved.id, "falha ao publicar probe:status");
            }
        }
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn probe(status: &str, last_seen_at: Option<chrono::DateTime<Utc>>) -> probes::Model {
        let now = Utc::now().into();
        probes::Model {
            id: 1,
            site_id: None,
            name: "probe-lan".into(),
            token_hash: "hash".into(),
            status: status.into(),
            version: Some("1.0.0".into()),
            last_seen_at: last_seen_at.map(Into::into),
            registered_at: None,
            revoked_at: None,
            configuration: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn probe_sem_heartbeat_nunca_esta_vivo() {
        assert!(!is_probe_alive(None));
        assert!(!is_probe_alive(Some(&probe(STATUS_ONLINE, None))));
    }

    #[test]
    fn a_janela_de_tolerancia_e_de_noventa_segundos() {
        let dentro = Utc::now() - Duration::seconds(PROBE_OFFLINE_AFTER_SECONDS - 1);
        let fora = Utc::now() - Duration::seconds(PROBE_OFFLINE_AFTER_SECONDS + 1);
        assert!(is_probe_alive(Some(&probe(STATUS_ONLINE, Some(dentro)))));
        assert!(!is_probe_alive(Some(&probe(STATUS_ONLINE, Some(fora)))));
    }

    #[test]
    fn probe_revogado_nunca_recebe_tarefa_mesmo_batendo_heartbeat() {
        let agora = Utc::now();
        assert!(!is_probe_alive(Some(&probe(
            probes::STATUS_REVOKED,
            Some(agora)
        ))));
    }

    #[test]
    fn o_payload_do_evento_usa_os_nomes_que_o_frontend_le() {
        let payload = status_payload(&probe(STATUS_OFFLINE, None));
        assert_eq!(payload["probeId"], 1);
        assert_eq!(payload["status"], "offline");
        assert_eq!(payload["lastSeenAt"], serde_json::Value::Null);
    }
}
