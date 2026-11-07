//! Os argumentos de uma tarefa, conferidos contra o JSON Schema da
//! ferramenta — o mesmo que a IA recebe.
//!
//! A conferência acontece ao salvar, para o erro aparecer no formulário (ou
//! voltar para a IA corrigir) e não às 03:00, no histórico. É de propósito
//! só o subconjunto que os contratos do catálogo usam: `type`, `enum`,
//! `required` e `items`.

use serde_json::{Map, Value};

/// Tira o que o formulário manda vazio (`null`, `""`, `[]`): campo opcional
/// em branco é campo ausente, e a ferramenta usa o padrão dela.
#[must_use]
pub fn clean(arguments: &Value) -> Value {
    let Some(object) = arguments.as_object() else {
        return arguments.clone();
    };
    let kept: Map<String, Value> = object
        .iter()
        .filter(|(_, value)| match value {
            Value::Null => false,
            Value::String(text) => !text.trim().is_empty(),
            Value::Array(items) => !items.is_empty(),
            _ => true,
        })
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    Value::Object(kept)
}

/// Os problemas dos argumentos, em português; vazio quando estão certos.
#[must_use]
pub fn problems(schema: &Value, arguments: &Value) -> Vec<String> {
    let Some(arguments) = arguments.as_object() else {
        return vec!["Os parâmetros precisam ser um objeto JSON".to_string()];
    };
    let empty = Map::new();
    let properties = schema["properties"].as_object().unwrap_or(&empty);
    let mut problems = Vec::new();
    for key in arguments.keys() {
        if !properties.contains_key(key) {
            let accepted: Vec<&str> = properties.keys().map(String::as_str).collect();
            problems.push(if accepted.is_empty() {
                format!("Parâmetro desconhecido: {key} (esta ferramenta não recebe parâmetros)")
            } else {
                format!(
                    "Parâmetro desconhecido: {key} (aceitos: {})",
                    accepted.join(", ")
                )
            });
        }
    }
    for required in schema["required"].as_array().into_iter().flatten() {
        if let Some(key) = required.as_str() {
            if !arguments.contains_key(key) {
                problems.push(format!("Falta o parâmetro obrigatório: {key}"));
            }
        }
    }
    for (key, value) in arguments {
        if let Some(spec) = properties.get(key) {
            if let Some(problem) = check_value(spec, value) {
                problems.push(format!("{key}: {problem}"));
            }
        }
    }
    problems
}

fn integer_like(value: &Value) -> bool {
    value.is_i64()
        || value.is_u64()
        || value
            .as_str()
            .is_some_and(|text| text.trim().parse::<i64>().is_ok())
}

fn number_like(value: &Value) -> bool {
    value.is_number()
        || value
            .as_str()
            .is_some_and(|text| text.trim().parse::<f64>().is_ok())
}

fn check_value(spec: &Value, value: &Value) -> Option<String> {
    if let Some(options) = spec["enum"].as_array() {
        if !options.contains(value) {
            let names: Vec<String> = options
                .iter()
                .map(|option| {
                    option
                        .as_str()
                        .map_or_else(|| option.to_string(), str::to_owned)
                })
                .collect();
            return Some(format!("use um destes valores: {}", names.join(", ")));
        }
    }
    let ok = match spec["type"].as_str() {
        Some("string") => value.is_string(),
        Some("integer") => integer_like(value),
        Some("number") => number_like(value),
        Some("boolean") => value.is_boolean(),
        Some("object") => value.is_object(),
        Some("array") => {
            let Some(items) = value.as_array() else {
                return Some("esperava uma lista".to_string());
            };
            return items
                .iter()
                .find_map(|item| check_value(&spec["items"], item))
                .map(|problem| format!("item da lista — {problem}"));
        }
        // Sem `type`: aceita qualquer valor (ex.: `value` das regras).
        _ => true,
    };
    (!ok).then(|| {
        let expected = match spec["type"].as_str() {
            Some("string") => "um texto",
            Some("integer") => "um número inteiro",
            Some("number") => "um número",
            Some("boolean") => "verdadeiro ou falso",
            _ => "um objeto",
        };
        format!("esperava {expected}")
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn esquema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "target": { "type": "string" },
                "count": { "type": "integer" },
                "action": { "type": "string", "enum": ["start", "stop"] },
                "ports": { "type": "array", "items": { "type": "integer" } },
                "verbose": { "type": "boolean" },
                "value": {}
            },
            "required": ["target"]
        })
    }

    #[test]
    fn argumentos_certos_nao_tem_problema() {
        let certos = json!({ "target": "8.8.8.8", "count": 4, "ports": [22, 80], "value": "down" });
        assert!(problems(&esquema(), &certos).is_empty());
        assert!(
            problems(&esquema(), &json!({ "target": "x", "count": "5" })).is_empty(),
            "número em texto é aceito, como a ferramenta aceita"
        );
    }

    #[test]
    fn aponta_cada_problema_em_portugues() {
        let errados =
            json!({ "count": "muitos", "action": "restart", "ports": [22, "x"], "extra": 1 });
        let lista = problems(&esquema(), &errados);
        assert!(lista
            .iter()
            .any(|p| p.starts_with("Parâmetro desconhecido: extra")));
        assert!(lista.contains(&"Falta o parâmetro obrigatório: target".to_string()));
        assert!(lista.contains(&"count: esperava um número inteiro".to_string()));
        assert!(lista.contains(&"action: use um destes valores: start, stop".to_string()));
        assert!(lista.iter().any(|p| p.starts_with("ports: item da lista")));
        assert_eq!(
            problems(&esquema(), &json!([])),
            vec!["Os parâmetros precisam ser um objeto JSON".to_string()]
        );
    }

    #[test]
    fn campo_vazio_do_formulario_vira_ausente() {
        let limpo = clean(
            &json!({ "target": "a", "count": null, "note": " ", "ports": [], "verbose": false }),
        );
        assert_eq!(limpo, json!({ "target": "a", "verbose": false }));
    }
}
