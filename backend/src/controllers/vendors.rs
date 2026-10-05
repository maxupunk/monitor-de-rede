//! Fabricantes por MAC: consulta, situação do registro do IEEE e atualização.

use loco_rs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::services::{
    discovery::device_identifier::{hint_from_mac, Classification},
    shared::errors::{AppError, AppResult},
    vendors::{self, service, MacVendor},
};

#[derive(Debug, Deserialize)]
struct LookupQuery {
    mac: String,
}

/// O fabricante e, quando ele basta, o tipo de aparelho que sugere.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LookupResponse {
    #[serde(flatten)]
    vendor: MacVendor,
    hint: Option<Classification>,
}

async fn lookup(
    State(ctx): State<AppContext>,
    Query(query): Query<LookupQuery>,
) -> AppResult<Response> {
    let digits = query.mac.chars().filter(char::is_ascii_hexdigit).count();
    if digits < 6 {
        return Err(AppError::validation(
            "Informe ao menos os 6 primeiros dígitos do MAC.",
        ));
    }
    service::ensure_loaded(&ctx.db).await;
    let vendor = vendors::lookup(&query.mac);
    let hint = hint_from_mac(&query.mac, vendor.vendor.as_deref());
    Ok(format::json(LookupResponse { vendor, hint })?)
}

async fn status(State(ctx): State<AppContext>) -> AppResult<Response> {
    Ok(format::json(service::status(&ctx.db).await?)?)
}

async fn refresh(State(ctx): State<AppContext>) -> AppResult<Response> {
    let outcome = service::refresh(&ctx.db, &service::configured_sources()).await?;
    Ok(format::json(outcome)?)
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/vendors")
        .add("/lookup", get(lookup))
        .add("/status", get(status))
        .add("/refresh", post(refresh))
}
