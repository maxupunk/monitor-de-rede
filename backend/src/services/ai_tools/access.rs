//! Quem conversa pode usar esta ferramenta? A mesma regra das rotas HTTP.

use std::str::FromStr;

use loco_rs::prelude::AppContext;
use sea_orm::EntityTrait;

use super::ToolArgs;
use crate::{
    models::users,
    services::{
        shared::errors::{AppError, AppResult},
        users::Role,
    },
};

/// O id do usuário da conversa, se o papel dele passa em `allowed`; senão a
/// recusa com `refusal`, que a IA repassa ao usuário.
///
/// # Errors
///
/// `forbidden` sem usuário identificado ou com papel insuficiente; erro do
/// banco.
pub async fn require_role(
    ctx: &AppContext,
    args: &ToolArgs,
    allowed: fn(Role) -> bool,
    refusal: &str,
) -> AppResult<i64> {
    let user_id = args
        .actor()
        .user_id
        .ok_or_else(|| AppError::forbidden("Usuário não identificado."))?;
    let user = users::Entity::find_by_id(user_id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::forbidden("Usuário não encontrado."))?;
    if allowed(Role::from_str(&user.role)?) {
        Ok(user_id)
    } else {
        Err(AppError::forbidden(refusal))
    }
}
