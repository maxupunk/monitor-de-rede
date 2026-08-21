//! Recepção dos resultados reportados pelos probes (§8.11).

use loco_rs::app::AppContext;
use sea_orm::EntityTrait;
use serde::Deserialize;

use crate::{
    models::{discovery_runs, monitors},
    services::{
        monitoring::{contracts::CheckResult, result_processor::process_result},
        shared::errors::{AppError, AppResult},
    },
};

/// Um item de `POST /api/probes/results`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeResultPayload {
    pub monitor_id: i64,
    pub task_id: Option<String>,
    pub result: CheckResult,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeDiscoveryResultPayload {
    pub run_id: i64,
    pub task_id: Option<String>,
    #[serde(default)]
    pub hosts: Vec<crate::services::discovery::merger::DiscoveredHost>,
    pub error: Option<String>,
}

/// Verifica se o monitor informado pertence ao probe autenticado.
///
/// Um agente só pode reportar resultados dos monitores que lhe foram
/// explicitamente atribuídos (`monitors.probe_id`). Qualquer outro `monitor_id`
/// é descartado com log de segurança — isso fecha a janela de um probe com
/// token padrão injetar resultados falsos em monitores alheios.
async fn assert_monitor_belongs_to_probe(
    ctx: &AppContext,
    monitor_id: i64,
    probe_id: i64,
) -> AppResult<()> {
    let Some(monitor) = monitors::Entity::find_by_id(monitor_id)
        .one(&ctx.db)
        .await?
    else {
        return Err(AppError::not_found("monitor não encontrado"));
    };
    if monitor.probe_id != Some(probe_id) {
        return Err(AppError::unauthorized("monitor não pertence a este probe"));
    }
    Ok(())
}

/// Verifica se a run de discovery informada pertence ao probe autenticado.
async fn assert_discovery_run_belongs_to_probe(
    ctx: &AppContext,
    run_id: i64,
    probe_id: i64,
) -> AppResult<()> {
    let Some(run) = discovery_runs::Entity::find_by_id(run_id)
        .one(&ctx.db)
        .await?
    else {
        return Err(AppError::not_found("run de discovery não encontrada"));
    };
    if run.probe_id != Some(probe_id) {
        return Err(AppError::unauthorized(
            "run de discovery não pertence a este probe",
        ));
    }
    Ok(())
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
        if let Err(error) = assert_monitor_belongs_to_probe(ctx, item.monitor_id, probe_id).await {
            tracing::warn!(
                %error,
                monitor_id = item.monitor_id,
                probe_id,
                "resultado de probe rejeitado: monitor não pertence ao probe"
            );
            continue;
        }
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

pub async fn receive_discovery_results(
    ctx: &AppContext,
    probe_id: i64,
    payloads: &[ProbeDiscoveryResultPayload],
) -> AppResult<usize> {
    let mut processed = 0;
    for item in payloads {
        if let Err(error) = assert_discovery_run_belongs_to_probe(ctx, item.run_id, probe_id).await
        {
            tracing::warn!(
                %error,
                run_id = item.run_id,
                probe_id,
                "resultado de discovery rejeitado: run não pertence ao probe"
            );
            continue;
        }
        match crate::services::discovery::service::complete_remote_discovery(
            ctx,
            probe_id,
            item.run_id,
            &item.hosts,
            item.error.as_deref(),
        )
        .await
        {
            Ok(()) => processed += 1,
            Err(error) => tracing::warn!(
                %error,
                run_id = item.run_id,
                probe_id,
                "erro ao processar discovery do probe"
            ),
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

    #[test]
    fn resultado_de_discovery_le_hosts_em_camel_case() {
        let payload: ProbeDiscoveryResultPayload = serde_json::from_value(json!({
            "runId": 8,
            "taskId": "discovery-8",
            "hosts": [{
                "ipAddress": "10.8.0.2",
                "macAddress": null,
                "hostname": null,
                "mdnsName": null,
                "vendor": null,
                "deviceType": null,
                "openPorts": [22],
                "confidence": 20,
                "data": {}
            }],
            "error": null
        }))
        .expect("resultado discovery");
        assert_eq!(payload.run_id, 8);
        assert_eq!(payload.hosts[0].open_ports, vec![22]);
    }
}
