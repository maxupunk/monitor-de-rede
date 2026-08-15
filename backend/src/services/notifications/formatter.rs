//! Formatação das mensagens de alerta (§8.9).
//!
//! Os textos são exibidos ao operador em Telegram/Discord/e-mail. Os emojis não
//! são enfeite: numa lista longa de mensagens, são eles que distinguem disparo
//! de recuperação à primeira vista. Mudar o formato muda o que já está no
//! histórico de quem lê essas notificações há meses.

use serde_json::Value;

use super::contracts::{NotificationMessage, Severity};
use crate::services::alerts::state_machine::EpisodeSummary;

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
///
/// Quando o episódio oscilou (houve recaídas dentro da janela de recuperação),
/// o corpo ganha o resumo — "oscilou N vezes; estável há X min" — para que a
/// única notificação de resolução conte a história inteira. Episódio sem
/// recaída não ganha nada: "oscilou 0 vezes" seria ruído numa boa notícia.
#[must_use]
pub fn alert_resolved(
    alert_event_id: i64,
    original_message: Option<&str>,
    reason: &str,
    episode: Option<EpisodeSummary>,
    metadata: Value,
) -> NotificationMessage {
    let mut body = format!(
        "{} foi normalizado. {reason}",
        original_message.unwrap_or("Alerta")
    );
    if let Some(episode) = episode.filter(|summary| summary.recurrence > 0) {
        if !body.ends_with('.') {
            body.push('.');
        }
        body.push_str(&format!(
            " Oscilou {} vezes; estável há {}.",
            episode.recurrence,
            stable_for_text(episode.stable_for_seconds)
        ));
    }
    NotificationMessage {
        title: format!("✅ [RESOLVIDO] Alerta #{alert_event_id}"),
        body,
        severity: Severity::Info,
        metadata,
    }
}

/// "5 min" / "42 s" — o tempo estável da janela em texto curto.
fn stable_for_text(seconds: i64) -> String {
    let minutes = seconds / 60;
    if minutes > 0 {
        format!("{minutes} min")
    } else {
        format!("{seconds} s")
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
            None,
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
        let resolucao = alert_resolved(3, None, "Monitor desativado", None, json!({}));
        assert_eq!(resolucao.body, "Alerta foi normalizado. Monitor desativado");
    }

    #[test]
    fn resolucao_de_episodio_oscilante_traz_o_resumo() {
        let resolucao = alert_resolved(
            4,
            Some("Host inacessível"),
            "Monitoramento normalizado",
            Some(EpisodeSummary {
                recurrence: 7,
                stable_for_seconds: 300,
            }),
            json!({}),
        );
        assert_eq!(
            resolucao.body,
            "Host inacessível foi normalizado. Monitoramento normalizado. \
             Oscilou 7 vezes; estável há 5 min."
        );
    }

    #[test]
    fn episodio_sem_recaida_nao_ganha_resumo() {
        let resolucao = alert_resolved(
            4,
            None,
            "Monitoramento normalizado",
            Some(EpisodeSummary {
                recurrence: 0,
                stable_for_seconds: 45,
            }),
            json!({}),
        );
        assert_eq!(
            resolucao.body,
            "Alerta foi normalizado. Monitoramento normalizado"
        );
    }

    #[test]
    fn tempo_estavel_abaixo_de_um_minuto_sai_em_segundos() {
        assert_eq!(stable_for_text(45), "45 s");
        assert_eq!(stable_for_text(60), "1 min");
        assert_eq!(stable_for_text(359), "5 min");
    }
}
