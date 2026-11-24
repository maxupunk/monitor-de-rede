//! Leitura tolerante dos argumentos que a IA envia.
//!
//! Modelos pequenos mandam `"5"` onde o schema pede `5`, espaços sobrando e
//! chaves vazias. Aceitar essas variações aqui evita que cada ferramenta
//! repita a mesma limpeza.

use serde_json::{json, Value};

use crate::{
    audit::AuditActor,
    shared::errors::{AppError, AppResult},
};

/// Contexto de execução quando a chamada de uma ferramenta veio do servidor MCP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpToolContext {
    pub token_id: i64,
    pub token_name: String,
    /// Aplicações de deploy liberadas para este token (`None` = todas que o
    /// usuário enxerga; `Some(ids)` = estritamente restrito a estes IDs).
    pub allowed_application_ids: Option<Vec<i64>>,
    /// Quando `true`, deploys manuais respeitam `target.requires_approval`.
    pub respect_target_approval: bool,
    /// Verdadeiro apenas quando o token possui o escopo `deploys:approve`.
    pub can_approve_deploys: bool,
}

impl McpToolContext {
    /// O token tem acesso à aplicação de deploy indicada?
    #[must_use]
    pub fn allows_application(&self, app_id: i64) -> bool {
        self.allowed_application_ids
            .as_ref()
            .is_none_or(|ids| ids.contains(&app_id))
    }

    /// Garante que o token tem acesso à aplicação de deploy indicada.
    ///
    /// # Errors
    ///
    /// `Forbidden` quando `allowed_application_ids` está definido e não contém `app_id`.
    pub fn require_application(&self, app_id: i64) -> AppResult<()> {
        if self.allows_application(app_id) {
            Ok(())
        } else {
            Err(AppError::forbidden(format!(
                "Este token MCP não tem acesso à aplicação #{app_id}"
            )))
        }
    }

    /// Garante que o token possui escopo para aprovar ou recusar deploys.
    ///
    /// # Errors
    ///
    /// `Forbidden` quando `can_approve_deploys` é falso.
    pub fn require_deploy_approval_scope(&self) -> AppResult<()> {
        if self.can_approve_deploys {
            Ok(())
        } else {
            Err(AppError::forbidden(
                "Este token MCP não tem permissão para aprovar ou recusar deploys",
            ))
        }
    }

    /// Deploys manuais podem pular `target.requires_approval`?
    #[must_use]
    pub const fn auto_approve_manual_deploys(&self) -> bool {
        !self.respect_target_approval
    }
}

/// Argumentos de uma chamada e quem a fez.
#[derive(Debug, Clone)]
pub struct ToolArgs {
    values: Value,
    actor: AuditActor,
    /// Conversa de onde veio a chamada — o modo "Aceitar automaticamente" dos
    /// plugins vale por conversa e equipamento.
    conversation: Option<String>,
    /// Contexto do servidor MCP quando a chamada veio de um cliente externo.
    mcp: Option<McpToolContext>,
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
            conversation: None,
            mcp: None,
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

    #[must_use]
    pub fn with_conversation(mut self, conversation: Option<String>) -> Self {
        self.conversation = conversation.filter(|key| !key.trim().is_empty());
        self
    }

    #[must_use]
    pub fn conversation(&self) -> Option<&str> {
        self.conversation.as_deref()
    }

    /// Anexa o contexto do token MCP que originou a chamada.
    #[must_use]
    pub fn with_mcp(mut self, mcp: McpToolContext) -> Self {
        self.mcp = Some(mcp);
        self
    }

    #[must_use]
    pub const fn mcp(&self) -> Option<&McpToolContext> {
        self.mcp.as_ref()
    }

    /// Os argumentos como vieram, para quem precisa do objeto inteiro.
    #[must_use]
    pub const fn values(&self) -> &Value {
        &self.values
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

    #[test]
    fn contexto_mcp_restringe_aplicacoes_e_aprovacao() {
        let sem_restricao = McpToolContext {
            token_id: 1,
            token_name: "ide".into(),
            allowed_application_ids: None,
            respect_target_approval: true,
            can_approve_deploys: false,
        };
        assert!(sem_restricao.allows_application(10));
        assert!(sem_restricao.require_application(10).is_ok());
        assert!(!sem_restricao.auto_approve_manual_deploys());
        assert!(matches!(
            sem_restricao.require_deploy_approval_scope(),
            Err(AppError::Forbidden(_))
        ));

        let restrito = McpToolContext {
            token_id: 2,
            token_name: "projeto-sgc".into(),
            allowed_application_ids: Some(vec![3, 7]),
            respect_target_approval: true,
            can_approve_deploys: true,
        };
        assert!(restrito.allows_application(3));
        assert!(!restrito.allows_application(4));
        assert!(matches!(
            restrito.require_application(4),
            Err(AppError::Forbidden(_))
        ));
        assert!(restrito.require_deploy_approval_scope().is_ok());

        let args = ToolArgs::from_value(json!({})).with_mcp(restrito.clone());
        assert_eq!(args.mcp(), Some(&restrito));
    }
}
