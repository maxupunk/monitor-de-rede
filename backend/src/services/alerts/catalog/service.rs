//! Aplicação idempotente do catálogo de regras (§8.7).
//!
//! "Já existe" cobre dois casos: a regra veio do mesmo template (`template_key`)
//! ou o usuário já criou à mão uma regra com condição e escopo idênticos. Em
//! ambos, o template é ignorado — nunca duplicamos.

use std::collections::{HashMap, HashSet};

use sea_orm::{ActiveModelTrait, ConnectionTrait, Set};
use serde::Serialize;
use serde_json::Value;

use crate::{
    models::alert_rules,
    services::{
        alerts::{
            catalog::templates::{self, AlertRuleTemplate},
            repository,
        },
        shared::errors::AppResult,
    },
};

/// Template acrescido do que já existe no banco — o que a tela precisa saber.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlertRuleTemplateView {
    #[serde(flatten)]
    pub template: AlertRuleTemplate,
    /// Já existe uma regra equivalente: não será criada de novo.
    pub applied: bool,
    /// Regra existente correspondente, quando houver.
    pub rule_id: Option<i64>,
}

/// Por que um template pedido não virou regra.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    AlreadyExists,
    UnknownTemplate,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedTemplate {
    pub key: String,
    pub reason: SkipReason,
}

#[derive(Debug, Default)]
pub struct CatalogApplicationResult {
    pub created: Vec<alert_rules::Model>,
    pub skipped: Vec<SkippedTemplate>,
}

/// Assinatura que identifica uma regra por condição + escopo.
///
/// É o segundo critério de idempotência: sem ele, quem já tinha criado à mão
/// "latência > 200 ms" ganharia uma segunda regra idêntica ao aplicar o
/// catálogo, e passaria a receber dois alertas por queda.
fn signature(
    condition: &Value,
    site: Option<i64>,
    device: Option<i64>,
    monitor: Option<i64>,
) -> String {
    let text = |key: &str| {
        condition
            .get(key)
            .map_or_else(String::new, |value| match value {
                Value::String(text) => text.clone(),
                Value::Null => String::new(),
                other => other.to_string(),
            })
    };
    let id = |value: Option<i64>| value.map_or_else(String::new, |value| value.to_string());
    format!(
        "{}|{}|{}|{}|{}|{}",
        text("field"),
        text("operator"),
        text("value"),
        id(site),
        id(device),
        id(monitor)
    )
}

fn rule_signature(rule: &alert_rules::Model) -> String {
    signature(
        &rule.condition,
        rule.site_id,
        rule.device_id,
        rule.monitor_id,
    )
}

fn template_signature(template: &AlertRuleTemplate) -> String {
    // Templates nascem sempre globais: nenhuma das três dimensões é delimitada.
    signature(&template.condition, None, None, None)
}

/// Índices de tudo que já existe, para decidir sem N consultas.
struct ExistingRules {
    by_template_key: HashMap<String, i64>,
    by_signature: HashMap<String, i64>,
}

impl ExistingRules {
    async fn load<C: ConnectionTrait>(db: &C) -> AppResult<Self> {
        let mut by_template_key = HashMap::new();
        let mut by_signature = HashMap::new();
        for rule in repository::find_all(db).await? {
            if let Some(key) = rule.template_key.clone() {
                by_template_key.entry(key).or_insert(rule.id);
            }
            by_signature.entry(rule_signature(&rule)).or_insert(rule.id);
        }
        Ok(Self {
            by_template_key,
            by_signature,
        })
    }

    fn matching(&self, template: &AlertRuleTemplate) -> Option<i64> {
        self.by_template_key
            .get(template.key)
            .or_else(|| self.by_signature.get(&template_signature(template)))
            .copied()
    }

    fn remember(&mut self, template: &AlertRuleTemplate, rule_id: i64) {
        self.by_template_key
            .insert(template.key.to_string(), rule_id);
        self.by_signature
            .insert(template_signature(template), rule_id);
    }
}

/// Catálogo completo com a marcação do que já está configurado.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn describe<C: ConnectionTrait>(db: &C) -> AppResult<Vec<AlertRuleTemplateView>> {
    let existing = ExistingRules::load(db).await?;
    Ok(templates::all()
        .into_iter()
        .map(|template| {
            let rule_id = existing.matching(&template);
            AlertRuleTemplateView {
                applied: rule_id.is_some(),
                rule_id,
                template,
            }
        })
        .collect())
}

/// Cria as regras das chaves informadas, pulando as que já existem.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn apply<C: ConnectionTrait>(
    db: &C,
    keys: &[String],
) -> AppResult<CatalogApplicationResult> {
    let mut result = CatalogApplicationResult::default();
    let mut existing = ExistingRules::load(db).await?;
    let mut seen = HashSet::new();

    for key in keys {
        if !seen.insert(key.clone()) {
            continue;
        }
        let Some(template) = templates::find(key) else {
            result.skipped.push(SkippedTemplate {
                key: key.clone(),
                reason: SkipReason::UnknownTemplate,
            });
            continue;
        };
        if existing.matching(&template).is_some() {
            result.skipped.push(SkippedTemplate {
                key: key.clone(),
                reason: SkipReason::AlreadyExists,
            });
            continue;
        }

        let rule = alert_rules::ActiveModel {
            name: Set(template.name.to_string()),
            r#type: Set(template.rule_type.to_string()),
            template_key: Set(Some(template.key.to_string())),
            condition: Set(template.condition.clone()),
            severity: Set(template.severity.to_string()),
            duration_seconds: Set(template.duration_seconds),
            recovery_window_seconds: Set(template.recovery_window_seconds),
            enabled: Set(true),
            ..Default::default()
        }
        .insert(db)
        .await?;

        // Mantém os índices coerentes dentro do próprio lote (evita duplicar
        // quando duas chaves resolvem para a mesma condição).
        existing.remember(&template, rule.id);
        result.created.push(rule);
    }

    Ok(result)
}

/// Provisiona o conjunto básico de regras em instalações novas.
///
/// Só age quando não existe regra alguma: quem já opera o sistema decide o que
/// manter, e uma regra apagada de propósito não pode ressuscitar no restart
/// (matriz de paridade #27).
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn ensure_defaults<C: ConnectionTrait>(db: &C) -> AppResult<CatalogApplicationResult> {
    if repository::count(db).await? > 0 {
        return Ok(CatalogApplicationResult::default());
    }
    let keys: Vec<String> = templates::recommended()
        .into_iter()
        .map(|template| template.key.to_string())
        .collect();
    apply(db, &keys).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn assinatura_normaliza_valor_textual_e_numerico_para_a_mesma_chave() {
        // A chave é montada sobre o valor em texto: `200` e `"200"` colidem de
        // propósito, porque descrevem a mesma regra para o operador.
        let numerica = signature(
            &json!({ "field": "latencyMs", "operator": "gt", "value": 200 }),
            None,
            None,
            None,
        );
        let textual = signature(
            &json!({ "field": "latencyMs", "operator": "gt", "value": "200" }),
            None,
            None,
            None,
        );
        assert_eq!(numerica, textual);
        assert_eq!(numerica, "latencyMs|gt|200|||");
    }

    #[test]
    fn escopo_diferente_produz_assinatura_diferente() {
        let condition = json!({ "field": "status", "operator": "eq", "value": "down" });
        assert_ne!(
            signature(&condition, None, None, None),
            signature(&condition, None, Some(7), None)
        );
    }

    #[test]
    fn view_do_catalogo_achata_o_template_para_o_frontend() {
        let view = AlertRuleTemplateView {
            template: templates::find("device_offline").expect("template existe"),
            applied: true,
            rule_id: Some(4),
        };
        let json = serde_json::to_value(&view).unwrap();
        // O frontend lê tudo no mesmo nível do objeto (`AlertRuleTemplate`).
        assert_eq!(json["key"], "device_offline");
        assert_eq!(json["severity"], "critical");
        assert_eq!(json["applied"], true);
        assert_eq!(json["ruleId"], 4);
    }

    #[test]
    fn motivo_do_skip_usa_o_vocabulario_do_frontend() {
        assert_eq!(
            serde_json::to_value(SkipReason::AlreadyExists).unwrap(),
            json!("already_exists")
        );
        assert_eq!(
            serde_json::to_value(SkipReason::UnknownTemplate).unwrap(),
            json!("unknown_template")
        );
    }
}
