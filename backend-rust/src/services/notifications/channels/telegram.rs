//! Canal Telegram (§8.9). Porte literal do `telegram_channel.ts`.

use serde_json::json;

use crate::services::notifications::contracts::{
    ChannelRequest, HttpChannelSpec, NotificationMessage,
};

pub struct TelegramChannel {
    bot_token: String,
    chat_id: String,
}

impl TelegramChannel {
    /// Lê `TELEGRAM_BOT_TOKEN` e `TELEGRAM_CHAT_ID`, como no backend anterior.
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            bot_token: std::env::var("TELEGRAM_BOT_TOKEN").unwrap_or_default(),
            chat_id: std::env::var("TELEGRAM_CHAT_ID").unwrap_or_default(),
        }
    }

    #[must_use]
    pub fn new(bot_token: impl Into<String>, chat_id: impl Into<String>) -> Self {
        Self {
            bot_token: bot_token.into(),
            chat_id: chat_id.into(),
        }
    }
}

impl HttpChannelSpec for TelegramChannel {
    fn name(&self) -> &str {
        "telegram"
    }

    fn is_configured(&self) -> bool {
        !self.bot_token.is_empty() && !self.chat_id.is_empty()
    }

    fn build_request(&self, message: &NotificationMessage) -> ChannelRequest {
        let text = format!(
            "🚨 *[{}] {}*\n\n{}",
            message.severity.as_str().to_uppercase(),
            message.title,
            message.body
        );
        ChannelRequest {
            url: format!("https://api.telegram.org/bot{}/sendMessage", self.bot_token),
            body: json!({
                "chat_id": self.chat_id,
                "text": text,
                "parse_mode": "Markdown",
            }),
            headers: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::notifications::contracts::Severity;

    fn mensagem() -> NotificationMessage {
        NotificationMessage {
            title: "Dispositivo sem resposta".into(),
            body: "Roteador: sem resposta".into(),
            severity: Severity::Critical,
            metadata: json!({}),
        }
    }

    #[test]
    fn sem_token_ou_chat_o_canal_nao_tenta() {
        assert!(!TelegramChannel::new("", "").is_configured());
        assert!(!TelegramChannel::new("token", "").is_configured());
        assert!(TelegramChannel::new("token", "123").is_configured());
    }

    #[test]
    fn monta_o_mesmo_payload_do_backend_anterior() {
        let request = TelegramChannel::new("t", "42").build_request(&mensagem());
        assert_eq!(request.url, "https://api.telegram.org/bott/sendMessage");
        assert_eq!(request.body["chat_id"], "42");
        assert_eq!(request.body["parse_mode"], "Markdown");
        assert_eq!(
            request.body["text"],
            "🚨 *[CRITICAL] Dispositivo sem resposta*\n\nRoteador: sem resposta"
        );
    }
}
