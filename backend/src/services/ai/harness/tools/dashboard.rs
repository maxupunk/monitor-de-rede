//! O layout do dashboard pelo chat: ler os cards e o catálogo, e propor
//! mudanças que só são gravadas depois que o usuário confirmar.
//!
//! As regras (o que pode repetir, posições, validação da configuração) são de
//! `services::dashboard`; aqui só se lê o argumento da IA e se resume o
//! resultado. O layout é compartilhado — a mudança aparece para todos que
//! usam o layout do servidor —, então toda chamada de
//! [`UpdateDashboardLayout`] pede confirmação, também no modo automático.

use async_trait::async_trait;
use loco_rs::prelude::AppContext;
use serde_json::{json, Value};

use super::{as_output, AiToolHandler, RiskAssessment, ToolArgs, ToolGroup, ToolKind, ToolOutput};
use crate::services::{
    dashboard::{
        self,
        catalog::{DashboardWidgetSize, DashboardWidgetType},
        layout::{CONFIG_KEYS, TIMEFRAMES},
        DashboardLayout, LayoutOperation,
    },
    shared::errors::{AppError, AppResult},
};

/// Mais que isso numa chamada é refazer o dashboard: melhor o `reset`.
const MAX_OPERATIONS: usize = 30;

/// Os cards como a IA precisa: posição entre os visíveis, id, tipo, título,
/// tamanho e configuração. Oculto vem sem posição.
fn widgets_view(layout: &DashboardLayout) -> Vec<Value> {
    layout
        .widgets()
        .iter()
        .map(|widget| {
            let mut item = json!({
                "position": layout.position_of(&widget.id),
                "id": widget.id,
                "type": widget.type_id(),
                "title": widget.label(),
                "size": widget.size().id(),
                "visible": widget.visible,
            });
            if !widget.config.is_empty() {
                item["config"] = Value::Object(widget.config.clone());
            }
            item
        })
        .collect()
}

fn catalog_view() -> Vec<Value> {
    DashboardWidgetType::ALL
        .into_iter()
        .map(|kind| {
            let spec = kind.spec();
            json!({
                "type": kind.id(),
                "title": spec.title,
                "description": spec.description,
                "multiple": spec.multiple,
                "default_size": spec.default_size.id(),
                "config": spec.config_keys,
            })
        })
        .collect()
}

/// As operações pedidas. Um objeto solto vale como lista de um.
fn operations(args: &ToolArgs) -> AppResult<Vec<LayoutOperation>> {
    let items = match args.raw("operations") {
        Some(Value::Array(items)) => items.clone(),
        Some(single @ Value::Object(_)) => vec![single.clone()],
        _ => Vec::new(),
    };
    if items.is_empty() {
        return Err(AppError::validation(
            "Informe operations: a lista de mudanças no layout",
        ));
    }
    if items.len() > MAX_OPERATIONS {
        return Err(AppError::validation(format!(
            "No máximo {MAX_OPERATIONS} operações por vez"
        )));
    }
    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            LayoutOperation::parse(item)
                .map_err(|error| AppError::validation(format!("Operação {}: {error}", index + 1)))
        })
        .collect()
}

pub struct GetDashboardLayout;

#[async_trait]
impl AiToolHandler for GetDashboardLayout {
    fn name(&self) -> &'static str {
        "get_dashboard_layout"
    }

    fn description(&self) -> &'static str {
        "Layout do dashboard (compartilhado): os cards na ordem, com id, tipo, título, tamanho, visibilidade e configuração, e o catálogo de cards que podem entrar. Consulte antes de update_dashboard_layout."
    }

    fn parameters(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Dashboard
    }

    fn schedulable(&self) -> bool {
        false
    }

    async fn execute(&self, ctx: &AppContext, _args: &ToolArgs) -> AppResult<ToolOutput> {
        let layout = dashboard::load(&ctx.db).await?;
        Ok(ToolOutput::data(json!({
            "widgets": widgets_view(&layout),
            "catalog": catalog_view(),
            "sizes": DashboardWidgetSize::ALL.map(DashboardWidgetSize::id),
            "timeframes": TIMEFRAMES,
            "note": "position conta só os cards visíveis, a partir de 0. Card de tipo único (multiple=false) tem id igual ao tipo.",
        })))
    }
}

pub struct UpdateDashboardLayout;

#[async_trait]
impl AiToolHandler for UpdateDashboardLayout {
    fn name(&self) -> &'static str {
        "update_dashboard_layout"
    }

    fn description(&self) -> &'static str {
        "Propõe mudanças no layout do dashboard; só grava depois que o usuário confirmar, e vale para todos que usam o layout do servidor. As operações rodam em ordem, tudo ou nada. Pegue ids e tipos em get_dashboard_layout."
    }

    fn parameters(&self) -> Value {
        let types: Vec<&str> = DashboardWidgetType::ALL
            .into_iter()
            .map(DashboardWidgetType::id)
            .collect();
        json!({
            "type": "object",
            "properties": {
                "operations": {
                    "type": "array",
                    "minItems": 1,
                    "items": {
                        "type": "object",
                        "properties": {
                            "op": { "type": "string", "enum": ["add", "remove", "move", "resize", "configure", "reset"] },
                            "id": { "type": "string", "description": "Card alvo (remove, move, resize, configure)" },
                            "type": { "type": "string", "enum": types, "description": "Tipo do card novo (add)" },
                            "size": { "type": "string", "enum": DashboardWidgetSize::ALL.map(DashboardWidgetSize::id) },
                            "position": { "type": "integer", "minimum": 0, "description": "Entre os visíveis, a partir de 0 (add, move)" },
                            "title": { "type": "string", "description": "add/configure; vazio volta ao padrão" },
                            "config": {
                                "type": "object",
                                "description": format!("add/configure: {}. timeframe: {}. Ids numéricos ou \"all\"; null remove a chave", CONFIG_KEYS.join(", "), TIMEFRAMES.join("|"))
                            }
                        },
                        "required": ["op"]
                    }
                }
            },
            "required": ["operations"]
        })
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Action
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Dashboard
    }

    /// O layout é de todos: nada de mudar sem a pessoa ver o resumo.
    fn schedulable(&self) -> bool {
        false
    }

    async fn risk(&self, _ctx: &AppContext, _args: &ToolArgs) -> RiskAssessment {
        RiskAssessment::high(
            "Muda o dashboard compartilhado de todos os usuários; sempre pede confirmação",
        )
    }

    async fn preview(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<String> {
        let changes = match operations(args) {
            Ok(operations) => dashboard::preview(&ctx.db, &operations).await,
            Err(error) => Err(error),
        };
        Ok(match as_output(changes)? {
            Ok(changes) => {
                let lines: Vec<String> = changes
                    .iter()
                    .enumerate()
                    .map(|(index, change)| format!("{}. {change}", index + 1))
                    .collect();
                format!(
                    "Alterar o layout do dashboard (vale para todos que usam o layout do servidor):\n{}",
                    lines.join("\n")
                )
            }
            Err(output) => format!(
                "Ação inválida: {}",
                output.data["error"]
                    .as_str()
                    .unwrap_or("argumentos incompletos")
            ),
        })
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let operations = match as_output(operations(args))? {
            Ok(operations) => operations,
            Err(output) => return Ok(output),
        };
        let (layout, changes) = match as_output(dashboard::apply(ctx, &operations).await)? {
            Ok(applied) => applied,
            Err(output) => return Ok(output),
        };
        Ok(ToolOutput::data(json!({
            "done": true,
            "changes": changes,
            "widgets": widgets_view(&layout),
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operacoes_aceitam_objeto_solto_e_recusam_lista_vazia() {
        let um = ToolArgs::from_value(json!({ "operations": { "op": "reset" } }));
        assert_eq!(operations(&um).unwrap(), vec![LayoutOperation::Reset]);

        let vazio = ToolArgs::from_value(json!({ "operations": [] }));
        assert!(operations(&vazio).is_err());

        let ruim =
            ToolArgs::from_value(json!({ "operations": [{ "op": "reset" }, { "op": "x" }] }));
        let erro = operations(&ruim).unwrap_err().to_string();
        assert!(erro.starts_with("Operação 2:"), "{erro}");
    }

    #[test]
    fn visao_da_ia_traz_posicao_so_dos_visiveis() {
        let mut layout = DashboardLayout::default_layout();
        layout.remove("health_gauge").unwrap();
        let cards = widgets_view(&layout);
        assert_eq!(cards[0]["position"], 0);
        assert_eq!(cards[1]["id"], "health_gauge");
        assert!(cards[1]["position"].is_null());
        assert_eq!(cards[2]["position"], 1);
        assert_eq!(cards[0]["size"], "full");
        assert_eq!(catalog_view().len(), DashboardWidgetType::ALL.len());
    }
}
