//! Validação de parâmetros de ação e de configuração de plugin contra o
//! esquema do manifesto.
//!
//! É um subconjunto deliberado do JSON Schema — o que um formulário gerado
//! consegue desenhar e o que protege o equipamento:
//!
//! * raiz `type: object`, com `properties` e `required`;
//! * propriedades `string`, `integer`, `number`, `boolean` com `enum`,
//!   `pattern`, `minLength`/`maxLength`, `minimum`/`maximum`, `default`,
//!   `title`, `description` — e `secret: true` em texto (senha: cifrada ao
//!   gravar, mascarada ao ler);
//! * `array` de escalares (lista de nomes) ou de objetos com propriedades
//!   escalares (a lista de SSIDs), com `minItems`/`maxItems`. Itens de objeto
//!   ganham um `id` estável, gerado pelo sistema — é por ele que uma senha
//!   mantida ("********") volta ao item certo.
//!
//! Esquema que usa algo fora disso é recusado na validação do manifesto, e não
//! ignorado: um `format` silenciosamente ignorado seria uma validação que o
//! autor acha que existe.
//!
//! O `pattern` é a defesa contra injeção de comando: parâmetro que vai para uma
//! linha de shell deveria ter um (e a revisão estática aponta quando não tem).

use regex::Regex;
use serde_json::{Map, Value};

const SCALAR_TYPES: &[&str] = &["string", "integer", "number", "boolean"];
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
    "secret",
    "source",
    "enumTitles",
    "advanced",
    "hidden",
    "items",
    "minItems",
    "maxItems",
];
/// Chave implícita dos itens de objeto (ver a nota do módulo).
pub const ITEM_ID: &str = "id";

/// O que a tela e o histórico recebem no lugar de um segredo.
pub const SECRET_MASK: &str = "********";

/// Confere se o esquema usa só o subconjunto suportado.
///
/// # Errors
///
/// Mensagem em português apontando a propriedade problemática.
pub fn validate_schema(schema: &Value) -> Result<(), String> {
    validate_object_schema(schema, "", true)
}

fn validate_object_schema(schema: &Value, path: &str, allow_arrays: bool) -> Result<(), String> {
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
        let full = join(path, name);
        if !allow_arrays && name == ITEM_ID {
            return Err(format!(
                "`{full}`: `{ITEM_ID}` é reservado — o sistema gera o id de cada item"
            ));
        }
        validate_property_schema(&full, property, allow_arrays)?;
    }
    // `required` e `order` (a ordem dos campos na tela — o JSON não guarda a
    // ordem das chaves, e o `jsonb` do PostgreSQL a reescreve) citam campos.
    for keyword in ["required", "order"] {
        let Some(list) = root.get(keyword) else {
            continue;
        };
        let list = list
            .as_array()
            .ok_or_else(|| format!("\"{keyword}\" precisa ser uma lista"))?;
        for item in list {
            let name = item
                .as_str()
                .ok_or_else(|| format!("\"{keyword}\" só aceita nomes"))?;
            if !properties.contains_key(name) {
                return Err(format!(
                    "`{name}` está em \"{keyword}\" mas não em \"properties\""
                ));
            }
        }
    }
    Ok(())
}

fn validate_property_schema(
    name: &str,
    property: &Value,
    allow_arrays: bool,
) -> Result<(), String> {
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
    // Apresentação: `enumTitles` dá o nome de cada opção do `enum` (mesma
    // ordem); `advanced` põe o campo em "Opções avançadas"; `hidden` o tira da
    // tela (o valor ainda vai — ex.: o nome atual de uma rede ao renomear).
    if let Some(titles) = property.get("enumTitles") {
        let count = property.get("enum").and_then(Value::as_array).map(Vec::len);
        let valid = titles.as_array().is_some_and(|list| {
            Some(list.len()) == count && list.iter().all(|title| title.as_str().is_some())
        });
        if !valid {
            return Err(format!(
                "`{name}`: `enumTitles` é uma lista de textos do tamanho do `enum`"
            ));
        }
    }
    for flag in ["advanced", "hidden"] {
        if property.get(flag).is_some_and(|value| !value.is_boolean()) {
            return Err(format!("`{name}`: `{flag}` é verdadeiro ou falso"));
        }
    }
    // `source`: o campo oferece os valores que os equipamentos têm — uma lista
    // da saída da ação de estado e a chave do item (`"networks.ssid"`).
    if let Some(source) = property.get("source") {
        let valid = source
            .as_str()
            .and_then(|path| path.split_once('.'))
            .is_some_and(|(list, key)| !list.is_empty() && !key.is_empty() && !key.contains('.'));
        if !valid || kind != "string" {
            return Err(format!(
                "`{name}`: `source` é texto no formato \"lista.chave\" (ex.: \"networks.ssid\") e só em campo de texto"
            ));
        }
    }
    if kind == "array" {
        if !allow_arrays {
            return Err(format!("`{name}`: lista dentro de lista não é suportada"));
        }
        let items = property
            .get("items")
            .ok_or_else(|| format!("a lista `{name}` precisa de `items`"))?;
        return match items.get("type").and_then(Value::as_str) {
            Some("object") => validate_object_schema(items, name, false),
            Some(scalar) if SCALAR_TYPES.contains(&scalar) => {
                validate_property_schema(&format!("{name}[]"), items, false)
            }
            _ => Err(format!(
                "os itens de `{name}` precisam ser `object` ou um tipo simples"
            )),
        };
    }
    if !SCALAR_TYPES.contains(&kind) {
        return Err(format!(
            "a propriedade `{name}` tem tipo `{kind}`; use string, integer, number, boolean ou array"
        ));
    }
    if property.contains_key("items") {
        return Err(format!("`{name}`: `items` só vale em `array`"));
    }
    if property.get("secret").and_then(Value::as_bool) == Some(true) && kind != "string" {
        return Err(format!("`{name}`: `secret` só vale em texto"));
    }
    if let Some(pattern) = property.get("pattern") {
        let pattern = pattern
            .as_str()
            .ok_or_else(|| format!("o `pattern` de `{name}` precisa ser texto"))?;
        Regex::new(pattern)
            .map_err(|error| format!("o `pattern` de `{name}` é inválido: {error}"))?;
    }
    Ok(())
}

fn join(path: &str, name: &str) -> String {
    if path.is_empty() {
        name.to_owned()
    } else {
        format!("{path}.{name}")
    }
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
    let mut errors = Vec::new();
    let output = validate_object(schema, given, "", false, &mut errors);
    if errors.is_empty() {
        Ok(Value::Object(output))
    } else {
        Err(errors)
    }
}

fn validate_object(
    schema: &Value,
    given: &Map<String, Value>,
    path: &str,
    is_item: bool,
    errors: &mut Vec<String>,
) -> Map<String, Value> {
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

    let mut output = Map::new();
    for key in given.keys() {
        if is_item && key == ITEM_ID {
            continue;
        }
        if !properties.contains_key(key) {
            errors.push(format!("parâmetro desconhecido: `{}`", join(path, key)));
        }
    }
    if is_item {
        if let Some(id) = given.get(ITEM_ID).and_then(Value::as_str) {
            output.insert(ITEM_ID.to_owned(), Value::String(id.to_owned()));
        }
    }
    for (name, property) in &properties {
        let full = join(path, name);
        let value = given
            .get(name)
            .filter(|value| !value.is_null())
            .or_else(|| property.get("default"));
        let Some(value) = value else {
            if required.contains(&name.as_str()) {
                errors.push(format!("`{full}` é obrigatório"));
            }
            continue;
        };
        if let Some(value) = check_value(&full, property, value, errors) {
            output.insert(name.clone(), value);
        }
    }
    output
}

fn check_value(
    name: &str,
    property: &Value,
    value: &Value,
    errors: &mut Vec<String>,
) -> Option<Value> {
    if property.get("type").and_then(Value::as_str) != Some("array") {
        return match check_scalar(name, property, value) {
            Ok(value) => Some(value),
            Err(error) => {
                errors.push(error);
                None
            }
        };
    }
    let Some(list) = value.as_array() else {
        errors.push(format!("`{name}` precisa ser uma lista"));
        return None;
    };
    let count = list.len() as u64;
    if let Some(min) = property.get("minItems").and_then(Value::as_u64) {
        if count < min {
            errors.push(format!("`{name}` precisa de ao menos {min} item(ns)"));
        }
    }
    if let Some(max) = property.get("maxItems").and_then(Value::as_u64) {
        if count > max {
            errors.push(format!("`{name}` aceita no máximo {max} item(ns)"));
        }
    }
    let items = property.get("items").cloned().unwrap_or(Value::Null);
    let objects = items.get("type").and_then(Value::as_str) == Some("object");
    let mut output = Vec::with_capacity(list.len());
    for (index, item) in list.iter().enumerate() {
        let item_name = format!("{name}[{}]", index + 1);
        if objects {
            let Some(map) = item.as_object() else {
                errors.push(format!("`{item_name}` precisa ser um objeto"));
                continue;
            };
            output.push(Value::Object(validate_object(
                &items, map, &item_name, true, errors,
            )));
        } else {
            match check_scalar(&item_name, &items, item) {
                Ok(value) => output.push(value),
                Err(error) => errors.push(error),
            }
        }
    }
    Some(Value::Array(output))
}

fn check_scalar(name: &str, property: &Value, value: &Value) -> Result<Value, String> {
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

/// Onde moram os campos secretos de um esquema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretField {
    /// Propriedade da raiz.
    Root(String),
    /// Campo de cada item de uma lista de objetos.
    Item { list: String, field: String },
}

fn is_secret(property: &Value) -> bool {
    property.get("secret").and_then(Value::as_bool) == Some(true)
}

/// Os campos `secret: true` do esquema.
#[must_use]
pub fn secret_fields(schema: Option<&Value>) -> Vec<SecretField> {
    let Some(properties) = schema
        .and_then(|schema| schema.get("properties"))
        .and_then(Value::as_object)
    else {
        return Vec::new();
    };
    let mut fields = Vec::new();
    for (name, property) in properties {
        if is_secret(property) {
            fields.push(SecretField::Root(name.clone()));
        }
        if let Some(items) = property
            .get("items")
            .and_then(|items| items.get("properties"))
            .and_then(Value::as_object)
        {
            for (field, item_property) in items {
                if is_secret(item_property) {
                    fields.push(SecretField::Item {
                        list: name.clone(),
                        field: field.clone(),
                    });
                }
            }
        }
    }
    fields
}

/// Aplica `f` a cada valor de campo secreto (raiz e itens de lista).
pub fn for_each_secret(value: &mut Value, fields: &[SecretField], f: &mut dyn FnMut(&mut Value)) {
    for field in fields {
        match field {
            SecretField::Root(name) => {
                if let Some(slot) = value.get_mut(name) {
                    f(slot);
                }
            }
            SecretField::Item { list, field } => {
                if let Some(items) = value.get_mut(list).and_then(Value::as_array_mut) {
                    for item in items {
                        if let Some(slot) = item.get_mut(field) {
                            f(slot);
                        }
                    }
                }
            }
        }
    }
}

/// Os valores em claro dos campos secretos — a lista que o runtime mascara no
/// transcript e na saída. Textos curtos demais ficam de fora (mascarar "ab"
/// apagaria pedaços de qualquer saída).
#[must_use]
pub fn secret_values(schema: Option<&Value>, value: &Value) -> Vec<String> {
    let mut copy = value.clone();
    let mut found = Vec::new();
    for_each_secret(&mut copy, &secret_fields(schema), &mut |slot| {
        if let Some(text) = slot.as_str().filter(|text| text.len() >= 4) {
            found.push(text.to_owned());
        }
    });
    found
}

/// Cópia com cada segredo preenchido trocado por [`SECRET_MASK`] — o que vai
/// para o banco (parâmetros de execução e de lote).
#[must_use]
pub fn mask_secrets(schema: Option<&Value>, value: &Value) -> Value {
    let mut masked = value.clone();
    for_each_secret(&mut masked, &secret_fields(schema), &mut |slot| {
        if slot.as_str().is_some_and(|text| !text.is_empty()) {
            *slot = Value::String(SECRET_MASK.into());
        }
    });
    masked
}

/// Listas de objetos do esquema (os itens que ganham `id`).
#[must_use]
pub fn object_lists(schema: Option<&Value>) -> Vec<String> {
    schema
        .and_then(|schema| schema.get("properties"))
        .and_then(Value::as_object)
        .map(|properties| {
            properties
                .iter()
                .filter(|(_, property)| {
                    property
                        .get("items")
                        .and_then(|items| items.get("type"))
                        .and_then(Value::as_str)
                        == Some("object")
                })
                .map(|(name, _)| name.clone())
                .collect()
        })
        .unwrap_or_default()
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

    fn wifi() -> Value {
        json!({
            "type": "object",
            "properties": {
                "country": { "type": "string", "pattern": "[A-Z]{2}", "default": "BR" },
                "mesh_key": { "type": "string", "secret": true },
                "networks": {
                    "type": "array", "maxItems": 2,
                    "items": {
                        "type": "object",
                        "properties": {
                            "ssid": { "type": "string", "minLength": 1 },
                            "key": { "type": "string", "secret": true },
                            "band_5g": { "type": "boolean", "default": true }
                        },
                        "required": ["ssid"]
                    }
                },
                "skip": { "type": "array", "items": { "type": "string", "maxLength": 32 } }
            }
        })
    }

    #[test]
    fn esquema_suportado_passa_e_desconhecido_e_recusado() {
        assert!(validate_schema(&schema()).is_ok());
        assert!(validate_schema(&wifi()).is_ok());
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
    fn titulos_das_opcoes_e_campos_avancados() {
        let ok = json!({ "type": "object", "properties": {
            "enc": { "type": "string", "enum": ["psk2", "none"],
                     "enumTitles": ["WPA2", "Aberta"], "advanced": true },
            "old": { "type": "string", "hidden": true } } });
        assert!(validate_schema(&ok).is_ok());
        let curto = json!({ "type": "object", "properties": {
            "enc": { "type": "string", "enum": ["psk2", "none"], "enumTitles": ["WPA2"] } } });
        assert!(validate_schema(&curto).is_err());
        let sem_enum = json!({ "type": "object", "properties": {
            "enc": { "type": "string", "enumTitles": ["WPA2"] } } });
        assert!(validate_schema(&sem_enum).is_err());
        let flag = json!({ "type": "object", "properties": {
            "a": { "type": "string", "advanced": "sim" } } });
        assert!(validate_schema(&flag).is_err());
    }

    #[test]
    fn origem_dos_valores_e_lista_ponto_chave() {
        let ok = json!({ "type": "object", "properties": {
            "ssid": { "type": "string", "source": "networks.ssid" } } });
        assert!(validate_schema(&ok).is_ok());
        for errado in ["networks", "a.b.c", ".ssid"] {
            let schema = json!({ "type": "object", "properties": {
                "ssid": { "type": "string", "source": errado } } });
            assert!(validate_schema(&schema).is_err(), "{errado}");
        }
        let numero = json!({ "type": "object", "properties": {
            "n": { "type": "integer", "source": "a.b" } } });
        assert!(validate_schema(&numero).is_err());
    }

    #[test]
    fn segredos_saem_mascarados_e_listados() {
        let value = json!({ "mesh_key": "mesh-secreta", "country": "BR",
            "networks": [ { "ssid": "Loja", "key": "senha-forte" }, { "ssid": "Aberta", "key": "" } ] });
        assert_eq!(
            secret_values(Some(&wifi()), &value),
            vec!["mesh-secreta".to_owned(), "senha-forte".to_owned()]
        );
        let masked = mask_secrets(Some(&wifi()), &value);
        assert_eq!(masked["mesh_key"], SECRET_MASK);
        assert_eq!(masked["networks"][0]["key"], SECRET_MASK);
        assert_eq!(masked["networks"][1]["key"], "");
        assert_eq!(masked["country"], "BR");
        assert_eq!(mask_secrets(None, &value), value);
    }

    #[test]
    fn ordem_dos_campos_so_cita_campos_do_esquema() {
        let ordenado = json!({ "type": "object", "order": ["b", "a"],
            "properties": { "a": { "type": "string" }, "b": { "type": "string" } } });
        assert!(validate_schema(&ordenado).is_ok());
        let fantasma = json!({ "type": "object", "order": ["c"],
            "properties": { "a": { "type": "string" } } });
        assert!(validate_schema(&fantasma)
            .unwrap_err()
            .contains("\"order\""));
        let item = json!({ "type": "object", "properties": { "l": { "type": "array",
            "items": { "type": "object", "order": [1],
                "properties": { "a": { "type": "string" } } } } } });
        assert!(validate_schema(&item).is_err());
    }

    #[test]
    fn lista_dentro_de_lista_id_reservado_e_secret_fora_de_texto_sao_recusados() {
        let aninhada = json!({ "type": "object", "properties": { "a": { "type": "array",
            "items": { "type": "object", "properties": { "b": { "type": "array",
                "items": { "type": "string" } } } } } } });
        assert!(validate_schema(&aninhada)
            .unwrap_err()
            .contains("lista dentro de lista"));
        let com_id = json!({ "type": "object", "properties": { "a": { "type": "array",
            "items": { "type": "object", "properties": { "id": { "type": "string" } } } } } });
        assert!(validate_schema(&com_id).unwrap_err().contains("reservado"));
        let segredo = json!({ "type": "object", "properties": {
            "n": { "type": "integer", "secret": true } } });
        assert!(validate_schema(&segredo).is_err());
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
    fn listas_validam_cada_item_com_caminho_legivel() {
        let ok = validate(
            Some(&wifi()),
            &json!({ "networks": [ { "id": "a1", "ssid": "Loja" } ], "skip": ["Visitantes"] }),
        )
        .unwrap();
        assert_eq!(
            ok["networks"][0],
            json!({ "id": "a1", "ssid": "Loja", "band_5g": true })
        );
        assert_eq!(ok["country"], "BR");

        let errors = validate(
            Some(&wifi()),
            &json!({ "networks": [ { "ssid": "" }, { "ssid": "B", "x": 1 }, { "ssid": "C" } ] }),
        )
        .unwrap_err();
        assert!(
            errors.iter().any(|e| e.contains("networks[1].ssid")),
            "{errors:?}"
        );
        assert!(
            errors.iter().any(|e| e.contains("networks[2].x")),
            "{errors:?}"
        );
        assert!(
            errors.iter().any(|e| e.contains("no máximo 2")),
            "{errors:?}"
        );
    }

    #[test]
    fn campos_secretos_e_listas_de_objetos_sao_encontrados() {
        assert_eq!(
            secret_fields(Some(&wifi())),
            vec![
                SecretField::Root("mesh_key".into()),
                SecretField::Item {
                    list: "networks".into(),
                    field: "key".into()
                },
            ]
        );
        assert_eq!(object_lists(Some(&wifi())), vec!["networks".to_string()]);
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
