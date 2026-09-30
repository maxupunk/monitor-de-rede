//! Os padrões de log como a tela os vê: categoria, confiança, se o operador
//! confirmou, e como virar alerta.
//!
//! Virar alerta é **sugestão**: a tela abre o diálogo de regra já preenchido
//! e o operador salva. Categoria com modelo no catálogo de alertas usa o
//! modelo; sem modelo, uma regra `log_pattern` com a regex do padrão.

use std::collections::HashSet;

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect,
};

use super::{
    categorizer::CLASSIFIED_EVENT,
    template::{hash_hex, parse_hash_hex, template_to_regex},
};
use crate::{
    dtos::logs::LogTemplateInfo,
    models::logs::log_templates,
    services::{
        ai::laya::decisions::log_category::is_category,
        events::EventBus,
        shared::errors::{AppError, AppResult},
    },
};

/// Quantos padrões a listagem devolve.
const LIST_LIMIT: u64 = 200;

/// Categoria → modelo do catálogo de alertas que já a cobre
/// (`alerts::catalog::templates`).
pub const CATEGORY_ALERT_TEMPLATES: &[(&str, &str)] = &[
    ("auth_failure", "log_login_failure"),
    ("reboot", "log_system_started"),
    ("routing", "log_routing_down"),
    ("link_change", "log_pppoe_flapping"),
    ("dhcp", "log_dhcp_pool_exhausted"),
    ("resource_exhaustion", "log_out_of_memory"),
    ("config_change", "log_config_changed"),
];

fn alert_template(category: Option<&str>) -> Option<String> {
    let category = category?;
    CATEGORY_ALERT_TEMPLATES
        .iter()
        .find(|(known, _)| *known == category)
        .map(|(_, key)| (*key).to_string())
}

#[must_use]
pub fn info(row: &log_templates::Model) -> LogTemplateInfo {
    let confirmed = row.user_category.is_some();
    let category = row.effective_category().map(str::to_string);
    LogTemplateInfo {
        template_hash: hash_hex(row.template_hash),
        template: row.template.clone(),
        example: row.example.clone(),
        alert_template: alert_template(category.as_deref()),
        category,
        confidence: if confirmed { None } else { row.confidence },
        model: if confirmed { None } else { row.model.clone() },
        confirmed,
        alert_regex: template_to_regex(&row.template),
    }
}

/// Os padrões de um conjunto de linhas (a página da tela).
///
/// # Errors
///
/// Propaga erro do banco de logs.
pub async fn infos_for(
    db: &DatabaseConnection,
    hashes: HashSet<i64>,
) -> AppResult<Vec<LogTemplateInfo>> {
    if hashes.is_empty() {
        return Ok(Vec::new());
    }
    let rows = log_templates::Entity::find()
        .filter(log_templates::Column::TemplateHash.is_in(hashes))
        .all(db)
        .await?;
    Ok(rows.iter().map(info).collect())
}

/// Os padrões mais recentes, opcionalmente de uma categoria.
///
/// # Errors
///
/// Propaga erro do banco de logs.
pub async fn list(
    db: &DatabaseConnection,
    category: Option<&str>,
) -> AppResult<Vec<LogTemplateInfo>> {
    let rows = log_templates::Entity::find()
        .order_by_desc(log_templates::Column::FirstSeenAt)
        .limit(LIST_LIMIT)
        .all(db)
        .await?;
    Ok(rows
        .iter()
        .filter(|row| category.is_none_or(|wanted| row.effective_category() == Some(wanted)))
        .map(info)
        .collect())
}

/// O operador confirma ou corrige a categoria de um padrão.
///
/// # Errors
///
/// Hash ou categoria inválidos, padrão inexistente, ou erro do banco.
pub async fn confirm(
    db: &DatabaseConnection,
    hash: &str,
    category: &str,
    events: Option<&EventBus>,
) -> AppResult<LogTemplateInfo> {
    let hash = parse_hash_hex(hash).ok_or_else(|| AppError::validation("Padrão inválido"))?;
    let category = category.trim();
    if !is_category(category) {
        return Err(AppError::validation(format!(
            "Categoria desconhecida: '{category}'"
        )));
    }
    let row = log_templates::Entity::find_by_id(hash)
        .one(db)
        .await?
        .ok_or_else(|| AppError::not_found("Padrão de log não encontrado"))?;
    let mut active: log_templates::ActiveModel = row.into();
    active.user_category = Set(Some(category.to_string()));
    active.confirmed_at = Set(Some(Utc::now().into()));
    let saved = active.update(db).await?;
    let info = info(&saved);
    if let Some(events) = events {
        events.publish_ephemeral(
            CLASSIFIED_EVENT,
            serde_json::to_value(&info).unwrap_or_default(),
        );
    }
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::ai::laya::decisions::log_category::LOG_CATEGORIES;

    #[test]
    fn todo_modelo_de_alerta_aponta_para_categoria_conhecida() {
        for (category, _) in CATEGORY_ALERT_TEMPLATES {
            assert!(
                LOG_CATEGORIES.iter().any(|(known, _)| known == category),
                "{category}"
            );
        }
    }

    #[test]
    fn confirmacao_do_operador_esconde_o_palpite() {
        let row = log_templates::Model {
            template_hash: 7,
            template: "ether# link down".into(),
            example: "ether3 link down".into(),
            app_name: None,
            category: Some("hardware".into()),
            confidence: Some(64),
            model: Some("laya:en".into()),
            classified_at: None,
            user_category: Some("link_change".into()),
            confirmed_at: None,
            first_seen_at: Utc::now().into(),
        };
        let info = info(&row);
        assert_eq!(info.category.as_deref(), Some("link_change"));
        assert!(info.confirmed);
        assert_eq!(info.confidence, None);
        assert_eq!(info.alert_template.as_deref(), Some("log_pppoe_flapping"));
        assert_eq!(info.alert_regex, r"ether\S* link down");
    }
}
