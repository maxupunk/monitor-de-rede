//! O plano de backup do próprio NetMonitor: para onde vão as cópias da
//! configuração, de quanto em quanto tempo e quantas ficam.
//!
//! É o mesmo formato do plano de cada banco de dados
//! ([`crate::services::databases::service`]) — destino, agenda, retenção e o
//! resultado da última execução —, e é isso que permite à tela mostrar os
//! dois do mesmo jeito.

use chrono::Utc;
use sea_orm::{ActiveModelTrait, ConnectionTrait, EntityTrait, Set};

use crate::{
    models::{storage_destinations, system_backup_plan},
    services::shared::{
        backup_schedule::{validate_policy, BackupPolicy},
        errors::{AppError, AppResult},
    },
};

/// O plano é uma linha só.
pub const PLAN_ID: i64 = 1;

/// O que o formulário envia.
#[derive(Debug, Clone)]
pub struct PlanInput {
    pub storage_destination_id: Option<i64>,
    pub backup_enabled: bool,
    pub backup_interval_hours: i32,
    pub backup_retention: i32,
}

/// O plano, criado desligado se ainda não existir (banco recém-criado).
///
/// # Errors
///
/// Erro do banco.
pub async fn get<C: ConnectionTrait>(db: &C) -> AppResult<system_backup_plan::Model> {
    if let Some(plan) = system_backup_plan::Entity::find_by_id(PLAN_ID)
        .one(db)
        .await?
    {
        return Ok(plan);
    }
    Ok(system_backup_plan::ActiveModel {
        id: Set(PLAN_ID),
        backup_enabled: Set(false),
        backup_interval_hours: Set(24),
        backup_retention: Set(14),
        ..Default::default()
    }
    .insert(db)
    .await?)
}

/// Grava o plano.
///
/// # Errors
///
/// Frequência ou retenção fora da faixa, backup automático sem destino, ou
/// destino que não existe mais.
pub async fn update<C: ConnectionTrait>(
    db: &C,
    input: PlanInput,
) -> AppResult<system_backup_plan::Model> {
    validate_policy(input.backup_interval_hours, input.backup_retention)?;
    if input.backup_enabled && input.storage_destination_id.is_none() {
        return Err(AppError::validation(
            "Escolha um destino para ligar o backup automático",
        ));
    }
    if let Some(id) = input.storage_destination_id {
        storage_destinations::Entity::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| AppError::validation("O destino escolhido não existe mais"))?;
    }
    let mut row: system_backup_plan::ActiveModel = get(db).await?.into();
    row.storage_destination_id = Set(input.storage_destination_id);
    row.backup_enabled = Set(input.backup_enabled);
    row.backup_interval_hours = Set(input.backup_interval_hours);
    row.backup_retention = Set(input.backup_retention);
    Ok(row.update(db).await?)
}

/// O que a agenda compartilhada precisa saber do plano.
#[must_use]
pub fn policy(plan: &system_backup_plan::Model) -> BackupPolicy {
    BackupPolicy {
        enabled: plan.backup_enabled && plan.storage_destination_id.is_some(),
        interval_hours: plan.backup_interval_hours,
        last_run_at: plan.last_backup_at.map(|at| at.with_timezone(&Utc)),
        last_run_failed: plan.last_backup_status.as_deref() == Some(super::copies::STATUS_FAILED),
    }
}

/// Avisa as telas abertas que o plano mudou (edição ou resultado de backup).
pub async fn publish_updated(ctx: &loco_rs::app::AppContext) {
    if let Ok(bus) = crate::services::events::EventBus::from_context(ctx) {
        if let Err(error) = bus
            .publish(&ctx.db, "backup_plan:updated", serde_json::json!({}))
            .await
        {
            tracing::warn!(%error, "falha ao publicar backup_plan:updated");
        }
    }
}
