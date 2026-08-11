//! Canal de webhook genérico (§8.9). Porte literal do `webhook_channel.ts`.

use chrono::Utc;
use serde_json::json;

use crate::services::notifications::contracts::{
    ChannelRequest, HttpChannelSpec, NotificationMessage,
};

pub struct WebhookChannel {
    url: String,
}

impl WebhookChannel {
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            url: std::env::var("GENERIC_WEBHOOK_URL").unwrap_or_default(),
        }
    }

    #[must_use]
    pub fn new(url: impl Into<String>) -> Self {
        Self { url: url.into() }
    }
}

impl HttpChannelSpec for WebhookChannel {
    fn name(&self) -> &str {
        "webhook"
    }

    fn is_configured(&self) -> bool {
        !self.url.is_empty()
    }

    fn build_request(&self, message: &NotificationMessage) -> ChannelRequest {
        ChannelRequest {
            url: self.url.clone(),
            // O corpo replica o do AdonisJS campo a campo: integrações de
            // cliente já leem `message.title`/`message.severity`.
            body: json!({
                "event": "notification",
                "timestamp": Utc::now().to_rfc3339(),
                "message": {
                    "title": message.title,
                    "body": message.body,
                    "severity": message.severity.as_str(),
                    "metadata": message.metadata,
                },
            }),
            headers: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::notifications::contracts::Severity;

    #[test]
    fn preserva_a_forma_do_corpo_esperada_por_integracoes() {
        let request =
            WebhookChannel::new("https://hook.test").build_request(&NotificationMessage {
                title: "Latência alta".into(),
                body: "Switch: 320 ms".into(),
                severity: Severity::Warning,
                metadata: json!({ "alertEventId": 9 }),
            });
        assert_eq!(request.body["event"], "notification");
        assert_eq!(request.body["message"]["title"], "Latência alta");
        assert_eq!(request.body["message"]["severity"], "warning");
        assert_eq!(request.body["message"]["metadata"]["alertEventId"], 9);
    }
}
