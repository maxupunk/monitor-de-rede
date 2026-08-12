//! Canal de e-mail (§8.9).
//!
//! Diferente dos outros três, este não é webhook: usa o `EmailSender` que o
//! Loco monta a partir de `config/*.yaml`. A entrega é real quando existe
//! mailer configurado e cai no log quando não existe — que é o caso do
//! ambiente de teste, cujo mailer é `stub`, e o do compose, que não tem SMTP.

use loco_rs::{app::AppContext, mailer::Email};

use crate::services::notifications::contracts::{NotificationChannel, NotificationMessage};

/// Destinatário usado quando `SMTP_TO` não está definido.
const DEFAULT_RECIPIENT: &str = "admin@monitor.local";

pub struct EmailChannel {
    recipient: String,
}

impl EmailChannel {
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            recipient: std::env::var("SMTP_TO")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_RECIPIENT.to_string()),
        }
    }

    #[must_use]
    pub fn new(recipient: impl Into<String>) -> Self {
        Self {
            recipient: recipient.into(),
        }
    }

    /// Assunto prefixado pela severidade em maiúsculas: é o que permite filtrar
    /// a caixa por `[CRITICAL]`. Formato observável — não reordene.
    fn subject(message: &NotificationMessage) -> String {
        format!(
            "[{}] {}",
            message.severity.as_str().to_uppercase(),
            message.title
        )
    }
}

#[async_trait::async_trait]
impl NotificationChannel for EmailChannel {
    fn name(&self) -> &str {
        "email"
    }

    async fn send(&self, ctx: &AppContext, message: &NotificationMessage) -> bool {
        let subject = Self::subject(message);
        let Some(mailer) = ctx.mailer.as_ref() else {
            tracing::info!(
                to = %self.recipient,
                %subject,
                body = %message.body,
                "canal de e-mail sem mailer configurado; notificação apenas registrada"
            );
            return false;
        };

        let email = Email {
            to: self.recipient.clone(),
            subject,
            text: message.body.clone(),
            html: format!("<p>{}</p>", html_escape(&message.body)),
            ..Default::default()
        };

        match mailer.mail(&email).await {
            Ok(()) => true,
            Err(error) => {
                tracing::warn!(%error, "erro ao enviar notificação por e-mail");
                false
            }
        }
    }
}

/// Escapa o mínimo para o corpo HTML: a mensagem carrega nome de dispositivo
/// digitado pelo operador, e um `<` solto quebraria o corpo do e-mail.
fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::notifications::contracts::Severity;

    #[test]
    fn assunto_carrega_a_severidade_em_caixa_alta() {
        let message = NotificationMessage {
            title: "Túnel VPN caiu".into(),
            body: "filial-01".into(),
            severity: Severity::Critical,
            metadata: serde_json::json!({}),
        };
        assert_eq!(EmailChannel::subject(&message), "[CRITICAL] Túnel VPN caiu");
    }

    #[test]
    fn corpo_html_nao_quebra_com_nome_de_dispositivo_marcado() {
        assert_eq!(
            html_escape("Switch <core> & AP"),
            "Switch &lt;core&gt; &amp; AP"
        );
    }
}
