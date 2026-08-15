//! Formatação das mensagens de alerta (§8.9).
//!
//! Os textos são exibidos ao operador em Telegram/Discord/e-mail. Os emojis não
//! são enfeite: numa lista longa de mensagens, são eles que distinguem disparo
//! de recuperação à primeira vista. Mudar o formato muda o que já está no
//! histórico de quem lê essas notificações há meses.

use serde_json::Value;

use super::contracts::{NotificationMessage, Severity};
use crate::services::alerts::{
    problem_kind::{ProblemDetail, ProblemKind},
    state_machine::EpisodeSummary,
};

/// Mensagem de disparo: `🚨 [SEVERIDADE] <regra>` + `<alvo>: <mensagem>`.
///
/// Quando o problema foi classificado (Fase 2), o corpo nomeia o tipo em
/// pt-BR — "Tipo: perda de pacotes — 23% de perda" — para o operador saber
/// **o que** está acontecendo sem abrir a Central.
#[must_use]
pub fn alert_triggered(
    rule_name: &str,
    target_label: &str,
    message: &str,
    severity: Severity,
    problem: Option<&ProblemDetail>,
    metadata: Value,
) -> NotificationMessage {
    let mut body = format!("{target_label}: {message}");
    if let Some(problem) = problem {
        if !body.ends_with('.') {
            body.push('.');
        }
        body.push_str(&format!(" Tipo: {}", problem.kind.label()));
        if let Some(observed) = &problem.observed {
            body.push_str(&format!(" — {observed}"));
        }
        body.push('.');
    }
    NotificationMessage {
        title: format!("🚨 [{}] {rule_name}", severity.as_str().to_uppercase()),
        body,
        severity,
        metadata,
    }
}

/// Mensagem única de "alvo oscilando" (Fase 3): `⚠️ [OSCILANDO] <alvo>`.
///
/// É a **única** notificação que a detecção de flapping emite. Daí em diante o
/// episódio segue aberto e silencioso — é esse o ponto: um alvo que cai e volta
/// 20 vezes por hora não pode gerar 40 mensagens. A volta ao normal é contada
/// pela notificação de resolução, que sabe que o episódio oscilou.
#[must_use]
pub fn alert_flapping(
    title: &str,
    transitions: u32,
    flap_window_seconds: i64,
    severity: Severity,
    metadata: Value,
) -> NotificationMessage {
    NotificationMessage {
        title: format!("⚠️ [OSCILANDO] {title}"),
        body: format!(
            "Alvo cronicamente instável: {transitions} recaídas em {}. \
             As notificações deste alerta ficam suspensas até ele estabilizar.",
            stable_for_text(flap_window_seconds)
        ),
        severity,
        metadata,
    }
}

/// Quantos itens a mensagem consolidada enumera antes de resumir o resto.
///
/// Dez cabe numa tela de celular; o que passa disso vira "e mais N". A conta
/// completa continua no título, que é o número que importa.
const DIGEST_PREVIEW: usize = 10;

/// Mensagem consolidada do agrupamento (Fase 4): `🔔 [8 ALERTAS] <grupo>`.
///
/// É o que substitui oito mensagens seguidas quando um site inteiro se mexe ao
/// mesmo tempo. O corpo enumera os alertas em vez de resumi-los: "8 alertas no
/// site Matriz" sem dizer **quais** obrigaria o operador a abrir a Central para
/// descobrir o óbvio.
#[must_use]
pub fn alert_digest(
    group_label: &str,
    items: &[(String, String)],
    severity: Severity,
) -> NotificationMessage {
    let total = items.len();
    let mut body = format!("{total} alertas em {group_label}:");
    for (title, detail) in items.iter().take(DIGEST_PREVIEW) {
        body.push_str(&format!("\n• {title} — {detail}"));
    }
    if total > DIGEST_PREVIEW {
        body.push_str(&format!("\n… e mais {}.", total - DIGEST_PREVIEW));
    }
    NotificationMessage {
        title: format!("🔔 [{total} ALERTAS] {group_label}"),
        body,
        severity,
        metadata: serde_json::json!({
            "digest": true,
            "group": group_label,
            "count": total,
        }),
    }
}

/// Mensagem de normalização. Severidade sempre `info`: o operador não precisa
/// ser acordado porque algo **voltou**.
///
/// Quando o episódio oscilou (houve recaídas dentro da janela de recuperação),
/// o corpo ganha o resumo — "oscilou N vezes; estável há X min" — para que a
/// única notificação de resolução conte a história inteira. Episódio sem
/// recaída não ganha resumo: "oscilou 0 vezes" seria ruído numa boa notícia.
/// Se o episódio chegou a ser declarado oscilante (Fase 3), o resumo abre com
/// "Estabilizou": é esta mensagem que encerra o aviso de flapping. O tipo do
/// problema ("Tipo: perda de pacotes") fecha a mensagem sempre que o episódio
/// foi classificado (Fase 2).
#[must_use]
pub fn alert_resolved(
    alert_event_id: i64,
    original_message: Option<&str>,
    reason: &str,
    episode: Option<EpisodeSummary>,
    problem_kind: Option<ProblemKind>,
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
            " {} {} vezes; estável há {}.",
            if episode.flapped {
                "Estabilizou após oscilar"
            } else {
                "Oscilou"
            },
            episode.recurrence,
            stable_for_text(episode.stable_for_seconds)
        ));
    }
    if let Some(kind) = problem_kind {
        if !body.ends_with('.') {
            body.push('.');
        }
        body.push_str(&format!(" Tipo: {}.", kind.label()));
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
            None,
            json!({}),
        );
        assert_eq!(disparo.title, "🚨 [CRITICAL] Dispositivo sem resposta");
        assert_eq!(disparo.body, "Roteador Matriz: Host inacessível");

        let resolucao = alert_resolved(
            9,
            Some("Host inacessível"),
            "Monitoramento normalizado",
            None,
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
    fn disparo_nomeia_o_tipo_do_problema_e_o_valor_observado() {
        let com_valor = alert_triggered(
            "Perda de pacotes acima de 10%",
            "Roteador Matriz",
            "Ping para 10.0.0.1 com 23% perda",
            Severity::Warning,
            Some(&ProblemDetail {
                kind: ProblemKind::PacketLoss,
                observed: Some("23% de perda".to_string()),
            }),
            json!({}),
        );
        assert_eq!(
            com_valor.body,
            "Roteador Matriz: Ping para 10.0.0.1 com 23% perda. \
             Tipo: perda de pacotes — 23% de perda."
        );

        let sem_valor = alert_triggered(
            "Túnel VPN instável",
            "filial-01",
            "Túnel VPN de filial-01 ficou instável.",
            Severity::Warning,
            Some(&ProblemDetail {
                kind: ProblemKind::VpnInstability,
                observed: None,
            }),
            json!({}),
        );
        assert_eq!(
            sem_valor.body,
            "filial-01: Túnel VPN de filial-01 ficou instável. Tipo: instabilidade VPN."
        );
    }

    #[test]
    fn resolucao_sem_mensagem_original_usa_o_rotulo_generico() {
        let resolucao = alert_resolved(3, None, "Monitor desativado", None, None, json!({}));
        assert_eq!(resolucao.body, "Alerta foi normalizado. Monitor desativado");
    }

    #[test]
    fn resolucao_de_episodio_oscilante_traz_o_resumo_e_o_tipo() {
        let resolucao = alert_resolved(
            4,
            Some("Host inacessível"),
            "Monitoramento normalizado",
            Some(EpisodeSummary {
                recurrence: 7,
                stable_for_seconds: 300,
                flapped: false,
            }),
            Some(ProblemKind::PacketLoss),
            json!({}),
        );
        assert_eq!(
            resolucao.body,
            "Host inacessível foi normalizado. Monitoramento normalizado. \
             Oscilou 7 vezes; estável há 5 min. Tipo: perda de pacotes."
        );
    }

    #[test]
    fn episodio_sem_recaida_nao_ganha_resumo_mas_nomeia_o_tipo() {
        let resolucao = alert_resolved(
            4,
            None,
            "Monitoramento normalizado",
            Some(EpisodeSummary {
                recurrence: 0,
                stable_for_seconds: 45,
                flapped: false,
            }),
            Some(ProblemKind::DnsFailure),
            json!({}),
        );
        assert_eq!(
            resolucao.body,
            "Alerta foi normalizado. Monitoramento normalizado. Tipo: falha de DNS."
        );
    }

    #[test]
    fn o_aviso_de_oscilacao_diz_quantas_recaidas_e_que_vai_silenciar() {
        let aviso = alert_flapping(
            "Dispositivo sem resposta — Roteador Matriz",
            5,
            900,
            Severity::Warning,
            json!({}),
        );
        assert_eq!(
            aviso.title,
            "⚠️ [OSCILANDO] Dispositivo sem resposta — Roteador Matriz"
        );
        assert_eq!(
            aviso.body,
            "Alvo cronicamente instável: 5 recaídas em 15 min. \
             As notificações deste alerta ficam suspensas até ele estabilizar."
        );
        assert_eq!(aviso.severity, Severity::Warning);
    }

    #[test]
    fn a_resolucao_de_um_episodio_que_oscilou_encerra_o_aviso_de_flapping() {
        let resolucao = alert_resolved(
            8,
            Some("Host inacessível"),
            "Monitoramento normalizado",
            Some(EpisodeSummary {
                recurrence: 12,
                stable_for_seconds: 900,
                flapped: true,
            }),
            None,
            json!({}),
        );
        assert_eq!(
            resolucao.body,
            "Host inacessível foi normalizado. Monitoramento normalizado. \
             Estabilizou após oscilar 12 vezes; estável há 15 min."
        );
    }

    #[test]
    fn tempo_estavel_abaixo_de_um_minuto_sai_em_segundos() {
        assert_eq!(stable_for_text(45), "45 s");
        assert_eq!(stable_for_text(60), "1 min");
        assert_eq!(stable_for_text(359), "5 min");
    }

    #[test]
    fn a_mensagem_consolidada_conta_e_enumera_os_alertas() {
        let itens = vec![
            (
                "Queda — Roteador".to_string(),
                "Host inacessível".to_string(),
            ),
            ("Queda — Switch".to_string(), "Host inacessível".to_string()),
        ];
        let digest = alert_digest("Matriz", &itens, Severity::Critical);
        assert_eq!(digest.title, "🔔 [2 ALERTAS] Matriz");
        assert_eq!(
            digest.body,
            "2 alertas em Matriz:\n\
             • Queda — Roteador — Host inacessível\n\
             • Queda — Switch — Host inacessível"
        );
        assert_eq!(digest.severity, Severity::Critical);
        assert_eq!(digest.metadata["digest"], json!(true));
        assert_eq!(digest.metadata["count"], json!(2));
    }

    #[test]
    fn a_lista_da_consolidacao_para_no_decimo_item() {
        let itens: Vec<(String, String)> = (0..14)
            .map(|index| (format!("Alerta {index}"), "detalhe".to_string()))
            .collect();
        let digest = alert_digest("Matriz", &itens, Severity::Warning);
        assert_eq!(digest.title, "🔔 [14 ALERTAS] Matriz");
        assert_eq!(digest.body.matches("\n• ").count(), DIGEST_PREVIEW);
        assert!(digest.body.ends_with("… e mais 4."));
    }
}
