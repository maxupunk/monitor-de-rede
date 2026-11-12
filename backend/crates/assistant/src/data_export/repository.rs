//! Consultas da exportação. As conversas são lidas em páginas: cada uma pode
//! ter até 2 MB, e a exportação não carrega todas de uma vez.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, Select,
};

use netmonitor_core::{
    models::{ai_conversations, devices, plugin_runs, users},
    shared::errors::AppResult,
};

/// Origem gravada em `plugin_runs` quando quem pediu foi a IA.
const AI_ORIGIN: &str = "ai";
/// Conversas por página lida do banco.
pub const PAGE_SIZE: u64 = 50;

fn conversations_since(since: Option<DateTime<Utc>>) -> Select<ai_conversations::Entity> {
    let query = ai_conversations::Entity::find();
    match since {
        Some(since) => query.filter(ai_conversations::Column::UpdatedAt.gte(since)),
        None => query,
    }
}

fn ai_runs_since(since: Option<DateTime<Utc>>) -> Select<plugin_runs::Entity> {
    let query = plugin_runs::Entity::find().filter(plugin_runs::Column::Origin.eq(AI_ORIGIN));
    match since {
        Some(since) => query.filter(plugin_runs::Column::CreatedAt.gte(since)),
        None => query,
    }
}

/// `(user_id, message_count, updated_at)` de cada conversa do período — sem
/// as mensagens, para o resumo custar pouco.
///
/// # Errors
///
/// Erro do banco.
pub async fn conversation_stats<C: ConnectionTrait>(
    db: &C,
    since: Option<DateTime<Utc>>,
) -> AppResult<Vec<(i64, i32, sea_orm::prelude::DateTimeWithTimeZone)>> {
    Ok(conversations_since(since)
        .select_only()
        .columns([
            ai_conversations::Column::UserId,
            ai_conversations::Column::MessageCount,
            ai_conversations::Column::UpdatedAt,
        ])
        .into_tuple()
        .all(db)
        .await?)
}

/// Uma página de conversas do período, das mais antigas para as mais novas.
///
/// # Errors
///
/// Erro do banco.
pub async fn conversation_page<C: ConnectionTrait>(
    db: &C,
    since: Option<DateTime<Utc>>,
    page: u64,
) -> AppResult<Vec<ai_conversations::Model>> {
    Ok(conversations_since(since)
        .order_by_asc(ai_conversations::Column::Id)
        .paginate(db, PAGE_SIZE)
        .fetch_page(page)
        .await?)
}

/// Quantas execuções em equipamento a IA pediu no período.
///
/// # Errors
///
/// Erro do banco.
pub async fn count_ai_runs<C: ConnectionTrait>(
    db: &C,
    since: Option<DateTime<Utc>>,
) -> AppResult<u64> {
    Ok(ai_runs_since(since).count(db).await?)
}

/// Execuções em equipamento pedidas pela IA no período, em ordem de criação.
///
/// # Errors
///
/// Erro do banco.
pub async fn ai_runs<C: ConnectionTrait>(
    db: &C,
    since: Option<DateTime<Utc>>,
) -> AppResult<Vec<plugin_runs::Model>> {
    Ok(ai_runs_since(since)
        .order_by_asc(plugin_runs::Column::Id)
        .all(db)
        .await?)
}

/// Nome de cada usuário, para quem revisa saber de quem é a conversa.
///
/// # Errors
///
/// Erro do banco.
pub async fn user_names<C: ConnectionTrait>(db: &C) -> AppResult<HashMap<i64, String>> {
    let rows: Vec<(i64, String)> = users::Entity::find()
        .select_only()
        .columns([users::users::Column::Id, users::users::Column::Name])
        .into_tuple()
        .all(db)
        .await?;
    Ok(rows.into_iter().collect())
}

/// Nome de cada equipamento citado nas execuções.
///
/// # Errors
///
/// Erro do banco.
pub async fn device_names<C: ConnectionTrait>(
    db: &C,
    ids: Vec<i64>,
) -> AppResult<HashMap<i64, String>> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows: Vec<(i64, String)> = devices::Entity::find()
        .select_only()
        .columns([devices::Column::Id, devices::Column::Name])
        .filter(devices::Column::Id.is_in(ids))
        .into_tuple()
        .all(db)
        .await?;
    Ok(rows.into_iter().collect())
}
