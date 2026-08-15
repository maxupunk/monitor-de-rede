//! Fila de tarefas dos probes, persistida em `probe_tasks` (§8.11).
//!
//! Precisa atravessar processos: quem enfileira é o `scheduler` e quem entrega
//! é o processo HTTP. Ver a migration `probe_tasks` para o histórico.

use chrono::{Duration, Utc};
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use serde::{Deserialize, Serialize};

use crate::{
    models::{
        _entities::{discovery_runs as discovery_runs_entity, probe_tasks as probe_tasks_entity},
        discovery_runs, probe_tasks,
    },
    services::shared::errors::AppResult,
};

/// Tempo máximo que uma tarefa enfileirada ainda vale.
///
/// Uma checagem que ficou parada porque o probe estava fora do ar não descreve
/// mais o presente: executá-la produziria um resultado carimbado com a hora
/// errada. Passado esse prazo a tarefa é descartada e o scheduler enfileira uma
/// nova no próximo ciclo (matriz de paridade #8).
pub const TASK_TTL_SECONDS: i64 = 120;

/// Teto de tarefas entregues por polling, para não travar um probe que ficou
/// atrás.
const DELIVERY_BATCH_LIMIT: u64 = 100;

/// Contrato de fio entre o servidor e o agente do probe.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeTask {
    pub id: String,
    pub monitor_id: i64,
    #[serde(rename = "type")]
    pub task_type: String,
    pub timeout_ms: i32,
    pub payload: serde_json::Value,
}

/// Contrato separado para discovery. Mantém compatibilidade com agentes que
/// conhecem apenas tarefas de monitor e evita monitor fictício no banco.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeDiscoveryTask {
    pub id: String,
    pub run_id: i64,
    pub cidr: String,
    pub timeout_ms: u64,
}

impl From<probe_tasks::Model> for ProbeTask {
    fn from(row: probe_tasks::Model) -> Self {
        Self {
            id: row.task_id,
            monitor_id: row.monitor_id,
            task_type: row.r#type,
            timeout_ms: row.timeout_ms,
            payload: row.payload,
        }
    }
}

/// Enfileira a tarefa, substituindo a pendente do mesmo monitor.
///
/// Sem a substituição, um probe offline acumularia uma tarefa por ciclo e
/// dispararia todas de uma vez ao voltar (matriz de paridade #9). O `DELETE`
/// prévio também é o que respeita o `UNIQUE(monitor_id)` da tabela.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn dispatch_task<C: ConnectionTrait>(
    db: &C,
    probe_id: i64,
    task: &ProbeTask,
) -> AppResult<()> {
    probe_tasks::Entity::delete_many()
        .filter(probe_tasks_entity::Column::MonitorId.eq(task.monitor_id))
        .exec(db)
        .await?;
    probe_tasks::ActiveModel {
        probe_id: Set(probe_id),
        monitor_id: Set(task.monitor_id),
        task_id: Set(task.id.clone()),
        r#type: Set(task.task_type.clone()),
        timeout_ms: Set(task.timeout_ms),
        payload: Set(task.payload.clone()),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(())
}

/// Entrega e remove as tarefas do probe, descartando as que já venceram.
///
/// A remoção acontece **antes** do filtro de validade: uma tarefa vencida some
/// da fila do mesmo jeito, senão ela seria reentregue a cada polling e nunca
/// executada.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn get_pending_tasks<C: ConnectionTrait>(
    db: &C,
    probe_id: i64,
) -> AppResult<Vec<ProbeTask>> {
    let rows = probe_tasks::Entity::find()
        .filter(probe_tasks_entity::Column::ProbeId.eq(probe_id))
        .order_by_asc(probe_tasks_entity::Column::Id)
        .limit(DELIVERY_BATCH_LIMIT)
        .all(db)
        .await?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let ids: Vec<i64> = rows.iter().map(|row| row.id).collect();
    probe_tasks::Entity::delete_many()
        .filter(probe_tasks_entity::Column::Id.is_in(ids))
        .exec(db)
        .await?;

    let cutoff = Utc::now() - Duration::seconds(TASK_TTL_SECONDS);
    Ok(rows
        .into_iter()
        .filter(|row| row.created_at.with_timezone(&Utc) > cutoff)
        .map(ProbeTask::from)
        .collect())
}

/// Entrega uma run remota pendente e a marca em execução. Uma única run por
/// polling evita que um probe pequeno receba blocos grandes em paralelo.
pub async fn get_pending_discovery_tasks<C: ConnectionTrait>(
    db: &C,
    probe_id: i64,
) -> AppResult<Vec<ProbeDiscoveryTask>> {
    let Some(run) = discovery_runs::Entity::find()
        .filter(discovery_runs_entity::Column::ProbeId.eq(probe_id))
        .filter(discovery_runs_entity::Column::Status.eq("pending"))
        .order_by_asc(discovery_runs_entity::Column::Id)
        .one(db)
        .await?
    else {
        return Ok(Vec::new());
    };
    let cidr = run
        .configuration
        .as_ref()
        .and_then(|value| value.get("cidr"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let claimed = discovery_runs::Entity::update_many()
        .col_expr(
            discovery_runs_entity::Column::Status,
            Expr::value("running"),
        )
        .col_expr(
            discovery_runs_entity::Column::StartedAt,
            Expr::value(Utc::now()),
        )
        .filter(discovery_runs_entity::Column::Id.eq(run.id))
        .filter(discovery_runs_entity::Column::Status.eq("pending"))
        .exec(db)
        .await?;
    if claimed.rows_affected == 0 {
        return Ok(Vec::new());
    }
    Ok(vec![ProbeDiscoveryTask {
        id: format!("discovery-{}", run.id),
        run_id: run.id,
        cidr,
        timeout_ms: 6 * 60 * 60 * 1_000,
    }])
}

/// Esvazia a fila de um probe — usado ao revogar ou remover o agente.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn clear_tasks_for_probe<C: ConnectionTrait>(db: &C, probe_id: i64) -> AppResult<()> {
    probe_tasks::Entity::delete_many()
        .filter(probe_tasks_entity::Column::ProbeId.eq(probe_id))
        .exec(db)
        .await?;
    Ok(())
}

/// Identificador da tarefa: monitor + instante, como no backend anterior.
///
/// Recebe o `now` em vez de lê-lo do relógio para o chamador poder testá-lo e
/// para o id combinar com o ciclo que o produziu.
#[must_use]
pub fn task_id(monitor_id: i64, now: chrono::DateTime<Utc>) -> String {
    format!("task-{monitor_id}-{}", now.timestamp_millis())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn o_contrato_de_fio_e_camel_case_com_type() {
        let task = ProbeTask {
            id: "task-1-1".into(),
            monitor_id: 1,
            task_type: "ping".into(),
            timeout_ms: 5_000,
            payload: json!({ "host": "10.0.0.1" }),
        };
        let json = serde_json::to_value(&task).unwrap();
        assert_eq!(json["monitorId"], 1);
        assert_eq!(json["timeoutMs"], 5_000);
        assert_eq!(json["type"], "ping");
        assert!(json.get("taskType").is_none());
    }

    #[test]
    fn o_id_da_tarefa_carrega_monitor_e_instante() {
        let now = chrono::DateTime::from_timestamp_millis(1_700_000_000_000).expect("instante");
        assert_eq!(task_id(42, now), "task-42-1700000000000");
    }

    #[test]
    fn discovery_usa_contrato_separado_de_monitor() {
        let task = ProbeDiscoveryTask {
            id: "discovery-9".into(),
            run_id: 9,
            cidr: "10.8.0.0/24".into(),
            timeout_ms: 5_000,
        };
        let json = serde_json::to_value(task).unwrap();
        assert_eq!(json["runId"], 9);
        assert_eq!(json["cidr"], "10.8.0.0/24");
        assert!(json.get("monitorId").is_none());
    }
}
