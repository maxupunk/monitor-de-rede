//! Validação dos parâmetros de uma ação contra o esquema do manifesto.
//!
//! É um subconjunto deliberado do JSON Schema — o que um formulário gerado
//! consegue desenhar e o que protege o equipamento: `type` (`object` na raiz;
//! `string`, `integer`, `number`, `boolean` nas propriedades), `required`,
//! `enum`, `pattern`, `minLength`/`maxLength` e `minimum`/`maximum`. Esquema
//! que usa algo fora disso é recusado na validação do manifesto, e não
//! ignorado: um `format` silenciosamente ignorado seria uma validação que o
//! autor acha que existe.
//!
//! O `pattern` é a defesa contra injeção de comando: parâmetro que vai para uma
//! linha de shell deveria ter um (e a revisão estática aponta quando não tem).

use regex::Regex;
use serde_json::{Map, Value};

const PROPERTY_TYPES: &[&str] = &["string", "integer", "number", "boolean"];
const PROPERTY_KEYS: &[&str] = &[
    "type",
    "title",
    "description",
    "enum",
    "pattern",
    "minLength",
    "maxLength",
    "minimum",
    "maximum",
    "default",
];

/// Confere se o esquema usa só o subconjunto suportado.
///
/// # Errors
///
/// Mensagem em português apontando a propriedade problemática.
pub fn validate_schema(schema: &Value) -> Result<(), String> {
    let root = schema
        .as_object()
        .ok_or("o esquema de parâmetros precisa ser um objeto")?;
    if root.get("type").and_then(Value::as_str) != Some("object") {
        return Err("o esquema de parâmetros precisa ter \"type\": \"object\"".into());
    }
    let properties = match root.get("properties") {
        None => return Ok(()),
        Some(Value::Object(properties)) => properties,
        Some(_) => return Err("\"properties\" precisa ser um objeto".into()),
    };
    for (name, property) in properties {
        let property = property
            .as_object()
            .ok_or_else(|| format!("a propriedade `{name}` precisa ser um objeto"))?;
        for key in property.keys() {
            if !PROPERTY_KEYS.contains(&key.as_str()) {
                return Err(format!(
                    "a propriedade `{name}` usa `{key}`, que não é suportado"
                ));
            }
        }
        let kind = property
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("a propriedade `{name}` precisa de `type`"))?;
        if !PROPERTY_TYPES.contains(&kind) {
            return Err(format!(
                "a propriedade `{name}` tem tipo `{kind}`; use string, integer, number ou boolean"
            ));
        }
        if let Some(pattern) = property.get("pattern") {
            let pattern = pattern
                .as_str()
                .ok_or_else(|| format!("o `pattern` de `{name}` precisa ser texto"))?;
            Regex::new(pattern)
                .map_err(|error| format!("o `pattern` de `{name}` é inválido: {error}"))?;
        }
    }
    if let Some(required) = root.get("required") {
        let required = required
            .as_array()
            .ok_or("\"required\" precisa ser uma lista")?;
        for item in required {
            let name = item.as_str().ok_or("\"required\" só aceita nomes")?;
            if !properties.contains_key(name) {
                return Err(format!(
                    "`{name}` está em \"required\" mas não em \"properties\""
                ));
            }
        }
    }
    Ok(())
}

/// Valida `params` e devolve o objeto com os valores padrão aplicados.
///
/// # Errors
///
/// Lista de problemas, um por campo, em português.
pub fn validate(schema: Option<&Value>, params: &Value) -> Result<Value, Vec<String>> {
    let empty = Map::new();
    let given = match params {
        Value::Null => &empty,
        Value::Object(map) => map,
        _ => return Err(vec!["os parâmetros precisam ser um objeto".into()]),
    };
    let Some(schema) = schema else {
        return if given.is_empty() {
            Ok(Value::Object(Map::new()))
        } else {
            Err(vec!["esta ação não recebe parâmetros".into()])
        };
    };
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let required: Vec<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();

    let mut errors = Vec::new();
    let mut output = Map::new();
    for key in given.keys() {
        if !properties.contains_key(key) {
            errors.push(format!("parâmetro desconhecido: `{key}`"));
        }
    }
    for (name, property) in &properties {
        let value = given
            .get(name)
            .filter(|value| !value.is_null())
            .or_else(|| property.get("default"));
        let Some(value) = value else {
            if required.contains(&name.as_str()) {
                errors.push(format!("`{name}` é obrigatório"));
            }
            continue;
        };
        match check_property(name, property, value) {
            Ok(value) => {
                output.insert(name.clone(), value);
            }
            Err(error) => errors.push(error),
        }
    }
    if errors.is_empty() {
        Ok(Value::Object(output))
    } else {
        Err(errors)
    }
}

fn check_property(name: &str, property: &Value, value: &Value) -> Result<Value, String> {
    let kind = property
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("string");
    let value = match (kind, value) {
        ("string", Value::String(_))
        | ("boolean", Value::Bool(_))
        | ("number", Value::Number(_)) => value.clone(),
        ("integer", Value::Number(number)) if number.is_i64() || number.is_u64() => value.clone(),
        _ => return Err(format!("`{name}` precisa ser do tipo {kind}")),
    };
    if let Some(options) = property.get("enum").and_then(Value::as_array) {
        if !options.contains(&value) {
            return Err(format!("`{name}` não está entre as opções permitidas"));
        }
    }
    if let Value::String(text) = &value {
        let length = text.chars().count() as u64;
        if let Some(min) = property.get("minLength").and_then(Value::as_u64) {
            if length < min {
                return Err(format!("`{name}` precisa ter ao menos {min} caracteres"));
            }
        }
        if let Some(max) = property.get("maxLength").and_then(Value::as_u64) {
            if length > max {
                return Err(format!("`{name}` aceita no máximo {max} caracteres"));
            }
        }
        if let Some(pattern) = property.get("pattern").and_then(Value::as_str) {
            // Ancorado: o autor escreve `[a-z]+` pensando no valor inteiro, e
            // sem âncora `rm -rf / ; a` passaria por conter um `a`.
            let anchored = format!("^(?:{pattern})$");
            let re = Regex::new(&anchored).map_err(|error| error.to_string())?;
            if !re.is_match(text) {
                return Err(format!("`{name}` tem formato inválido"));
            }
        }
    }
    if let Some(number) = value.as_f64() {
        if let Some(min) = property.get("minimum").and_then(Value::as_f64) {
            if number < min {
                return Err(format!("`{name}` precisa ser no mínimo {min}"));
            }
        }
        if let Some(max) = property.get("maximum").and_then(Value::as_f64) {
            if number > max {
                return Err(format!("`{name}` precisa ser no máximo {max}"));
            }
        }
    }
    Ok(value)
}

/// Propriedades `string` sem `pattern` nem `enum` — as que podem carregar
/// injeção para uma linha de shell. Usado pela revisão estática.
#[must_use]
pub fn unconstrained_strings(schema: Option<&Value>) -> Vec<String> {
    schema
        .and_then(|schema| schema.get("properties"))
        .and_then(Value::as_object)
        .map(|properties| {
            properties
                .iter()
                .filter(|(_, property)| {
                    property.get("type").and_then(Value::as_str) == Some("string")
                        && property.get("pattern").is_none()
                        && property.get("enum").is_none()
                })
                .map(|(name, _)| name.clone())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "pattern": "[a-z0-9._+-]+", "maxLength": 64 },
                "count": { "type": "integer", "minimum": 1, "maximum": 10, "default": 3 },
                "mode": { "type": "string", "enum": ["fast", "safe"] }
            },
            "required": ["name"]
        })
    }

    #[test]
    fn esquema_suportado_passa_e_desconhecido_e_recusado() {
        assert!(validate_schema(&schema()).is_ok());
        assert!(validate_schema(&json!({"type": "array"})).is_err());
        assert!(validate_schema(&json!({
            "type": "object",
            "properties": { "x": { "type": "string", "format": "ipv4" } }
        }))
        .is_err());
        assert!(validate_schema(&json!({
            "type": "object", "properties": {}, "required": ["fantasma"]
        }))
        .is_err());
    }

    #[test]
    fn aplica_padrao_e_valida_tipos() {
        let ok = validate(Some(&schema()), &json!({"name": "luci-app-sqm"})).unwrap();
        assert_eq!(ok, json!({"name": "luci-app-sqm", "count": 3}));
        assert!(validate(Some(&schema()), &json!({})).is_err());
        assert!(validate(Some(&schema()), &json!({"name": "x", "count": "3"})).is_err());
        assert!(validate(Some(&schema()), &json!({"name": "x", "extra": 1})).is_err());
    }

    #[test]
    fn pattern_e_ancorado_contra_injecao() {
        let error = validate(Some(&schema()), &json!({"name": "a; rm -rf /"})).unwrap_err();
        assert!(error[0].contains("formato inválido"));
    }

    #[test]
    fn acao_sem_esquema_nao_aceita_parametro() {
        assert_eq!(validate(None, &Value::Null).unwrap(), json!({}));
        assert!(validate(None, &json!({"x": 1})).is_err());
    }

    #[test]
    fn strings_sem_restricao_sao_apontadas() {
        let schema = json!({
            "type": "object",
            "properties": {
                "livre": { "type": "string" },
                "presa": { "type": "string", "pattern": "[a-z]+" }
            }
        });
        assert_eq!(
            unconstrained_strings(Some(&schema)),
            vec!["livre".to_string()]
        );
    }
}
