//! O aviso que o navegador manda quando uma requisição não teve resposta.

use serde::Deserialize;

/// Como a requisição terminou do lado do navegador.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClientFailureKind {
    /// O navegador desistiu de esperar a resposta.
    Timeout,
    /// A conexão caiu ou nem abriu (rede, proxy, túnel).
    Network,
}

impl ClientFailureKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Timeout => "tempo esgotado",
            Self::Network => "conexão interrompida",
        }
    }
}

/// `POST /api/me/client-failures`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientFailureInput {
    pub method: String,
    /// Caminho da API (`/api/docker/hosts/agent-9/containers`), sem query.
    pub path: String,
    pub kind: ClientFailureKind,
    /// Quanto o navegador esperou.
    pub elapsed_ms: u64,
    /// A tela em que o operador estava (`/docker/compose`).
    #[serde(default)]
    pub page: Option<String>,
}
