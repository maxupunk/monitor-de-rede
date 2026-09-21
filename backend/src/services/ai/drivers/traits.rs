use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

use crate::{dtos::ai::TestConnectionResponse, services::shared::errors::AppResult};

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

#[derive(Debug, Clone, Default)]
pub struct AiChatChunk {
    pub text_delta: Option<String>,
    pub tool_calls: Vec<AiToolCall>,
    pub finish_reason: Option<String>,
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
