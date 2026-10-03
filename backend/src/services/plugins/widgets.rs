//! Componentes de campo do formulário de um plugin e a regra que mostra um
//! campo conforme outro.
//!
//! O plugin **escolhe** o componente (`"widget": "ip"`); quem o desenha é a
//! interface do sistema. É um catálogo fechado de propósito: a IA que escreve
//! um plugin não inventa tela, só diz o que o campo é — e cada componente traz
//! a própria validação, então ninguém precisa escrever (e errar) um `pattern`
//! de IP à mão.
//!
//! * `ip`, `cidr`, `mac`, `hostname`, `url` e `textarea` em texto;
//! * `port` em inteiro (1–65535);
//! * `slider` em número ou inteiro, com `minimum` e `maximum`;
//! * `tags` em lista de textos.
//!
//! `visibleWhen` (`{ "field": "encryption", "notIn": ["none"] }`) esconde o
//! campo quando a regra não vale: ele some da tela, deixa de ser obrigatório e
//! o valor não vai para o script — a senha de uma rede aberta não existe.

use std::net::IpAddr;

use serde_json::{Map, Value};

/// Um componente de campo do catálogo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Widget {
    Ip,
    Cidr,
    Mac,
    Port,
    Hostname,
    Url,
    Textarea,
    Slider,
    Tags,
}

impl Widget {
    pub const ALL: [Self; 9] = [
        Self::Ip,
        Self::Cidr,
        Self::Mac,
        Self::Port,
        Self::Hostname,
        Self::Url,
        Self::Textarea,
        Self::Slider,
        Self::Tags,
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ip => "ip",
            Self::Cidr => "cidr",
            Self::Mac => "mac",
            Self::Port => "port",
            Self::Hostname => "hostname",
            Self::Url => "url",
            Self::Textarea => "textarea",
            Self::Slider => "slider",
            Self::Tags => "tags",
        }
    }

    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|widget| widget.name() == name)
    }

    /// O componente do campo, se o esquema declara um.
    #[must_use]
    pub fn of(property: &Value) -> Option<Self> {
        property
            .get("widget")
            .and_then(Value::as_str)
            .and_then(Self::parse)
    }

    /// O tipo de campo que o componente desenha.
    const fn expected_type(self) -> &'static str {
        match self {
            Self::Port => "integer",
            Self::Slider => "number ou integer",
            Self::Tags => "array de string",
            _ => "string",
        }
    }

    fn fits(self, kind: &str, item_kind: Option<&str>) -> bool {
        match self {
            Self::Port => kind == "integer",
            Self::Slider => kind == "integer" || kind == "number",
            Self::Tags => kind == "array" && item_kind == Some("string"),
            _ => kind == "string",
        }
    }

    /// O valor já sai conferido pelo componente — conta como `pattern` para a
    /// revisão de injeção de comando.
    #[must_use]
    pub const fn constrains(self) -> bool {
        matches!(
            self,
            Self::Ip | Self::Cidr | Self::Mac | Self::Port | Self::Hostname | Self::Url
        )
    }

    /// Confere o valor de um campo deste componente. `Err` é a frase que vai
    /// para a tela, já sem o nome do campo.
    ///
    /// # Errors
    ///
    /// O valor não é o que o componente promete (IP, MAC, porta…).
    pub fn check(self, value: &Value) -> Result<(), &'static str> {
        match (self, value) {
            (Self::Ip, Value::String(text)) => text
                .parse::<IpAddr>()
                .map(|_| ())
                .map_err(|_| "precisa ser um endereço IP (ex.: 192.168.1.1)"),
            (Self::Cidr, Value::String(text)) => is_cidr(text)
                .then_some(())
                .ok_or("precisa ser uma rede no formato 192.168.1.0/24"),
            (Self::Mac, Value::String(text)) => is_mac(text)
                .then_some(())
                .ok_or("precisa ser um MAC (ex.: AA:BB:CC:DD:EE:FF)"),
            (Self::Hostname, Value::String(text)) => is_hostname(text)
                .then_some(())
                .ok_or("precisa ser um nome de host (letras, números, hífen e ponto)"),
            (Self::Url, Value::String(text)) => is_url(text)
                .then_some(())
                .ok_or("precisa ser um endereço http:// ou https://"),
            (Self::Port, Value::Number(number)) => number
                .as_u64()
                .filter(|port| (1..=65_535).contains(port))
                .map(|_| ())
                .ok_or("precisa ser uma porta de 1 a 65535"),
            _ => Ok(()),
        }
    }
}

fn is_cidr(text: &str) -> bool {
    let Some((address, prefix)) = text.split_once('/') else {
        return false;
    };
    let Ok(prefix) = prefix.parse::<u8>() else {
        return false;
    };
    match address.parse::<IpAddr>() {
        Ok(IpAddr::V4(_)) => prefix <= 32,
        Ok(IpAddr::V6(_)) => prefix <= 128,
        Err(_) => false,
    }
}

fn is_mac(text: &str) -> bool {
    let separator = if text.contains('-') { '-' } else { ':' };
    let parts: Vec<&str> = text.split(separator).collect();
    parts.len() == 6
        && parts
            .iter()
            .all(|part| part.len() == 2 && part.bytes().all(|b| b.is_ascii_hexdigit()))
}

fn is_hostname(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 253
        && text.split('.').all(|label| {
            let bytes = label.as_bytes();
            (1..=63).contains(&bytes.len())
                && bytes.first() != Some(&b'-')
                && bytes.last() != Some(&b'-')
                && bytes
                    .iter()
                    .all(|b| b.is_ascii_alphanumeric() || *b == b'-')
        })
}

fn is_url(text: &str) -> bool {
    reqwest::Url::parse(text)
        .is_ok_and(|url| matches!(url.scheme(), "http" | "https") && url.host().is_some())
}

/// Confere `widget` e `group` de uma propriedade do esquema.
///
/// # Errors
///
/// Mensagem em português apontando o campo.
pub fn validate_presentation(name: &str, property: &Map<String, Value>) -> Result<(), String> {
    if let Some(group) = property.get("group") {
        let valid = group
            .as_str()
            .is_some_and(|text| !text.trim().is_empty() && text.chars().count() <= 40);
        if !valid {
            return Err(format!("`{name}`: `group` é um texto de 1 a 40 caracteres"));
        }
    }
    let Some(widget) = property.get("widget") else {
        return Ok(());
    };
    let names: Vec<&str> = Widget::ALL.iter().map(|widget| widget.name()).collect();
    let widget = widget.as_str().and_then(Widget::parse).ok_or_else(|| {
        format!(
            "`{name}`: `widget` precisa ser um destes: {}",
            names.join(", ")
        )
    })?;
    let kind = property.get("type").and_then(Value::as_str).unwrap_or("");
    let item_kind = property
        .get("items")
        .and_then(|items| items.get("type"))
        .and_then(Value::as_str);
    if !widget.fits(kind, item_kind) {
        return Err(format!(
            "`{name}`: o componente `{}` é para campo {}",
            widget.name(),
            widget.expected_type()
        ));
    }
    if widget == Widget::Slider
        && !(property.get("minimum").is_some_and(Value::is_number)
            && property.get("maximum").is_some_and(Value::is_number))
    {
        return Err(format!(
            "`{name}`: o componente `slider` precisa de `minimum` e `maximum`"
        ));
    }
    Ok(())
}

/// A regra `visibleWhen` de um campo.
#[derive(Debug, Clone, PartialEq)]
pub struct VisibleWhen {
    pub field: String,
    rule: Rule,
}

#[derive(Debug, Clone, PartialEq)]
enum Rule {
    Equals(Value),
    In(Vec<Value>),
    NotIn(Vec<Value>),
}

impl VisibleWhen {
    /// Lê a regra do campo, se ele tem uma.
    ///
    /// # Errors
    ///
    /// A regra está fora do formato `{ field, equals | in | notIn }`.
    pub fn of(name: &str, property: &Value) -> Result<Option<Self>, String> {
        let Some(raw) = property.get("visibleWhen") else {
            return Ok(None);
        };
        let invalid = || {
            format!(
                "`{name}`: `visibleWhen` é {{ \"field\": \"outro_campo\", e um de \"equals\", \"in\" ou \"notIn\" }}"
            )
        };
        let object = raw.as_object().ok_or_else(invalid)?;
        let field = object
            .get("field")
            .and_then(Value::as_str)
            .filter(|field| !field.is_empty())
            .ok_or_else(invalid)?;
        let list = |key: &str| object.get(key).and_then(Value::as_array).cloned();
        let rules = [
            object.get("equals").cloned().map(Rule::Equals),
            list("in").map(Rule::In),
            list("notIn").map(Rule::NotIn),
        ];
        let mut found = rules.into_iter().flatten();
        let rule = found.next().ok_or_else(invalid)?;
        if found.next().is_some() || object.len() != 2 {
            return Err(invalid());
        }
        Ok(Some(Self {
            field: field.to_owned(),
            rule,
        }))
    }

    /// A regra vale para estes valores (os informados, com os padrões)?
    #[must_use]
    pub fn holds(&self, values: &Map<String, Value>) -> bool {
        let value = values.get(&self.field).unwrap_or(&Value::Null);
        match &self.rule {
            Rule::Equals(expected) => value == expected,
            Rule::In(options) => options.contains(value),
            Rule::NotIn(options) => !options.contains(value),
        }
    }
}

/// Confere a `visibleWhen` de cada propriedade contra as irmãs: o campo citado
/// existe, é simples e não é o próprio.
///
/// # Errors
///
/// Mensagem em português apontando o campo.
pub fn validate_visibility(path: &str, properties: &Map<String, Value>) -> Result<(), String> {
    for (name, property) in properties {
        let full = if path.is_empty() {
            name.clone()
        } else {
            format!("{path}.{name}")
        };
        let Some(rule) = VisibleWhen::of(&full, property)? else {
            continue;
        };
        let target = properties.get(&rule.field).filter(|_| rule.field != *name);
        let simple = target
            .and_then(|target| target.get("type"))
            .and_then(Value::as_str)
            .is_some_and(|kind| kind != "array");
        if !simple {
            return Err(format!(
                "`{full}`: `visibleWhen` cita `{}`, que precisa ser outro campo simples do mesmo formulário",
                rule.field
            ));
        }
    }
    Ok(())
}

/// Os valores do formulário com os padrões do esquema — é sobre eles que a
/// `visibleWhen` decide.
#[must_use]
pub fn effective_values(
    properties: &Map<String, Value>,
    given: &Map<String, Value>,
) -> Map<String, Value> {
    properties
        .iter()
        .filter_map(|(name, property)| {
            given
                .get(name)
                .filter(|value| !value.is_null())
                .or_else(|| property.get("default"))
                .map(|value| (name.clone(), value.clone()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn prop(value: Value) -> Map<String, Value> {
        value.as_object().unwrap().clone()
    }

    #[test]
    fn cada_componente_confere_o_proprio_valor() {
        assert!(Widget::Ip.check(&json!("192.168.1.1")).is_ok());
        assert!(Widget::Ip.check(&json!("fe80::1")).is_ok());
        assert!(Widget::Ip.check(&json!("192.168.1.300")).is_err());
        assert!(Widget::Cidr.check(&json!("10.0.0.0/8")).is_ok());
        assert!(Widget::Cidr.check(&json!("10.0.0.0/33")).is_err());
        assert!(Widget::Cidr.check(&json!("10.0.0.0")).is_err());
        assert!(Widget::Mac.check(&json!("aa:bb:cc:dd:ee:ff")).is_ok());
        assert!(Widget::Mac.check(&json!("AA-BB-CC-DD-EE-FF")).is_ok());
        assert!(Widget::Mac.check(&json!("aa:bb:cc:dd:ee")).is_err());
        assert!(Widget::Hostname.check(&json!("ap-sala.lan")).is_ok());
        assert!(Widget::Hostname.check(&json!("-ruim")).is_err());
        assert!(Widget::Hostname.check(&json!("a b")).is_err());
        assert!(Widget::Url.check(&json!("https://exemplo.com/x")).is_ok());
        assert!(Widget::Url.check(&json!("ftp://exemplo.com")).is_err());
        assert!(Widget::Port.check(&json!(443)).is_ok());
        assert!(Widget::Port.check(&json!(0)).is_err());
        assert!(Widget::Port.check(&json!(70_000)).is_err());
        assert!(Widget::Textarea.check(&json!("qualquer\ntexto")).is_ok());
    }

    #[test]
    fn o_componente_precisa_caber_no_tipo_do_campo() {
        let ok = prop(json!({ "type": "string", "widget": "ip" }));
        assert!(validate_presentation("gw", &ok).is_ok());
        let porta_texto = prop(json!({ "type": "string", "widget": "port" }));
        let erro = validate_presentation("porta", &porta_texto).unwrap_err();
        assert!(erro.contains("integer"), "{erro}");
        let tags =
            prop(json!({ "type": "array", "items": { "type": "string" }, "widget": "tags" }));
        assert!(validate_presentation("dns", &tags).is_ok());
        let desconhecido = prop(json!({ "type": "string", "widget": "mapa" }));
        assert!(validate_presentation("x", &desconhecido)
            .unwrap_err()
            .contains("ip, cidr"));
        let slider = prop(json!({ "type": "integer", "widget": "slider" }));
        assert!(validate_presentation("potência", &slider)
            .unwrap_err()
            .contains("minimum"));
        let grupo = prop(json!({ "type": "string", "group": "" }));
        assert!(validate_presentation("x", &grupo).is_err());
    }

    #[test]
    fn a_visibilidade_cita_um_campo_irmao_simples() {
        let campos = prop(json!({
            "encryption": { "type": "string", "enum": ["none", "psk2"] },
            "key": { "type": "string", "visibleWhen": { "field": "encryption", "notIn": ["none"] } }
        }));
        assert!(validate_visibility("", &campos).is_ok());
        let fantasma = prop(json!({
            "key": { "type": "string", "visibleWhen": { "field": "nada", "equals": 1 } }
        }));
        assert!(validate_visibility("", &fantasma)
            .unwrap_err()
            .contains("nada"));
        let duas_regras = prop(json!({
            "a": { "type": "boolean" },
            "b": { "type": "string", "visibleWhen": { "field": "a", "equals": true, "in": [true] } }
        }));
        assert!(validate_visibility("", &duas_regras).is_err());
    }

    #[test]
    fn a_regra_decide_sobre_os_valores_com_os_padroes() {
        let regra = VisibleWhen::of(
            "key",
            &json!({ "visibleWhen": { "field": "encryption", "notIn": ["none"] } }),
        )
        .unwrap()
        .unwrap();
        let propriedades = prop(json!({
            "encryption": { "type": "string", "default": "psk2" },
            "key": { "type": "string" }
        }));
        let padrao = effective_values(&propriedades, &Map::new());
        assert!(regra.holds(&padrao));
        let aberta = effective_values(&propriedades, &prop(json!({ "encryption": "none" })));
        assert!(!regra.holds(&aberta));
    }
}
