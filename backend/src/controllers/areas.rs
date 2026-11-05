//! Áreas do produto (ADR 017): estado, impacto de desligar e mudança.
//!
//! O controller extrai, delega e serializa: a regra mora em
//! `netmonitor_core::areas` (estado e validação) e nas políticas de cada área
//! (`services::area_policies`). Aqui só se junta a auditoria e a publicação no
//! SSE — que é o que atualiza menu, rotas e dashboard em todas as abas.

use crate::{
    controllers::auth_guard::require_admin,
    dtos::areas::AreaToggleInput,
    services::{
        area_policies,
        audit::{
            AuditAction, AuditActor, AuditChanges, AuditEntryInput, AuditService, ResourceType,
        },
        events::EventBus,
        scheduled_tasks,
        shared::errors::{AppError, AppResult},
    },
};
use axum::{extract::Path, http::HeaderMap};
use loco_rs::prelude::*;
use netmonitor_core::areas::{self, Area, AreaImpact, AreaService, AreasSnapshot};

fn parse_area(id: &str) -> AppResult<Area> {
    Area::from_id(id).ok_or_else(|| AppError::not_found(format!("Área \"{id}\" não existe.")))
}

/// `GET /api/areas` — o estado de todas as áreas (qualquer usuário: é o que
/// monta o menu). O mesmo corpo chega pelo SSE em `areas:updated`.
async fn index(State(ctx): State<AppContext>) -> AppResult<Response> {
    let policies = area_policies::registry();
    Ok(format::json(
        AreaService::new(&ctx.db, &policies).snapshot().await?,
    )?)
}

/// `GET /api/areas/{area}/impact` — o que acontece ao desligar, para a
/// confirmação. Só admin, como a mudança.
async fn impact(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Response> {
    require_admin(
        &ctx,
        &headers,
        "Apenas administradores podem ligar ou desligar áreas.",
    )
    .await?;
    let area = parse_area(&id)?;
    Ok(format::json(area_impact(&ctx, area).await?)?)
}

/// O impacto da área somado ao das tarefas agendadas — que dependem do
/// catálogo da IA, fora do alcance das políticas.
async fn area_impact(ctx: &AppContext, area: Area) -> AppResult<AreaImpact> {
    let policies = area_policies::registry();
    let mut impact = AreaService::new(&ctx.db, &policies).impact(area).await?;
    impact
        .consequences
        .extend(scheduled_tasks::areas::consequence(&ctx.db, area).await?);
    Ok(impact)
}

/// `PUT /api/areas/{area}` — liga ou desliga. Só admin; auditado; publicado.
async fn update(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<AreaToggleInput>,
) -> AppResult<Response> {
    let user = require_admin(
        &ctx,
        &headers,
        "Apenas administradores podem ligar ou desligar áreas.",
    )
    .await?;
    let area = parse_area(&id)?;
    let policies = area_policies::registry();
    let before = areas::is_enabled(&ctx.db, area).await?;
    let snapshot = AreaService::new(&ctx.db, &policies)
        .set(area, input.enabled)
        .await?;

    record_change(&ctx, &headers, user.id, area, before, input.enabled).await;
    publish(&ctx, &snapshot);
    Ok(format::json(snapshot)?)
}

/// Audita a mudança de uma área (só quando o estado mudou de fato).
pub(crate) async fn record_change(
    ctx: &AppContext,
    headers: &HeaderMap,
    user_id: i64,
    area: Area,
    before: bool,
    enabled: bool,
) {
    if before == enabled {
        return;
    }
    let _ = AuditService::new(&ctx.db)
        .log(
            AuditActor::from_user_id(user_id, headers),
            AuditEntryInput {
                action: AuditAction::Update,
                resource_type: ResourceType::SystemSetting,
                resource_id: None,
                resource_label: Some(format!("Área {}", area.title())),
                description: Some(format!(
                    "Área {} {}",
                    area.title(),
                    if enabled { "ligada" } else { "desligada" }
                )),
                changes: Some(AuditChanges {
                    old: Some(serde_json::json!({ "enabled": before })),
                    new: Some(serde_json::json!({ "enabled": enabled })),
                }),
            },
        )
        .await;
}

/// Publica o estado no SSE: menu, rotas e dashboard de todas as abas mudam.
pub(crate) fn publish(ctx: &AppContext, snapshot: &AreasSnapshot) {
    if let Ok(bus) = EventBus::from_context(ctx) {
        areas::publish(&bus, snapshot);
    }
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/areas")
        .add("/", get(index))
        .add("/{area}", put(update))
        .add("/{area}/impact", get(impact))
}
