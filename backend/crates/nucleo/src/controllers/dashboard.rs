//! Persistência do layout compartilhado do dashboard.

use crate::{
    dtos::resources::DashboardLayoutInput, services::dashboard, services::shared::errors::AppResult,
};
use loco_rs::prelude::*;
async fn get_layout(State(ctx): State<AppContext>) -> AppResult<Response> {
    let stored = dashboard::stored(&ctx.db).await?;
    Ok(format::json(
        serde_json::json!({"layout":stored.layout,"updatedAt":stored.updated_at}),
    )?)
}
/// `GET /api/dashboard/catalog` — tipos de card e layout padrão: a fonte única
/// que a tela usa no lugar de uma lista própria (ARQ-04).
async fn get_catalog() -> AppResult<Response> {
    Ok(format::json(dashboard::catalog::dashboard_catalog())?)
}

async fn save_layout(
    State(ctx): State<AppContext>,
    Json(input): Json<DashboardLayoutInput>,
) -> AppResult<Response> {
    let saved = dashboard::save(&ctx, &input.layout, input.client_id.as_deref()).await?;
    Ok(format::json(
        serde_json::json!({"success":true,"updatedAt":saved.updated_at.to_rfc3339()}),
    )?)
}
pub fn routes() -> Routes {
    Routes::new()
        .prefix("/dashboard")
        .add("/layout", get(get_layout).post(save_layout))
        .add("/catalog", get(get_catalog))
}
