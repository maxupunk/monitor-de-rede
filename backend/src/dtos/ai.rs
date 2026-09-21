use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ChatMessageInput {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ChatStreamRequest {
    pub messages: Vec<ChatMessageInput>,
    #[serde(default)]
    pub device_id: Option<i64>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ChatStreamRequestHelper {
    #[serde(rename_all = "camelCase")]
    Object {
        messages: Vec<ChatMessageInput>,
        #[serde(default)]
        device_id: Option<i64>,
    },
    Array(Vec<ChatMessageInput>),
}

impl<'de> Deserialize<'de> for ChatStreamRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let helper = ChatStreamRequestHelper::deserialize(deserializer)?;
        match helper {
            ChatStreamRequestHelper::Object {
                messages,
                device_id,
            } => Ok(Self {
                messages,
                device_id,
            }),
            ChatStreamRequestHelper::Array(messages) => Ok(Self {
                messages,
                device_id: None,
            }),
        }
    }
}

/// Deserializa identificadores de modelo aceitando string simples ou objeto com id/name.
pub fn deserialize_model_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum ModelInput {
        String(String),
        ObjectWithId { id: String },
        ObjectWithName { name: String },
        Fallback(serde_json::Value),
    }

    let input = ModelInput::deserialize(deserializer)?;
    let val = match input {
        ModelInput::String(s) => s,
        ModelInput::ObjectWithId { id } => id,
        ModelInput::ObjectWithName { name } => name,
        ModelInput::Fallback(val) => val
            .get("id")
            .and_then(|v| v.as_str())
            .or_else(|| val.get("name").and_then(|v| v.as_str()))
            .unwrap_or("")
            .to_string(),
    };
    Ok(val.trim().to_string())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestConnectionInputHelper {
    pub driver: String,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(deserialize_with = "deserialize_model_id")]
    pub model: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct TestConnectionInput {
    pub driver: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model: String,
}

impl<'de> Deserialize<'de> for TestConnectionInput {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let h = TestConnectionInputHelper::deserialize(deserializer)?;
        Ok(Self {
            driver: h.driver,
            base_url: h.base_url,
            api_key: h.api_key,
            model: h.model,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct TestConnectionResponse {
    pub success: bool,
    pub latency_ms: f64,
    pub message: String,
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AiToolCallInfo {
    pub id: String,
    pub name: String,
    #[ts(type = "Record<string, unknown>")]
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AiToolResultInfo {
    pub id: String,
    pub name: String,
    #[ts(type = "Record<string, unknown>")]
    pub result: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct OllamaModelItem {
    pub name: String,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub parameter_size: Option<String>,
    #[serde(default)]
    pub modified_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct OllamaRecommendedModel {
    pub name: String,
    pub description: String,
    pub parameter_size: String,
    pub is_installed: bool,
    pub tool_calling_optimized: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct OllamaModelsResponse {
    pub installed: Vec<OllamaModelItem>,
    pub recommended: Vec<OllamaRecommendedModel>,
    #[serde(default = "default_true")]
    pub is_online: bool,
    #[serde(default)]
    pub error_message: Option<String>,
}

const fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct OllamaPullRequest {
    pub model: String,
    #[serde(default)]
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct OllamaPullProgress {
    pub status: String,
    #[serde(default)]
    pub digest: Option<String>,
    #[serde(default)]
    pub total: Option<u64>,
    #[serde(default)]
    pub completed: Option<u64>,
    #[serde(default)]
    pub percentage: Option<f64>,
    pub done: bool,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct OpenCodeModelItem {
    pub id: String,
    #[serde(default)]
    pub is_free: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_tools: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct OpenCodeModelsResponse {
    pub models: Vec<OpenCodeModelItem>,
    #[serde(default = "default_true")]
    pub is_online: bool,
    #[serde(default)]
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct OpenRouterModelItem {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub is_free: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_length: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_tools: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct OpenRouterModelsResponse {
    pub models: Vec<OpenRouterModelItem>,
    #[serde(default = "default_true")]
    pub is_online: bool,
    #[serde(default)]
    pub error_message: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_test_connection_input_object_model() {
        let json_data = r#"{
            "driver": "openrouter",
            "model": {
                "id": "google/gemma-4-26b-a4b-it:free",
                "name": "Google Gemma"
            }
        }"#;

        let input: TestConnectionInput = serde_json::from_str(json_data).unwrap();
        assert_eq!(input.model, "google/gemma-4-26b-a4b-it:free");
    }

    #[test]
    fn test_deserialize_test_connection_input_string_model() {
        let json_data = r#"{
            "driver": "ollama",
            "model": "llama3.2"
        }"#;

        let input: TestConnectionInput = serde_json::from_str(json_data).unwrap();
        assert_eq!(input.model, "llama3.2");
    }

    #[test]
    fn test_deserialize_chat_stream_request_object() {
        let json_data = r#"{
            "messages": [
                { "role": "user", "content": "oi" }
            ],
            "deviceId": 42
        }"#;

        let req: ChatStreamRequest = serde_json::from_str(json_data).unwrap();
        assert_eq!(req.messages.len(), 1);
        assert_eq!(req.messages[0].content, "oi");
        assert_eq!(req.device_id, Some(42));
    }

    #[test]
    fn test_deserialize_chat_stream_request_array() {
        let json_data = r#"[
            { "role": "user", "content": "oi." }
        ]"#;

        let req: ChatStreamRequest = serde_json::from_str(json_data).unwrap();
        assert_eq!(req.messages.len(), 1);
        assert_eq!(req.messages[0].content, "oi.");
        assert_eq!(req.device_id, None);
    }
}
