//! Quem conversa pode usar esta ferramenta? A mesma regra das rotas HTTP.

use std::str::FromStr;

use loco_rs::prelude::AppContext;
use sea_orm::EntityTrait;

use super::args::ToolArgs;
use crate::{
    areas::Area,
    models::users,
    roles::{AreaAccess, Role},
    shared::errors::{AppError, AppResult},
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

/// Garante que o usuário ativo da chamada é administrador na `area` indicada
/// (ou administrador global, que tem papel `Admin` em todas as áreas).
///
/// # Errors
///
/// `Forbidden` sem usuário identificado, inativo ou sem papel `Admin` na área;
/// erro do banco.
pub async fn require_area_admin(
    ctx: &AppContext,
    args: &ToolArgs,
    area: Area,
    refusal: &str,
) -> AppResult<i64> {
    let user_id = args
        .actor()
        .user_id
        .ok_or_else(|| AppError::forbidden("Usuário não identificado."))?;
    let user = users::Entity::find_by_id(user_id)
        .one(&ctx.db)
        .await?
        .filter(|user| user.active)
        .ok_or_else(|| AppError::forbidden("Usuário não encontrado ou inativo."))?;
    let access = AreaAccess::of_user(&user)?;
    if access.role_in(area) == Some(Role::Admin) || access.global() == Role::Admin {
        Ok(user_id)
    } else {
        Err(AppError::forbidden(refusal))
    }
}
