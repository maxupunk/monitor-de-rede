//! Layout compartilhado do dashboard: catálogo de cards, operações sobre o
//! layout e a gravação em `system_settings` (`DASHBOARD_LAYOUT`).
//!
//! Toda gravação publica `dashboard:layout_updated` com
//! `{ layout, clientId }` no barramento global — é o que faz as outras abas
//! (e a própria, quando a mudança veio da IA) redesenharem o dashboard. O
//! `clientId` é o da aba que salvou; a IA grava com `null`.
//!
//! - [`catalog`]: tipos de card, tamanhos e o layout padrão.
//! - [`layout`]: as operações puras (adicionar, remover, mover...).

pub mod catalog;
pub mod layout;

use chrono::Utc;
use loco_rs::prelude::AppContext;
use rand::Rng;
use sea_orm::ConnectionTrait;
use serde_json::{json, Value};

pub use catalog::{DashboardWidgetSize, DashboardWidgetType};
pub use layout::{DashboardLayout, DashboardWidget, LayoutOperation};

use crate::{
    models::system_settings::{Model as Setting, DASHBOARD_LAYOUT},
    services::{
        events::EventBus,
        shared::{
            errors::{AppError, AppResult},
            transaction::begin_write,
        },
    },
};

/// Evento SSE de layout salvo.
pub const LAYOUT_UPDATED_EVENT: &str = "dashboard:layout_updated";

/// O layout como está gravado, para o `GET` da tela.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredLayout {
    /// `None` quando nada foi salvo (ou o valor gravado não é uma lista).
    pub layout: Option<Value>,
    pub updated_at: Option<String>,
}

/// Lê o layout gravado, sem completar com o padrão.
///
/// # Errors
///
/// Erro do banco.
pub async fn stored<C: ConnectionTrait>(db: &C) -> AppResult<StoredLayout> {
    let setting = Setting::get(db, DASHBOARD_LAYOUT).await?;
    let updated_at = setting.as_ref().map(|row| row.updated_at.to_rfc3339());
    let layout = setting
        .and_then(|row| row.value)
        .and_then(|value| serde_json::from_str::<Value>(&value).ok())
        .filter(Value::is_array);
    Ok(StoredLayout { layout, updated_at })
}

/// O layout em uso: o gravado, ou o padrão quando não há nenhum.
///
/// # Errors
///
/// Erro do banco.
pub async fn load<C: ConnectionTrait>(db: &C) -> AppResult<DashboardLayout> {
    Ok(match stored(db).await?.layout {
        Some(Value::Array(items)) if !items.is_empty() => DashboardLayout::from_values(&items),
        _ => DashboardLayout::default_layout(),
    })
}

async fn write<C: ConnectionTrait>(db: &C, layout: &[Value]) -> AppResult<Setting> {
    let text = serde_json::to_string(layout).map_err(|error| AppError::Internal(error.into()))?;
    Setting::set(db, DASHBOARD_LAYOUT, Some(text)).await
}

/// Avisa as abas abertas. Falha de publicação não desfaz a gravação — só
/// fica no log.
async fn publish(ctx: &AppContext, layout: &[Value], client_id: Option<&str>) {
    if let Ok(bus) = EventBus::from_context(ctx) {
        let payload = json!({ "layout": layout, "clientId": client_id });
        if let Err(error) = bus.publish(&ctx.db, LAYOUT_UPDATED_EVENT, payload).await {
            tracing::warn!(%error, "falha ao publicar o layout do dashboard");
        }
    }
}

/// Grava o layout que a tela mandou, como veio, e avisa as outras abas.
///
/// # Errors
///
/// Erro do banco.
pub async fn save(
    ctx: &AppContext,
    layout: &[Value],
    client_id: Option<&str>,
) -> AppResult<Setting> {
    let saved = write(&ctx.db, layout).await?;
    publish(ctx, layout, client_id).await;
    Ok(saved)
}

/// Id de uma instância nova: `<tipo>_<milissegundos>_<5 letras>`, como a
/// tela gera.
#[must_use]
pub fn new_instance_id(kind: DashboardWidgetType) -> String {
    const ALPHABET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut rng = rand::thread_rng();
    let suffix: String = (0..5)
        .map(|_| char::from(ALPHABET[rng.gen_range(0..ALPHABET.len())]))
        .collect();
    format!("{}_{}_{suffix}", kind.id(), Utc::now().timestamp_millis())
}

/// O que as operações fariam no layout atual, sem gravar: as frases da
/// confirmação ou o erro da primeira inválida.
///
/// # Errors
///
/// Operação inválida ou erro do banco.
pub async fn preview<C: ConnectionTrait>(
    db: &C,
    operations: &[LayoutOperation],
) -> AppResult<Vec<String>> {
    let mut layout = load(db).await?;
    layout.apply_all(operations, &mut new_instance_id)
}

/// Aplica as operações, em ordem, ao layout atual — tudo ou nada —, grava e
/// avisa as abas (`clientId` nulo: a mudança não veio de nenhuma delas).
///
/// # Errors
///
/// Operação inválida (nada é gravado) ou erro do banco.
pub async fn apply(
    ctx: &AppContext,
    operations: &[LayoutOperation],
) -> AppResult<(DashboardLayout, Vec<String>)> {
    let txn = begin_write(&ctx.db).await?;
    let mut layout = load(&txn).await?;
    let changes = layout.apply_all(operations, &mut new_instance_id)?;
    let values = layout.to_values();
    write(&txn, &values).await?;
    txn.commit().await?;
    publish(ctx, &values, None).await;
    Ok((layout, changes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_de_instancia_segue_o_formato_da_tela() {
        let id = new_instance_id(DashboardWidgetType::CpuUsage);
        let partes: Vec<&str> = id.rsplitn(3, '_').collect();
        assert_eq!(partes.len(), 3, "{id}");
        assert_eq!(partes[0].len(), 5);
        assert!(partes[1].parse::<i64>().is_ok(), "{id}");
        assert_eq!(partes[2], "cpu_usage");
        assert_ne!(id, new_instance_id(DashboardWidgetType::CpuUsage));
    }
}
