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
    /// Dispositivo, monitor ou alerta de onde o chat foi aberto: vira contexto
    /// no system prompt para a IA não precisar perguntar "qual?".
    #[serde(default)]
    #[ts(type = "number | null")]
    pub device_id: Option<i64>,
    #[serde(default)]
    #[ts(type = "number | null")]
    pub monitor_id: Option<i64>,
    #[serde(default)]
    #[ts(type = "number | null")]
    pub alert_id: Option<i64>,
    /// O que o usuário marcou com `@` na última pergunta.
    #[serde(default)]
    pub mentions: Vec<AiMention>,
    /// Resumo das mensagens já compactadas — elas não vêm mais em `messages`.
    #[serde(default)]
    pub summary: Option<String>,
    /// O que a resposta anterior mediu: base da decisão de compactar.
    #[serde(default)]
    pub context_hint: Option<ContextHint>,
    /// Compactar agora, mesmo cabendo na janela.
    #[serde(default)]
    pub compact: bool,
}

/// Medida da resposta anterior da conversa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ContextHint {
    /// Tokens da conversa na última rodada (entrada + saída).
    #[ts(type = "number")]
    pub tokens: u64,
    /// Janela do modelo que respondeu.
    #[serde(default)]
    #[ts(type = "number | null")]
    pub window: Option<u64>,
    /// Modelo que respondeu.
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ChatStreamRequestHelper {
    #[serde(rename_all = "camelCase")]
    Object {
        messages: Vec<ChatMessageInput>,
        #[serde(default)]
        device_id: Option<i64>,
        #[serde(default)]
        monitor_id: Option<i64>,
        #[serde(default)]
        alert_id: Option<i64>,
        #[serde(default)]
        mentions: Vec<AiMention>,
        #[serde(default)]
        summary: Option<String>,
        #[serde(default)]
        context_hint: Option<ContextHint>,
        #[serde(default)]
        compact: bool,
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
                monitor_id,
                alert_id,
                mentions,
                summary,
                context_hint,
                compact,
            } => Ok(Self {
                messages,
                device_id,
                monitor_id,
                alert_id,
                mentions,
                summary,
                context_hint,
                compact,
            }),
            ChatStreamRequestHelper::Array(messages) => Ok(Self {
                messages,
                device_id: None,
                monitor_id: None,
                alert_id: None,
                mentions: Vec::new(),
                summary: None,
                context_hint: None,
                compact: false,
            }),
        }
    }
}

/// Tipo do que se marca com `@` no chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum AiMentionKind {
    Device,
    Monitor,
    Container,
    /// Uma fonte de dados inteira: logs, alertas, checagens, Docker.
    Source,
}

/// Algo marcado com `@` na pergunta: diz à IA de qual recurso se trata, sem
/// ela precisar adivinhar pelo nome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AiMention {
    pub kind: AiMentionKind,
    /// Id do dispositivo/monitor, id do container ou nome da fonte.
    pub id: String,
    pub label: String,
    /// Linha curta para a lista de sugestões (IP, tipo, estado).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiMentionQuery {
    pub q: Option<String>,
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

/// Grandeza do eixo Y de um gráfico da IA — o frontend escolhe o formatador por ela.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum AiChartUnit {
    Latency,
    Bandwidth,
    Percentage,
    Generic,
}

/// O que o eixo X representa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum AiChartAxis {
    /// `time` de cada ponto é um instante RFC 3339 — a tela formata a data.
    Time,
    /// `time` é um rótulo pronto ("14h") — a tela mostra como veio.
    Label,
}

/// Amostra de uma série: instante RFC 3339 (ou rótulo, conforme o eixo) e
/// valor na unidade do gráfico.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AiChartPoint {
    pub time: String,
    #[ts(type = "number")]
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AiChartSeries {
    pub id: String,
    pub label: String,
    pub points: Vec<AiChartPoint>,
}

/// Gráfico produzido por uma ferramenta da IA e desenhado no chat com o
/// mesmo componente das telas de monitor e interface.
///
/// Os pontos vão só para a tela; a IA recebe o resumo estatístico.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AiChart {
    pub title: String,
    pub subtitle: Option<String>,
    pub unit: AiChartUnit,
    pub x_axis: AiChartAxis,
    pub series: Vec<AiChartSeries>,
    #[ts(type = "number | null")]
    pub avg_value: Option<f64>,
}

/// Corpo de `POST /api/ai/tools/execute`: a chamada que o usuário confirmou.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ExecuteToolRequest {
    pub name: String,
    #[serde(default)]
    #[ts(type = "Record<string, unknown>")]
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ExecuteToolResponse {
    #[ts(type = "Record<string, unknown>")]
    pub result: serde_json::Value,
    pub chart: Option<AiChart>,
}

/// Corpo de criação/atualização de uma conversa salva do assistente.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AiConversationInput {
    pub title: String,
    /// Mensagens como a tela as exibe (texto, ferramentas, gráficos).
    #[ts(type = "unknown[]")]
    pub messages: serde_json::Value,
}

/// Item da lista de conversas: sem as mensagens, que só vêm ao abrir.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AiConversationSummary {
    #[ts(type = "number")]
    pub id: i64,
    pub title: String,
    pub updated_at: String,
    #[ts(type = "number")]
    pub message_count: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AiConversationDetail {
    #[ts(type = "number")]
    pub id: i64,
    pub title: String,
    pub updated_at: String,
    #[ts(type = "unknown[]")]
    pub messages: serde_json::Value,
}
