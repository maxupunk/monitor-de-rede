//! Leitura tolerante dos argumentos que a IA envia.
//!
//! Modelos pequenos mandam `"5"` onde o schema pede `5`, espaços sobrando e
//! chaves vazias. Aceitar essas variações aqui evita que cada ferramenta
//! repita a mesma limpeza.

use serde_json::{json, Value};

use crate::services::{
    audit::AuditActor,
    shared::errors::{AppError, AppResult},
};

/// Argumentos de uma chamada e quem a fez.
#[derive(Debug, Clone)]
pub struct ToolArgs {
    values: Value,
    actor: AuditActor,
}

impl ToolArgs {
    /// JSON inválido vira objeto vazio: a ferramenta decide o que é obrigatório.
    #[must_use]
    pub fn parse(raw: &str) -> Self {
        Self::from_value(serde_json::from_str(raw).unwrap_or_else(|_| json!({})))
    }

    /// Argumentos já em JSON; qualquer coisa que não seja objeto vira vazio.
    #[must_use]
    pub fn from_value(values: Value) -> Self {
        let values = if values.is_object() {
            values
        } else {
            json!({})
        };
        Self {
            values,
            actor: AuditActor::default(),
        }
    }

    /// Registra o usuário que confirmou a chamada (auditoria, autoria).
    #[must_use]
    pub fn with_actor(mut self, actor: AuditActor) -> Self {
        self.actor = actor;
        self
    }

    #[must_use]
    pub const fn actor(&self) -> &AuditActor {
        &self.actor
    }

    /// Texto não vazio, já sem espaços nas pontas. Números também valem,
    /// porque um id de dispositivo costuma chegar como `12`.
    #[must_use]
    pub fn text(&self, key: &str) -> Option<String> {
        match self.values.get(key)? {
            Value::String(text) => {
                let trimmed = text.trim();
                (!trimmed.is_empty()).then(|| trimmed.to_string())
            }
            Value::Number(number) => Some(number.to_string()),
            _ => None,
        }
    }

    /// O valor como veio, para argumentos que aceitam número ou texto.
    #[must_use]
    pub fn raw(&self, key: &str) -> Option<&Value> {
        self.values.get(key).filter(|value| !value.is_null())
    }

    /// # Errors
    ///
    /// Validação quando o argumento falta ou está vazio.
    pub fn required_text(&self, key: &str, message: &str) -> AppResult<String> {
        self.text(key).ok_or_else(|| AppError::validation(message))
    }

    /// Inteiro não negativo, aceitando número ou texto numérico.
    #[must_use]
    pub fn integer(&self, key: &str) -> Option<i64> {
        match self.values.get(key)? {
            Value::Number(number) => number
                .as_i64()
                .or_else(|| number.as_f64().map(|value| value as i64)),
            Value::String(text) => text.trim().parse().ok(),
            _ => None,
        }
        .filter(|value| *value >= 0)
    }

    /// Inteiro limitado a `[min, max]`, com `default` quando ausente.
    #[must_use]
    pub fn integer_in(&self, key: &str, default: i64, min: i64, max: i64) -> i64 {
        self.integer(key).unwrap_or(default).clamp(min, max)
    }

    #[must_use]
    pub fn flag(&self, key: &str) -> bool {
        match self.values.get(key) {
            Some(Value::Bool(value)) => *value,
            Some(Value::String(text)) => text.trim().eq_ignore_ascii_case("true"),
            _ => false,
        }
    }

    /// Lista de textos não vazios. Um texto solto vale como lista de um.
    #[must_use]
    pub fn texts(&self, key: &str) -> Vec<String> {
        let items = match self.values.get(key) {
            Some(Value::Array(items)) => items.as_slice(),
            Some(single) => std::slice::from_ref(single),
            None => &[],
        };
        items
            .iter()
            .filter_map(|item| match item {
                Value::String(text) => Some(text.trim().to_string()),
                Value::Number(number) => Some(number.to_string()),
                _ => None,
            })
            .filter(|text| !text.is_empty())
            .collect()
    }

    /// Lista de portas; entradas fora de `1..=65535` são descartadas.
    #[must_use]
    pub fn ports(&self, key: &str) -> Option<Vec<u16>> {
        let list = self.values.get(key)?.as_array()?;
        let ports: Vec<u16> = list
            .iter()
            .filter_map(|item| match item {
                Value::Number(number) => number.as_u64(),
                Value::String(text) => text.trim().parse().ok(),
                _ => None,
            })
            .filter_map(|port| u16::try_from(port).ok())
            .filter(|port| *port > 0)
            .collect();
        (!ports.is_empty()).then_some(ports)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_invalido_vira_objeto_vazio() {
        let args = ToolArgs::parse("não é json");
        assert_eq!(args.text("target"), None);
        assert!(!args.flag("x"));
    }

    #[test]
    fn lista_de_textos_aceita_texto_solto_e_descarta_vazios() {
        let args = ToolArgs::parse(r#"{"a": [" Borda ", "", 7, null], "b": "MPPT"}"#);
        assert_eq!(args.texts("a"), vec!["Borda", "7"]);
        assert_eq!(args.texts("b"), vec!["MPPT"]);
        assert!(args.texts("c").is_empty());
    }

    #[test]
    fn texto_aceita_numero_e_descarta_vazio() {
        let args = ToolArgs::parse(r#"{"a": "  roteador ", "b": 12, "c": "   "}"#);
        assert_eq!(args.text("a").as_deref(), Some("roteador"));
        assert_eq!(args.text("b").as_deref(), Some("12"));
        assert_eq!(args.text("c"), None);
        assert!(args.required_text("c", "obrigatório").is_err());
    }

    #[test]
    fn inteiro_aceita_texto_e_respeita_limites() {
        let args = ToolArgs::parse(r#"{"n": "48", "m": 999, "neg": -3}"#);
        assert_eq!(args.integer("n"), Some(48));
        assert_eq!(args.integer("neg"), None);
        assert_eq!(args.integer_in("m", 24, 1, 168), 168);
        assert_eq!(args.integer_in("ausente", 24, 1, 168), 24);
    }

    #[test]
    fn portas_invalidas_sao_descartadas() {
        let args = ToolArgs::parse(r#"{"p": [22, "443", 0, 70000, "x"], "vazio": []}"#);
        assert_eq!(args.ports("p"), Some(vec![22, 443]));
        assert_eq!(args.ports("vazio"), None);
    }

    #[test]
    fn argumento_que_nao_e_objeto_vira_vazio() {
        let args = ToolArgs::from_value(json!([1, 2]));
        assert_eq!(args.text("0"), None);
        assert_eq!(args.actor().user_id, None);
    }

    #[test]
    fn flag_aceita_texto() {
        let args = ToolArgs::parse(r#"{"a": true, "b": "TRUE", "c": "no"}"#);
        assert!(args.flag("a"));
        assert!(args.flag("b"));
        assert!(!args.flag("c"));
    }
}
