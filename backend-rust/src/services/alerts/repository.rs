//! Acesso às regras de alerta (§8.7).
//!
//! Isola a semântica de escopo (`NULL` = vale para todo mundo) do restante do
//! motor, que só precisa saber *quais* regras avaliar.

use sea_orm::{ColumnTrait, Condition, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter};

use crate::{
    models::alert_rules,
    services::{alerts::contracts::AlertEvaluationScope, shared::errors::AppResult},
};

/// Regras habilitadas aplicáveis ao alvo.
///
/// Cada dimensão (site, dispositivo, monitor) é filtrada de forma
/// **independente**: a regra vale quando não delimita aquela dimensão ou quando
/// aponta exatamente para o alvo avaliado. Uma regra global (as três colunas
/// nulas) atende qualquer escopo.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn find_enabled_for_scope<C: ConnectionTrait>(
    db: &C,
    scope: AlertEvaluationScope,
) -> AppResult<Vec<alert_rules::Model>> {
    let dimension = |column: alert_rules::Column, target: Option<i64>| {
        let mut any = Condition::any().add(column.is_null());
        if let Some(id) = target {
            any = any.add(column.eq(id));
        }
        any
    };

    Ok(alert_rules::Entity::find_ordered()
        .filter(alert_rules::Column::Enabled.eq(true))
        .filter(dimension(alert_rules::Column::SiteId, scope.site_id))
        .filter(dimension(alert_rules::Column::DeviceId, scope.device_id))
        .filter(dimension(alert_rules::Column::MonitorId, scope.monitor_id))
        .all(db)
        .await?)
}

/// Todas as regras, na ordem de id — a mesma de `GET /api/alert-rules`.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn find_all<C: ConnectionTrait>(db: &C) -> AppResult<Vec<alert_rules::Model>> {
    Ok(alert_rules::Entity::find_ordered().all(db).await?)
}

/// Quantas regras existem. É o que decide se a instalação é nova (§8.7,
/// `ensure_defaults`).
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn count<C: ConnectionTrait>(db: &C) -> AppResult<u64> {
    Ok(alert_rules::Entity::find().count(db).await?)
}
