//! Avaliação de uma condição de regra contra os fatos observados (§8.7).
//!
//! O comportamento é porte literal do `rule_evaluator.ts`. A parte não óbvia
//! está em `eq`/`neq`: no JavaScript eles usam `===`, **sem coerção**, e é por
//! isso que o template `snmp_interface_oper_down` compara com a string `"2"`.
//! Consertar isso aqui silenciaria a regra de todo mundo que já a aplicou.

use serde_json::Value;

use super::contracts::AlertDataset;

/// Comparadores oferecidos na tela de regras.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    Eq,
    Neq,
    Gt,
    Gte,
    Lt,
    Lte,
    Contains,
}

impl Operator {
    /// Forma persistida em `alert_rules.condition.operator`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Eq => "eq",
            Self::Neq => "neq",
            Self::Gt => "gt",
            Self::Gte => "gte",
            Self::Lt => "lt",
            Self::Lte => "lte",
            Self::Contains => "contains",
        }
    }

    /// `None` para operador desconhecido — o `default` do `switch` original
    /// devolvia `false`, e quem chama traduz isso em "regra não bateu".
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "eq" => Some(Self::Eq),
            "neq" => Some(Self::Neq),
            "gt" => Some(Self::Gt),
            "gte" => Some(Self::Gte),
            "lt" => Some(Self::Lt),
            "lte" => Some(Self::Lte),
            "contains" => Some(Self::Contains),
            _ => None,
        }
    }
}

/// Condição de uma regra, já normalizada para `{field, operator, value}`.
#[derive(Debug, Clone)]
pub struct AlertRuleCondition {
    pub field: String,
    pub operator: Option<Operator>,
    pub value: Value,
}

impl AlertRuleCondition {
    /// Lê a condição do JSON gravado na regra.
    ///
    /// Devolve `None` quando `field` ou `operator` não são strings — o mesmo
    /// critério do `normalizeCondition` do controller, para uma linha corrompida
    /// no banco nunca derrubar o ciclo do scheduler.
    #[must_use]
    pub fn from_json(raw: &Value) -> Option<Self> {
        let object = raw.as_object()?;
        let field = object.get("field")?.as_str()?.to_string();
        let operator_raw = object.get("operator")?.as_str()?;
        Some(Self {
            field,
            operator: Operator::parse(operator_raw),
            value: object.get("value").cloned().unwrap_or(Value::Null),
        })
    }
}

/// `true` quando o fato observado satisfaz a condição.
#[must_use]
pub fn evaluate(condition: &AlertRuleCondition, dataset: &AlertDataset) -> bool {
    // Campo ausente ou nulo nunca dispara: a regra fala de uma medida que este
    // ciclo não produziu, e não de uma medida que veio zerada.
    let field_value = match dataset.get(&condition.field) {
        None | Some(Value::Null) => return false,
        Some(value) => value,
    };

    let Some(operator) = condition.operator else {
        return false;
    };

    match operator {
        Operator::Eq => strict_equals(field_value, &condition.value),
        Operator::Neq => !strict_equals(field_value, &condition.value),
        Operator::Gt => compare(field_value, &condition.value, |a, b| a > b),
        Operator::Gte => compare(field_value, &condition.value, |a, b| a >= b),
        Operator::Lt => compare(field_value, &condition.value, |a, b| a < b),
        Operator::Lte => compare(field_value, &condition.value, |a, b| a <= b),
        Operator::Contains => as_string(field_value).contains(&as_string(&condition.value)),
    }
}

/// Equivalente ao `===` do JavaScript.
///
/// Dois números comparam pelo valor numérico e não pela representação: o JSON
/// da regra pode trazer `200` (inteiro) enquanto a métrica chega como `200.0`
/// (`f64` do checker), e para o `===` do original isso é o mesmo número. Entre
/// tipos diferentes não há coerção alguma — `2` nunca é igual a `"2"`.
fn strict_equals(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(a), Value::Number(b)) => match (a.as_f64(), b.as_f64()) {
            (Some(a), Some(b)) => a == b,
            _ => a == b,
        },
        _ => left == right,
    }
}

/// Comparação numérica: reproduz o `Number(x)` do JavaScript, inclusive o
/// `NaN` que faz toda comparação virar `false`.
fn compare(left: &Value, right: &Value, ordering: impl Fn(f64, f64) -> bool) -> bool {
    match (as_number(left), as_number(right)) {
        (Some(a), Some(b)) => ordering(a, b),
        _ => false,
    }
}

/// `Number(value)` do JavaScript, restrito ao que um dataset produz.
fn as_number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        // `Number("10")` é 10; `Number("abc")` é NaN e desqualifica a comparação.
        Value::String(text) => text.trim().parse::<f64>().ok(),
        Value::Bool(flag) => Some(if *flag { 1.0 } else { 0.0 }),
        _ => None,
    }
}

/// `String(value)` do JavaScript, para o `contains`.
fn as_string(value: &Value) -> String {
    match value {
        // `as_str` evita as aspas que o `to_string` do serde acrescentaria.
        Value::String(text) => text.clone(),
        Value::Null => "null".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn dataset(pairs: &[(&str, Value)]) -> AlertDataset {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), value.clone()))
            .collect()
    }

    fn condition(field: &str, operator: &str, value: Value) -> AlertRuleCondition {
        AlertRuleCondition::from_json(&json!({
            "field": field, "operator": operator, "value": value
        }))
        .expect("condição válida")
    }

    #[test]
    fn campo_ausente_ou_nulo_nunca_dispara() {
        let facts = dataset(&[("latencyMs", Value::Null)]);
        assert!(!evaluate(&condition("latencyMs", "gt", json!(10)), &facts));
        assert!(!evaluate(&condition("packetLoss", "gt", json!(10)), &facts));
        // `neq` também não dispara: no original o retorno antecipado vem antes
        // de olhar o operador.
        assert!(!evaluate(&condition("latencyMs", "neq", json!(1)), &facts));
    }

    #[test]
    fn eq_compara_sem_coercao_como_o_javascript() {
        // Matriz de paridade #28: o template `snmp_interface_oper_down` grava
        // `"2"` string; um `2` numérico não pode satisfazê-lo.
        let numerico = dataset(&[("ifOperStatus", json!(2))]);
        assert!(!evaluate(
            &condition("ifOperStatus", "eq", json!("2")),
            &numerico
        ));
        let textual = dataset(&[("ifOperStatus", json!("2"))]);
        assert!(evaluate(
            &condition("ifOperStatus", "eq", json!("2")),
            &textual
        ));
    }

    #[test]
    fn eq_ignora_a_representacao_do_numero() {
        let facts = dataset(&[("latencyMs", json!(200.0))]);
        assert!(evaluate(&condition("latencyMs", "eq", json!(200)), &facts));
        assert!(!evaluate(
            &condition("latencyMs", "neq", json!(200)),
            &facts
        ));
    }

    #[test]
    fn comparadores_numericos_aceitam_string_numerica_e_recusam_texto() {
        let facts = dataset(&[("latencyMs", json!("250"))]);
        assert!(evaluate(&condition("latencyMs", "gt", json!(200)), &facts));
        let lixo = dataset(&[("latencyMs", json!("indisponível"))]);
        assert!(!evaluate(&condition("latencyMs", "gt", json!(200)), &lixo));
        assert!(!evaluate(&condition("latencyMs", "lt", json!(200)), &lixo));
    }

    #[test]
    fn limites_de_gte_e_lte() {
        let facts = dataset(&[("statusCode", json!(400))]);
        assert!(evaluate(
            &condition("statusCode", "gte", json!(400)),
            &facts
        ));
        assert!(!evaluate(
            &condition("statusCode", "gt", json!(400)),
            &facts
        ));
        assert!(evaluate(
            &condition("statusCode", "lte", json!(400)),
            &facts
        ));
    }

    #[test]
    fn contains_compara_como_texto() {
        let facts = dataset(&[("interfaceName", json!("GigabitEthernet0/1"))]);
        assert!(evaluate(
            &condition("interfaceName", "contains", json!("Gigabit")),
            &facts
        ));
        assert!(!evaluate(
            &condition("interfaceName", "contains", json!("Serial")),
            &facts
        ));
        // `String(200)` é "200": um número na condição não quebra o operador.
        let numerico = dataset(&[("statusCode", json!(503))]);
        assert!(evaluate(
            &condition("statusCode", "contains", json!(50)),
            &numerico
        ));
    }

    #[test]
    fn operador_desconhecido_nao_dispara() {
        let facts = dataset(&[("status", json!("down"))]);
        let regra = AlertRuleCondition::from_json(&json!({
            "field": "status", "operator": "aproximadamente", "value": "down"
        }))
        .expect("condição com operador desconhecido ainda é legível");
        assert!(regra.operator.is_none());
        assert!(!evaluate(&regra, &facts));
    }

    #[test]
    fn condicao_corrompida_nao_vira_panico() {
        assert!(AlertRuleCondition::from_json(&json!({ "operator": "eq" })).is_none());
        assert!(AlertRuleCondition::from_json(&json!({ "field": 1, "operator": "eq" })).is_none());
        assert!(AlertRuleCondition::from_json(&json!([])).is_none());
        // `value` ausente é lido como `null`, não como erro.
        let sem_valor =
            AlertRuleCondition::from_json(&json!({ "field": "status", "operator": "eq" }))
                .expect("field e operator bastam");
        assert_eq!(sem_valor.value, Value::Null);
    }
}
