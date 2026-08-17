//! Preferências globais do sistema.
//!
//! Controller extrai, valida, delega e serializa: a faixa aceita de cada campo
//! e os padrões vivem em [`crate::services::preferences`], que é também quem os
//! pontos de consumo consultam.

use loco_rs::prelude::*;

use crate::services::{
    preferences::{self, Preferences},
    shared::errors::AppResult,
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

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/settings")
        .add("/", get(show).put(update))
}
