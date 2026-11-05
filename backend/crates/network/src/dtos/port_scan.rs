//! Entrada da varredura de portas.

use serde::Deserialize;

/// Payload da ferramenta de varredura. A validação semântica fica no
/// controller para que a mensagem continue compatível com o frontend.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortScanInput {
    pub host: String,
    pub protocol: String,
    pub ports: Vec<u16>,
    pub timeout_ms: Option<u64>,
    pub profile: Option<String>,
}
