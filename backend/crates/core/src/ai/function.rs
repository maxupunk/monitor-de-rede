//! O contrato de uma ferramenta no formato de function calling da OpenAI —
//! o que vai ao provedor em cada rodada.

use serde::{Deserialize, Serialize};

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
