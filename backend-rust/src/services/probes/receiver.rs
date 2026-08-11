//! Recepção dos resultados reportados pelos probes (§8.11).

use loco_rs::app::AppContext;
use serde::Deserialize;

use crate::services::{
    monitoring::{contracts::CheckResult, result_processor::process_result},
    shared::errors::AppResult,
};

/// Um item de `POST /api/probes/results`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeResultPayload {
    pub monitor_id: i64,
    pub task_id: Option<String>,
    pub result: CheckResult,
}

/// Processa um lote inteiro.
///
/// Uma falha por item é registrada e o laço segue: um resultado malformado — ou
/// um monitor apagado entre o despacho e o retorno — não pode descartar os
/// outros resultados do mesmo lote, que já custaram uma volta de rede.
///
/// # Errors
///
/// Nunca falha por causa de um item; devolve `Err` só se o próprio contexto
/// estiver inutilizável.
pub async fn receive_batch_results(
    ctx: &AppContext,
    probe_id: i64,
    payloads: &[ProbeResultPayload],
) -> AppResult<usize> {
    let mut processed = 0;
    for item in payloads {
        match process_result(ctx, item.monitor_id, &item.result, Some(probe_id)).await {
            Ok(Some(_)) => processed += 1,
            Ok(None) => {
                tracing::debug!(
                    monitor_id = item.monitor_id,
                    probe_id,
                    "resultado descartado: monitor não existe mais"
                );
            }
            Err(error) => {
                tracing::warn!(
                    %error,
                    monitor_id = item.monitor_id,
                    probe_id,
                    "erro ao processar resultado do probe"
                );
            }
        }
    }
    Ok(processed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn o_payload_do_agente_e_lido_em_camel_case() {
        let payload: ProbeResultPayload = serde_json::from_value(json!({
            "monitorId": 7,
            "taskId": "task-7-1",
            "result": {
                "success": true,
                "status": "up",
                "startedAt": "2026-08-11T10:00:00Z",
                "finishedAt": "2026-08-11T10:00:01Z",
                "durationMs": 1000,
                "message": "ok",
                "metrics": [{ "name": "latency", "value": 12.5, "unit": "ms" }],
                "data": {}
            }
        }))
        .expect("payload do agente");
        assert_eq!(payload.monitor_id, 7);
        assert_eq!(payload.task_id.as_deref(), Some("task-7-1"));
        assert_eq!(payload.result.metrics[0].value, 12.5);
    }

    #[test]
    fn task_id_e_opcional() {
        let payload: ProbeResultPayload = serde_json::from_value(json!({
            "monitorId": 7,
            "result": {
                "success": false,
                "status": "down",
                "startedAt": "2026-08-11T10:00:00Z",
                "finishedAt": "2026-08-11T10:00:01Z",
                "durationMs": 0,
                "message": null,
                "metrics": [],
                "data": {}
            }
        }))
        .expect("payload sem taskId");
        assert!(payload.task_id.is_none());
    }
}
