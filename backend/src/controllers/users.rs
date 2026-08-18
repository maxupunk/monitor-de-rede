//! CRUD HTTP de usuários. As regras de segurança ficam em `UserService`.

use axum::{
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use loco_rs::prelude::*;

use crate::{
    controllers::auth_guard::AUTHENTICATED_USER_HEADER,
    dtos::users::{CreateUserInput, UpdateUserInput},
    services::{
        shared::errors::{AppError, AppResult},
        users::UserService,
    },
    views::users::UserDetailResponse,
};

fn actor_pid(headers: &HeaderMap) -> AppResult<&str> {
    headers
        .get(AUTHENTICATED_USER_HEADER)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| AppError::unauthorized("Não autenticado"))
}

async fn index(State(ctx): State<AppContext>) -> AppResult<Response> {
    let users = UserService::new(&ctx.db).list().await?;
    Ok(format::json(
        users
            .into_iter()
            .map(UserDetailResponse::from)
            .collect::<Vec<_>>(),
    )?)
}

async fn store(
    State(ctx): State<AppContext>,
    Json(input): Json<CreateUserInput>,
) -> AppResult<Response> {
    let user = UserService::new(&ctx.db).create(&input).await?;
    Ok((StatusCode::CREATED, Json(UserDetailResponse::from(user))).into_response())
}

async fn show(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let user = UserService::new(&ctx.db).find(id).await?;
    Ok(format::json(UserDetailResponse::from(user))?)
}

async fn update(
    headers: HeaderMap,
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
    Json(input): Json<UpdateUserInput>,
) -> AppResult<Response> {
    let user = UserService::new(&ctx.db)
        .update(actor_pid(&headers)?, id, &input)
        .await?;
    Ok(format::json(UserDetailResponse::from(user))?)
}

async fn destroy(
    headers: HeaderMap,
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    UserService::new(&ctx.db)
        .delete(actor_pid(&headers)?, id)
        .await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/users")
        .add("/", get(index).post(store))
        .add("/{id}", get(show).put(update).delete(destroy))
}
