//! Formatação das mensagens de alerta (§8.9).
//!
//! Os textos são exibidos ao operador em Telegram/Discord/e-mail. Os emojis não
//! são enfeite: numa lista longa de mensagens, são eles que distinguem disparo
//! de recuperação à primeira vista. Mudar o formato muda o que já está no
//! histórico de quem lê essas notificações há meses.

use serde_json::Value;

use super::contracts::{NotificationMessage, Severity};

/// Mensagem de disparo: `🚨 [SEVERIDADE] <regra>` + `<alvo>: <mensagem>`.
#[must_use]
pub fn alert_triggered(
    rule_name: &str,
    target_label: &str,
    message: &str,
    severity: Severity,
    metadata: Value,
) -> NotificationMessage {
    NotificationMessage {
        title: format!("🚨 [{}] {rule_name}", severity.as_str().to_uppercase()),
        body: format!("{target_label}: {message}"),
        severity,
        metadata,
    }
}

/// Mensagem de normalização. Severidade sempre `info`: o operador não precisa
/// ser acordado porque algo **voltou**.
#[must_use]
pub fn alert_resolved(
    alert_event_id: i64,
    original_message: Option<&str>,
    reason: &str,
    metadata: Value,
) -> NotificationMessage {
    NotificationMessage {
        title: format!("✅ [RESOLVIDO] Alerta #{alert_event_id}"),
        body: format!(
            "{} foi normalizado. {reason}",
            original_message.unwrap_or("Alerta")
        ),
        severity: Severity::Info,
        metadata,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn textos_batem_com_os_do_backend_anterior() {
        let disparo = alert_triggered(
            "Dispositivo sem resposta",
            "Roteador Matriz",
            "Host inacessível",
            Severity::Critical,
            json!({}),
        );
        assert_eq!(disparo.title, "🚨 [CRITICAL] Dispositivo sem resposta");
        assert_eq!(disparo.body, "Roteador Matriz: Host inacessível");

        let resolucao = alert_resolved(
            9,
            Some("Host inacessível"),
            "Monitoramento normalizado",
            json!({}),
        );
        assert_eq!(resolucao.title, "✅ [RESOLVIDO] Alerta #9");
        assert_eq!(
            resolucao.body,
            "Host inacessível foi normalizado. Monitoramento normalizado"
        );
        assert_eq!(resolucao.severity, Severity::Info);
    }

    #[test]
    fn resolucao_sem_mensagem_original_usa_o_rotulo_generico() {
        let resolucao = alert_resolved(3, None, "Monitor desativado", json!({}));
        assert_eq!(resolucao.body, "Alerta foi normalizado. Monitor desativado");
    }
}
