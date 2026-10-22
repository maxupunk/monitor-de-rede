//! A trilha de auditoria: quem mudou o quê, quando, e o antes/depois.
//!
//! Responde "o que mudou antes de o problema começar?": a IA acha no log o
//! primeiro sinal do problema (grep, logs) e pergunta aqui pelas alterações
//! na janela anterior a ele (`until` + `hours`). Só leitura; IP e navegador de
//! quem alterou não saem daqui.

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use loco_rs::prelude::AppContext;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::{json, Value};

use super::{AiToolHandler, ToolArgs, ToolGroup, ToolOutput};
use crate::{
    models::_entities::users,
    services::{
        audit::{AuditFilters, AuditService},
        shared::{errors::AppResult, text::truncate_chars},
    },
};

const DEFAULT_LIMIT: i64 = 20;
const MAX_LIMIT: i64 = 50;
/// Teto do antes/depois por entrada: um plugin ou uma configuração inteira
/// estouraria o contexto da IA.
const CHANGES_MAX_CHARS: usize = 600;
const ACTIONS: [&str; 6] = ["create", "update", "delete", "login", "logout", "execute"];

pub struct AuditLog;

#[async_trait]
impl AiToolHandler for AuditLog {
    fn name(&self) -> &'static str {
        "get_audit_log"
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Audit
    }

    fn description(&self) -> &'static str {
        "Trilha de auditoria: quem criou, alterou, excluiu ou executou o quê e quando, com o antes/depois de cada alteração. \
Use 'until' (o instante em que o problema começou) e 'hours' para ver o que mudou antes dele; 'search' procura na descrição \
e no nome do recurso; 'resource_type' (device, monitor, alert_rule, plugin, system_setting, vpn_peer…) e 'action' filtram."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "until": { "type": "string", "description": "Fim da janela em RFC 3339 (ex: 2026-09-21T14:00:00Z). Omitido: agora." },
                "hours": { "type": "integer", "description": "Horas antes de 'until' (1 a 720, padrão 24)" },
                "search": { "type": "string", "description": "Texto na descrição ou no nome do recurso alterado" },
                "resource_type": { "type": "string", "description": "Tipo do recurso (device, monitor, alert_rule, plugin, system_setting, vpn_peer, network…)" },
                "action": { "type": "string", "enum": ACTIONS },
                "limit": { "type": "integer", "description": "Entradas (1 a 50, padrão 20), da mais recente para a mais antiga" }
            }
        })
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let until = match args.text("until") {
            Some(raw) => match DateTime::parse_from_rfc3339(&raw) {
                Ok(at) => at.with_timezone(&Utc),
                Err(_) => {
                    return Ok(ToolOutput::not_found(format!(
                        "Instante '{raw}' inválido; use RFC 3339 (ex: 2026-09-21T14:00:00Z)"
                    )))
                }
            },
            None => Utc::now(),
        };
        let from = until - Duration::hours(args.integer_in("hours", 24, 1, 720));
        let limit = args
            .integer_in("limit", DEFAULT_LIMIT, 1, MAX_LIMIT)
            .unsigned_abs();
        let filters = AuditFilters {
            from: Some(from),
            to: Some(until),
            search: args.text("search"),
            resource_type: args.text("resource_type").map(|kind| kind.to_lowercase()),
            action: args.text("action").map(|action| action.to_lowercase()),
            ..AuditFilters::default()
        };
        let page = AuditService::new(&ctx.db)
            .list(filters, Some(1), Some(limit))
            .await?;
        let user_ids: Vec<i64> = page.data.iter().filter_map(|entry| entry.user_id).collect();
        let user_names: HashMap<i64, String> = users::Entity::find()
            .filter(users::Column::Id.is_in(user_ids))
            .all(&ctx.db)
            .await?
            .into_iter()
            .map(|user| (user.id, user.name))
            .collect();
        Ok(ToolOutput::data(json!({
            "from": from.to_rfc3339(),
            "until": until.to_rfc3339(),
            "total": page.meta.total,
            "entries": page.data.iter().map(|entry| json!({
                "at": entry.created_at.with_timezone(&Utc).to_rfc3339(),
                "user": entry.user_id.and_then(|id| user_names.get(&id)),
                "action": entry.action,
                "resource": entry.resource_type,
                "resource_id": entry.resource_id,
                "target": entry.resource_label,
                "description": entry.description,
                "changes": entry.changes.as_ref().map(compact_changes),
            })).collect::<Vec<_>>(),
        })))
    }
}

/// O antes/depois em uma linha, cortado no teto.
fn compact_changes(changes: &Value) -> String {
    truncate_chars(&changes.to_string(), CHANGES_MAX_CHARS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn antes_e_depois_cabem_no_teto() {
        let grande = json!({ "old": "a".repeat(2_000), "new": "b" });
        let compacto = compact_changes(&grande);
        assert_eq!(
            compacto.chars().count(),
            CHANGES_MAX_CHARS + 1,
            "corta e marca com …"
        );
        assert!(compacto.ends_with('…'));
        let pequeno = compact_changes(&json!({ "old": 1, "new": 2 }));
        assert!(pequeno.contains(r#""old":1"#) && pequeno.contains(r#""new":2"#));
    }
}
