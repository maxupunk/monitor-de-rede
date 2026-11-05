//! Contrato dos canais de notificação (§8.9).

use loco_rs::app::AppContext;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Gravidade da mensagem. Mesmo vocabulário de `alert_rules.severity`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

impl Severity {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Critical => "critical",
        }
    }

    /// Lê a severidade gravada na regra. Valor desconhecido vira `warning`, e
    /// não `info`: uma linha antiga com severidade que ninguém reconhece mais
    /// jamais pode acabar silenciando a notificação.
    #[must_use]
    pub fn parse(raw: &str) -> Self {
        match raw {
            "info" => Self::Info,
            "critical" | "error" => Self::Critical,
            _ => Self::Warning,
        }
    }
}

/// Mensagem pronta para entrega, independente do canal.
#[derive(Debug, Clone)]
pub struct NotificationMessage {
    pub title: String,
    pub body: String,
    pub severity: Severity,
    pub metadata: Value,
}

/// Um destino de notificação.
///
/// `send` devolve `bool` em vez de `Result` de propósito: no motor de alertas,
/// falha de entrega **nunca** pode abortar a gravação do alerta. O erro é
/// registrado no canal e o resultado é só "chegou ou não chegou".
#[async_trait::async_trait]
pub trait NotificationChannel: Send + Sync {
    fn name(&self) -> &str;

    async fn send(&self, ctx: &AppContext, message: &NotificationMessage) -> bool;
}

/// Requisição HTTP que um canal de webhook precisa montar.
#[derive(Debug, Clone)]
pub struct ChannelRequest {
    pub url: String,
    pub body: Value,
    pub headers: Vec<(String, String)>,
}

/// O que distingue um canal de webhook do outro.
///
/// Telegram, Discord e webhook genérico fazem o mesmo `POST` JSON com o mesmo
/// tratamento de erro; a única diferença é qual URL e qual corpo montar. Daí
/// composição em vez de herança: [`HttpNotificationChannel`] carrega o envio e
/// o `spec` descreve só o que muda.
pub trait HttpChannelSpec: Send + Sync {
    fn name(&self) -> &str;

    /// Falta configuração (token/URL): o envio é pulado **sem tentar**, para
    /// não gastar um round-trip a cada alerta numa instalação que nunca
    /// configurou o canal.
    fn is_configured(&self) -> bool;

    fn build_request(&self, message: &NotificationMessage) -> ChannelRequest;
}
