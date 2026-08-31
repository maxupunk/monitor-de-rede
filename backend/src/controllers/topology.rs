//! Rotas de topologia: consulta do grafo e enlaces manuais.

use axum::{http::StatusCode, response::IntoResponse};
use loco_rs::prelude::*;
use std::collections::BTreeMap;

use crate::{
    dtos::resources::{
        TopologyLayoutInput, TopologyLinkInput, TopologyLinkUpdateInput, UnmanagedSwitchInput,
    },
    services::{
        events::EventBus,
        shared::errors::{AppError, AppResult},
        topology::{layout, service},
    },
};

fn parse_site_id(query: &BTreeMap<String, String>) -> AppResult<Option<i64>> {
    query
        .get("site_id")
        .or_else(|| query.get("siteId"))
        .map(|value| {
            value
                .parse::<i64>()
                .map_err(|_| AppError::validation("siteId inválido"))
        })
        .transpose()
}

async fn index(
    State(ctx): State<AppContext>,
    Query(query): Query<BTreeMap<String, String>>,
) -> AppResult<Response> {
    let site_id = parse_site_id(&query)?;
    let live = query
        .get("live")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);
    Ok(format::json(
        service::get_topology_with_live(&ctx, site_id, live).await?,
    )?)
}

async fn store_link(
    State(ctx): State<AppContext>,
    Json(input): Json<TopologyLinkInput>,
) -> AppResult<Response> {
    let link = service::create_manual_link(
        &ctx.db,
        input.source_device_id,
        input.target_device_id,
        input.source_interface_id,
        input.target_interface_id,
        input.link_type,
    )
    .await?;
    emit_topology_updated(&ctx).await;
    Ok((StatusCode::CREATED, axum::Json(link)).into_response())
}

async fn update_link(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
    Json(input): Json<TopologyLinkUpdateInput>,
) -> AppResult<Response> {
    let link = service::update_manual_link(
        &ctx.db,
        id,
        input.source_interface_id,
        input.target_interface_id,
        input.link_type,
    )
    .await?;
    emit_topology_updated(&ctx).await;
    Ok(format::json(link)?)
}

async fn store_unmanaged_switch(
    State(ctx): State<AppContext>,
    Json(input): Json<UnmanagedSwitchInput>,
) -> AppResult<Response> {
    let device = service::create_unmanaged_switch(&ctx.db, input).await?;
    emit_topology_updated(&ctx).await;
    Ok((StatusCode::CREATED, axum::Json(device)).into_response())
}

async fn destroy_link(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    if !service::delete_link(&ctx.db, id).await? {
        return Err(AppError::not_found("Ligação não encontrada"));
    }
    emit_topology_updated(&ctx).await;
    Ok(format::json(
        serde_json::json!({ "message": "Ligação removida com sucesso" }),
    )?)
}

async fn recalculate(State(ctx): State<AppContext>) -> AppResult<Response> {
    let count = service::infer_subnet_links(&ctx.db).await?;
    emit_topology_updated(&ctx).await;
    Ok(format::json(
        serde_json::json!({ "message":"Recálculo de topologia concluído", "inferredCount":count }),
    )?)
}

async fn get_layout(
    State(ctx): State<AppContext>,
    Query(query): Query<BTreeMap<String, String>>,
) -> AppResult<Response> {
    let site_id = parse_site_id(&query)?;
    Ok(format::json(layout::load_layout(&ctx.db, site_id).await?)?)
}

async fn save_layout(
    State(ctx): State<AppContext>,
    Query(query): Query<BTreeMap<String, String>>,
    Json(input): Json<TopologyLayoutInput>,
) -> AppResult<Response> {
    let site_id = parse_site_id(&query)?;
    let layout = layout::TopologyLayout {
        nodes: input
            .nodes
            .into_iter()
            .map(|node| layout::TopologyLayoutNode {
                device_id: node.device_id,
                x: node.x,
                y: node.y,
            })
            .collect(),
    };
    layout::save_layout(&ctx.db, site_id, layout).await?;
    Ok(format::json(serde_json::json!({ "success": true }))?)
}

async fn emit_topology_updated(ctx: &AppContext) {
    if let Ok(bus) = EventBus::from_context(ctx) {
        if let Err(error) = bus
            .publish(&ctx.db, "topology:updated", serde_json::json!({}))
            .await
        {
            tracing::warn!(%error, "falha ao publicar topology:updated");
        }
    }
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/topology")
        .add("/", get(index))
        .add("/layout", get(get_layout).put(save_layout))
        .add("/links", post(store_link))
        .add("/links/{id}", put(update_link).delete(destroy_link))
        .add("/unmanaged-switch", post(store_unmanaged_switch))
        .add("/recalculate", post(recalculate))
}
