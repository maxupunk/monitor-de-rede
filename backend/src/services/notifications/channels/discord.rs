//! Canal Discord (§8.9). Porte literal do `discord_channel.ts`.

use chrono::Utc;
use serde_json::json;

use crate::services::notifications::contracts::{
    ChannelRequest, HttpChannelSpec, NotificationMessage, Severity,
};

/// Cores do embed, em decimal, mantidas estáveis para o histórico do canal
/// continuar visualmente coerente.
const COLOR_CRITICAL: u32 = 15_158_332;
const COLOR_WARNING: u32 = 16_776_960;
const COLOR_INFO: u32 = 3_447_003;

pub struct DiscordChannel {
    webhook_url: String,
}

impl DiscordChannel {
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            webhook_url: std::env::var("DISCORD_WEBHOOK_URL").unwrap_or_default(),
        }
    }

    #[must_use]
    pub fn new(webhook_url: impl Into<String>) -> Self {
        Self {
            webhook_url: webhook_url.into(),
        }
    }
}

impl HttpChannelSpec for DiscordChannel {
    fn name(&self) -> &str {
        "discord"
    }

    fn is_configured(&self) -> bool {
        !self.webhook_url.is_empty()
    }

    fn build_request(&self, message: &NotificationMessage) -> ChannelRequest {
        let color = match message.severity {
            Severity::Critical => COLOR_CRITICAL,
            Severity::Warning => COLOR_WARNING,
            Severity::Info => COLOR_INFO,
        };
        ChannelRequest {
            url: self.webhook_url.clone(),
            body: json!({
                "embeds": [{
                    "title": message.title,
                    "description": message.body,
                    "color": color,
                    "timestamp": Utc::now().to_rfc3339(),
                }]
            }),
            headers: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cor_do_embed_segue_a_severidade() {
        let canal = DiscordChannel::new("https://discord.test/hook");
        let cor = |severity| {
            canal.build_request(&NotificationMessage {
                title: "t".into(),
                body: "b".into(),
                severity,
                metadata: json!({}),
            })
        };
        assert_eq!(
            cor(Severity::Critical).body["embeds"][0]["color"],
            COLOR_CRITICAL
        );
        assert_eq!(
            cor(Severity::Warning).body["embeds"][0]["color"],
            COLOR_WARNING
        );
        assert_eq!(cor(Severity::Info).body["embeds"][0]["color"], COLOR_INFO);
    }

    #[test]
    fn sem_webhook_o_canal_nao_tenta() {
        assert!(!DiscordChannel::new("").is_configured());
    }
}
