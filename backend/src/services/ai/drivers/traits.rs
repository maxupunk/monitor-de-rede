use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

use crate::{
    dtos::ai::TestConnectionResponse,
    services::{ai::context_window::sources::LearnedWindow, shared::errors::AppResult},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

impl Serialize for AiToolCall {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        struct FunctionPayload<'a> {
            name: &'a str,
            arguments: &'a str,
        }

        #[derive(Serialize)]
        struct ToolCallPayload<'a> {
            id: &'a str,
            r#type: &'static str,
            function: FunctionPayload<'a>,
        }

        ToolCallPayload {
            id: &self.id,
            r#type: "function",
            function: FunctionPayload {
                name: &self.name,
                arguments: &self.arguments,
            },
        }
        .serialize(serializer)
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum AiToolCallHelper {
    Standard {
        id: String,
        #[serde(default, rename = "type")]
        _type: Option<String>,
        function: AiToolCallFunctionHelper,
    },
    Flat {
        id: String,
        name: String,
        arguments: String,
    },
}

#[derive(Deserialize)]
struct AiToolCallFunctionHelper {
    name: String,
    arguments: String,
}

impl<'de> Deserialize<'de> for AiToolCall {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let helper = AiToolCallHelper::deserialize(deserializer)?;
        match helper {
            AiToolCallHelper::Standard { id, function, .. } => Ok(Self {
                id,
                name: function.name,
                arguments: function.arguments,
            }),
            AiToolCallHelper::Flat {
                id,
                name,
                arguments,
            } => Ok(Self {
                id,
                name,
                arguments,
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiMessage {
    pub role: String,
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<AiToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiToolFunction {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiTool {
    pub r#type: String,
    pub function: AiToolFunction,
}

/// Tokens consumidos numa chamada ao provedor.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AiUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    /// Parte de `prompt_tokens` lida do cache de prefixo do provedor.
    pub cached_tokens: u64,
}

impl AiUsage {
    /// Lê o objeto `usage` do protocolo OpenAI; ausente ou vazio vira `None`.
    #[must_use]
    pub fn from_openai(value: &serde_json::Value) -> Option<Self> {
        let field = |name: &str| value.get(name).and_then(serde_json::Value::as_u64);
        // OpenAI e OpenRouter: `prompt_tokens_details.cached_tokens`;
        // DeepSeek: `prompt_cache_hit_tokens`.
        let cached = value
            .pointer("/prompt_tokens_details/cached_tokens")
            .and_then(serde_json::Value::as_u64)
            .or_else(|| field("prompt_cache_hit_tokens"))
            .unwrap_or(0);
        let usage = Self {
            prompt_tokens: field("prompt_tokens").unwrap_or(0),
            completion_tokens: field("completion_tokens").unwrap_or(0),
            cached_tokens: cached,
        };
        (usage != Self::default()).then_some(usage)
    }

    pub fn add(&mut self, other: Self) {
        self.prompt_tokens += other.prompt_tokens;
        self.completion_tokens += other.completion_tokens;
        self.cached_tokens += other.cached_tokens;
    }
}

#[derive(Debug, Clone, Default)]
pub struct AiChatChunk {
    pub text_delta: Option<String>,
    pub tool_calls: Vec<AiToolCall>,
    pub finish_reason: Option<String>,
    /// Chega no último pedaço do stream, quando o provedor informa.
    pub usage: Option<AiUsage>,
    /// Modelo que de fato respondeu, quando o provedor informa — um roteador
    /// (`openrouter/free`) escolhe um diferente do configurado.
    pub model: Option<String>,
}

pub type AiChunkStream = Pin<Box<dyn Stream<Item = AppResult<AiChatChunk>> + Send>>;

/// Parâmetros de geração independentes do provedor.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AiChatOptions {
    /// Teto de tokens de saída; `None` deixa o padrão do provedor.
    pub max_tokens: Option<u32>,
}

#[async_trait]
pub trait AiDriver: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;

    /// Modelo configurado; vale quando o stream não diz qual respondeu.
    fn model(&self) -> Option<&str> {
        None
    }

    /// Janela de contexto de `model` segundo o provedor. `None` quando ele não
    /// informa — quem chama cai na estimativa pelo nome.
    async fn context_window(&self, _model: &str) -> Option<LearnedWindow> {
        None
    }

    /// Envia mensagens e ferramentas disponíveis, retornando um stream de deltas de texto e tool calls.
    async fn chat_stream(
        &self,
        messages: &[AiMessage],
        tools: &[AiTool],
        options: AiChatOptions,
    ) -> AppResult<AiChunkStream>;

    /// Testa a conectividade com o provedor, medindo latência e validando a autenticação.
    async fn test_connection(&self) -> AppResult<TestConnectionResponse>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_tool_call_serialization_openai_spec() {
        let tc = AiToolCall {
            id: "call_abc123".into(),
            name: "recent_alerts".into(),
            arguments: "{\"limit\":5}".into(),
        };

        let json_val = serde_json::to_value(&tc).unwrap();
        assert_eq!(json_val["id"], "call_abc123");
        assert_eq!(json_val["type"], "function");
        assert_eq!(json_val["function"]["name"], "recent_alerts");
        assert_eq!(json_val["function"]["arguments"], "{\"limit\":5}");
    }

    #[test]
    fn test_ai_message_with_tool_calls_serialization() {
        let msg = AiMessage {
            role: "assistant".into(),
            content: None,
            tool_calls: Some(vec![AiToolCall {
                id: "call_456".into(),
                name: "infrastructure_summary".into(),
                arguments: "{}".into(),
            }]),
            tool_call_id: None,
        };

        let json_val = serde_json::to_value(&msg).unwrap();
        assert_eq!(json_val["role"], "assistant");
        assert!(json_val.get("content").unwrap().is_null());
        assert_eq!(json_val["tool_calls"][0]["id"], "call_456");
        assert_eq!(json_val["tool_calls"][0]["type"], "function");
        assert_eq!(
            json_val["tool_calls"][0]["function"]["name"],
            "infrastructure_summary"
        );
        assert_eq!(json_val["tool_calls"][0]["function"]["arguments"], "{}");
    }

    #[test]
    fn test_ai_tool_call_deserialization_standard() {
        let json_str = r#"{
            "id": "call_123",
            "type": "function",
            "function": {
                "name": "ping",
                "arguments": "{\"target\":\"1.1.1.1\"}"
            }
        }"#;

        let tc: AiToolCall = serde_json::from_str(json_str).unwrap();
        assert_eq!(tc.id, "call_123");
        assert_eq!(tc.name, "ping");
        assert_eq!(tc.arguments, "{\"target\":\"1.1.1.1\"}");
    }

    #[test]
    fn uso_de_tokens_do_protocolo_openai() {
        let usage = AiUsage::from_openai(&serde_json::json!({
            "prompt_tokens": 1200, "completion_tokens": 85, "total_tokens": 1285
        }));
        assert_eq!(
            usage,
            Some(AiUsage {
                prompt_tokens: 1200,
                completion_tokens: 85,
                cached_tokens: 0,
            })
        );
        let cacheado = AiUsage::from_openai(&serde_json::json!({
            "prompt_tokens": 1200, "completion_tokens": 85,
            "prompt_tokens_details": { "cached_tokens": 1024 }
        }));
        assert_eq!(cacheado.map(|usage| usage.cached_tokens), Some(1024));
        assert_eq!(AiUsage::from_openai(&serde_json::json!(null)), None);

        let mut total = AiUsage::default();
        total.add(usage.unwrap());
        total.add(usage.unwrap());
        assert_eq!(total.completion_tokens, 170);
    }

    #[test]
    fn test_ai_tool_call_deserialization_flat() {
        let json_str = r#"{
            "id": "call_123",
            "name": "ping",
            "arguments": "{\"target\":\"1.1.1.1\"}"
        }"#;

        let tc: AiToolCall = serde_json::from_str(json_str).unwrap();
        assert_eq!(tc.id, "call_123");
        assert_eq!(tc.name, "ping");
        assert_eq!(tc.arguments, "{\"target\":\"1.1.1.1\"}");
    }
}
