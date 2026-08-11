//! Persistência do layout compartilhado do dashboard.

use crate::{
    dtos::resources::DashboardLayoutInput,
    models::system_settings::{Model, DASHBOARD_LAYOUT},
    services::shared::errors::AppResult,
};
use loco_rs::prelude::*;
async fn get_layout(State(ctx): State<AppContext>) -> AppResult<Response> {
    let setting = Model::get(&ctx.db, DASHBOARD_LAYOUT).await?;
    let updated_at = setting.as_ref().map(|row| row.updated_at.to_rfc3339());
    let layout = setting
        .and_then(|row| row.value)
        .and_then(|value| serde_json::from_str::<serde_json::Value>(&value).ok())
        .filter(serde_json::Value::is_array);
    Ok(format::json(
        serde_json::json!({"layout":layout,"updatedAt":updated_at}),
    )?)
}
async fn save_layout(
    State(ctx): State<AppContext>,
    Json(input): Json<DashboardLayoutInput>,
) -> AppResult<Response> {
    let saved =
        Model::set(
            &ctx.db,
            DASHBOARD_LAYOUT,
            Some(serde_json::to_string(&input.layout).map_err(|error| {
                crate::services::shared::errors::AppError::Internal(error.into())
            })?),
        )
        .await?;
    Ok(format::json(
        serde_json::json!({"success":true,"updatedAt":saved.updated_at.to_rfc3339()}),
    )?)
}
pub fn routes() -> Routes {
    Routes::new()
        .prefix("/dashboard")
        .add("/layout", get(get_layout).post(save_layout))
}
