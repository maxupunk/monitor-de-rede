//! Preferências globais do sistema.
//!
//! Controller extrai, valida, delega e serializa: a faixa aceita de cada campo
//! e os padrões vivem em [`crate::services::preferences`], que é também quem os
//! pontos de consumo consultam.

use std::str::FromStr;

use axum::http::HeaderMap;
use loco_rs::prelude::*;

use crate::{
    controllers::auth_guard::AUTHENTICATED_USER_HEADER,
    services::{
        audit::{AuditAction, AuditActor, AuditEntryInput, AuditService, ResourceType},
        onboarding,
        preferences::{self, Preferences},
        settings::database,
        shared::errors::{AppError, AppResult},
        syslog::{db::LogsDb, nat::NatDetector, SyslogService},
        users::Role,
    },
};

/// `GET /api/settings` — as preferências em vigor.
async fn show(State(ctx): State<AppContext>) -> AppResult<Response> {
    Ok(format::json(preferences::load(&ctx.db).await?)?)
}

/// `PUT /api/settings` — grava e devolve o que ficou valendo.
///
/// Devolve o documento gravado, e não um `{success:true}`: os valores voltam
/// aparados e validados, e a tela precisa refletir exatamente o que passou a
/// valer — não o que foi digitado.
async fn update(
    State(ctx): State<AppContext>,
    Json(entrada): Json<Preferences>,
) -> AppResult<Response> {
    Ok(format::json(preferences::save(&ctx.db, entrada).await?)?)
}

/// `GET /api/settings/onboarding` — status do assistente inicial e dados detectados.
async fn onboarding_status(State(ctx): State<AppContext>) -> AppResult<Response> {
    Ok(format::json(
        onboarding::get_status(&ctx.db, &detector(&ctx)).await?,
    )?)
}

/// `POST /api/settings/onboarding/complete` — marca o onboarding como concluído.
async fn complete_onboarding(State(ctx): State<AppContext>) -> AppResult<Response> {
    Ok(format::json(onboarding::mark_completed(&ctx.db).await?)?)
}

/// `GET /api/settings/database-size` — tipo, tamanho e amplitude temporal do histórico do banco de dados.
async fn database_size(State(ctx): State<AppContext>) -> AppResult<Response> {
    let logs_db = LogsDb::from_context(&ctx).ok();
    Ok(format::json(
        database::database_info(
            &ctx.db,
            &ctx.config.database.uri,
            logs_db.as_ref().map(LogsDb::connection),
        )
        .await?,
    )?)
}

/// `POST /api/settings/clear-history` — limpa métricas, resultados e logs históricos, liberando disco.
async fn clear_history(headers: HeaderMap, State(ctx): State<AppContext>) -> AppResult<Response> {
    require_admin(&ctx, &headers).await?;

    let logs_db = LogsDb::from_context(&ctx).ok();
    let stats = database::clear_history(&ctx.db, logs_db.as_ref().map(LogsDb::connection)).await?;

    let _ = AuditService::new(&ctx.db)
        .log(
            AuditActor::from_headers(&headers, &ctx.db)
                .await
                .unwrap_or_default(),
            AuditEntryInput {
                action: AuditAction::Delete,
                resource_type: ResourceType::SystemSetting,
                resource_id: None,
                resource_label: Some("database_history".to_string()),
                description: Some(format!(
                    "Histórico do sistema apagado ({} registros removidos)",
                    stats.total_deleted
                )),
                changes: None,
            },
        )
        .await;

    Ok(format::json(stats)?)
}

/// `POST /api/settings/clear-all-items` — limpa todos os cadastros e dados, mantendo apenas usuários e preferências.
async fn clear_all_items(headers: HeaderMap, State(ctx): State<AppContext>) -> AppResult<Response> {
    require_admin(&ctx, &headers).await?;

    let logs_db = LogsDb::from_context(&ctx).ok();
    let stats =
        database::clear_all_items(&ctx.db, logs_db.as_ref().map(LogsDb::connection)).await?;

    let _ = AuditService::new(&ctx.db)
        .log(
            AuditActor::from_headers(&headers, &ctx.db)
                .await
                .unwrap_or_default(),
            AuditEntryInput {
                action: AuditAction::Delete,
                resource_type: ResourceType::SystemSetting,
                resource_id: None,
                resource_label: Some("database_all_items".to_string()),
                description: Some(format!(
                    "Todos os itens e cadastros apagados ({} dispositivos, {} monitores, {} redes removidos)",
                    stats.devices_deleted, stats.monitors_deleted, stats.networks_deleted
                )),
                changes: None,
            },
        )
        .await;

    Ok(format::json(stats)?)
}

async fn require_admin(ctx: &AppContext, headers: &HeaderMap) -> AppResult<()> {
    let pid = headers
        .get(AUTHENTICATED_USER_HEADER)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| AppError::unauthorized("Não autenticado"))?;

    let user = crate::models::users::Model::find_by_pid(&ctx.db, pid)
        .await
        .map_err(|_| AppError::unauthorized("Não autenticado"))?;

    let role = Role::from_str(&user.role)?;
    if !role.can_manage_users() {
        return Err(AppError::forbidden(
            "Apenas administradores podem apagar o histórico do sistema.",
        ));
    }

    Ok(())
}

fn detector(ctx: &AppContext) -> NatDetector {
    SyslogService::from_context(ctx).map_or_else(NatDetector::detect, |servico| {
        servico.ingestor.resolver().nat().clone()
    })
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/settings")
        .add("/", get(show).put(update))
        .add("/onboarding", get(onboarding_status))
        .add("/onboarding/complete", post(complete_onboarding))
        .add("/database-size", get(database_size))
        .add("/clear-history", post(clear_history))
        .add("/clear-all-items", post(clear_all_items))
}
