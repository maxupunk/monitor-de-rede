//! Estado de um túnel WireGuard traduzido para o vocabulário das regras (§8.7).
//!
//! Só publica fatos, como o dataset de interfaces: a decisão de alertar — e com
//! qual severidade — pertence às regras cadastradas.

use serde_json::{json, Value};

use crate::{
    models::vpn_peers::VpnPeerConnectionStatus,
    services::alerts::{
        contracts::AlertDataset,
        fields::{self, vpn_status_transition},
    },
};

/// O que se sabe do túnel neste ciclo.
#[derive(Debug, Clone)]
pub struct VpnPeerFacts<'a> {
    pub peer_name: &'a str,
    pub status: VpnPeerConnectionStatus,
    pub previous_status: Option<VpnPeerConnectionStatus>,
    pub seconds_since_activity: Option<i64>,
}

/// Estado em que o túnel é considerado no ar para efeito de transição.
const fn is_healthy(status: VpnPeerConnectionStatus) -> bool {
    matches!(status, VpnPeerConnectionStatus::Connected)
}

#[must_use]
pub fn status_label(status: VpnPeerConnectionStatus) -> &'static str {
    match status {
        VpnPeerConnectionStatus::Connected => "connected",
        VpnPeerConnectionStatus::Unstable => "unstable",
        VpnPeerConnectionStatus::Disconnected => "disconnected",
        VpnPeerConnectionStatus::Awaiting => "awaiting",
    }
}

#[must_use]
pub fn build(facts: &VpnPeerFacts<'_>) -> AlertDataset {
    let mut dataset = AlertDataset::new();
    dataset.insert(fields::VPN_PEER_NAME.into(), json!(facts.peer_name));
    dataset.insert(
        fields::VPN_PEER_STATUS.into(),
        json!(status_label(facts.status)),
    );

    if let Some(seconds) = facts.seconds_since_activity {
        dataset.insert(fields::VPN_SECONDS_SINCE_ACTIVITY.into(), json!(seconds));
    }

    if let Some(transition) = resolve_transition(facts.previous_status, facts.status) {
        dataset.insert(fields::VPN_STATUS_TRANSITION.into(), json!(transition));
        dataset.insert(
            fields::VPN_PREVIOUS_STATUS.into(),
            facts
                .previous_status
                .map_or(Value::Null, |status| json!(status_label(status))),
        );
    }

    dataset
}

/// Qual transição o par (anterior, atual) descreve.
///
/// Sem estado anterior não há transição: o primeiro ciclo depois de criar o
/// peer — ou depois de subir a versão que passou a persistir o estado — só
/// estabelece a linha de base. Alertar ali reportaria como queda um túnel que
/// talvez nunca tenha subido (matriz de paridade #38).
#[must_use]
pub fn resolve_transition(
    previous: Option<VpnPeerConnectionStatus>,
    current: VpnPeerConnectionStatus,
) -> Option<&'static str> {
    let previous = previous?;
    if previous == current {
        return None;
    }

    let was_healthy = is_healthy(previous);
    if was_healthy && current == VpnPeerConnectionStatus::Disconnected {
        return Some(vpn_status_transition::DISCONNECTED);
    }
    if was_healthy && current == VpnPeerConnectionStatus::Unstable {
        return Some(vpn_status_transition::DESTABILIZED);
    }
    // `awaiting ➔ connected` também é retorno: o túnel subiu pela primeira vez
    // e qualquer alerta aberto sobre ele deixou de fazer sentido.
    if !was_healthy && is_healthy(current) {
        return Some(vpn_status_transition::RECONNECTED);
    }
    // Degradação em cadeia (`unstable ➔ disconnected`) conta como queda: quem
    // configurou a regra de queda espera ser avisado, mesmo que o túnel já
    // estivesse claudicando no ciclo anterior.
    if current == VpnPeerConnectionStatus::Disconnected {
        return Some(vpn_status_transition::DISCONNECTED);
    }
    None
}

/// `true` quando o dataset descreve alguma mudança de estado do túnel.
#[must_use]
pub fn has_transition(dataset: &AlertDataset) -> bool {
    dataset.contains_key(fields::VPN_STATUS_TRANSITION)
}

/// `true` quando o túnel voltou — sinaliza ao motor que os alertas podem fechar.
#[must_use]
pub fn is_recovery(dataset: &AlertDataset) -> bool {
    dataset.get(fields::VPN_STATUS_TRANSITION) == Some(&json!(vpn_status_transition::RECONNECTED))
}

/// Frase legível do que foi observado, usada como mensagem do alerta.
#[must_use]
pub fn describe(dataset: &AlertDataset) -> String {
    let name = dataset
        .get(fields::VPN_PEER_NAME)
        .and_then(Value::as_str)
        .unwrap_or("desconhecido");
    let silence = dataset
        .get(fields::VPN_SECONDS_SINCE_ACTIVITY)
        .and_then(Value::as_i64)
        .map_or_else(String::new, |seconds| {
            format!(" Sem sinal há {}.", format_seconds(seconds))
        });

    match dataset
        .get(fields::VPN_STATUS_TRANSITION)
        .and_then(Value::as_str)
    {
        Some(vpn_status_transition::DISCONNECTED) => {
            format!("Túnel VPN de {name} caiu.{silence}")
        }
        Some(vpn_status_transition::DESTABILIZED) => {
            format!("Túnel VPN de {name} ficou instável.{silence}")
        }
        Some(vpn_status_transition::RECONNECTED) => {
            format!("Túnel VPN de {name} voltou a responder.")
        }
        _ => {
            let status = dataset
                .get(fields::VPN_PEER_STATUS)
                .and_then(Value::as_str)
                .unwrap_or("desconhecido");
            format!(
                "Túnel VPN de {name} está {}.{silence}",
                status_in_portuguese(status)
            )
        }
    }
}

fn status_in_portuguese(status: &str) -> &str {
    match status {
        "connected" => "conectado",
        "unstable" => "instável",
        "disconnected" => "desconectado",
        "awaiting" => "aguardando primeira conexão",
        other => other,
    }
}

fn format_seconds(seconds: i64) -> String {
    if seconds < 60 {
        return format!("{seconds}s");
    }
    let minutes = seconds / 60;
    if minutes < 60 {
        return format!("{minutes} min");
    }
    format!("{}h{:02}", minutes / 60, minutes % 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use VpnPeerConnectionStatus::{Awaiting, Connected, Disconnected, Unstable};

    #[test]
    fn sem_estado_anterior_o_ciclo_so_estabelece_a_linha_de_base() {
        assert_eq!(resolve_transition(None, Disconnected), None);
        let dataset = build(&VpnPeerFacts {
            peer_name: "filial-01",
            status: Disconnected,
            previous_status: None,
            seconds_since_activity: None,
        });
        assert!(!has_transition(&dataset));
    }

    #[test]
    fn classifica_todas_as_transicoes_de_status() {
        assert_eq!(
            resolve_transition(Some(Connected), Disconnected),
            Some(vpn_status_transition::DISCONNECTED)
        );
        assert_eq!(
            resolve_transition(Some(Connected), Unstable),
            Some(vpn_status_transition::DESTABILIZED)
        );
        assert_eq!(
            resolve_transition(Some(Unstable), Connected),
            Some(vpn_status_transition::RECONNECTED)
        );
        assert_eq!(
            resolve_transition(Some(Awaiting), Connected),
            Some(vpn_status_transition::RECONNECTED)
        );
        // Degradação em cadeia ainda é queda.
        assert_eq!(
            resolve_transition(Some(Unstable), Disconnected),
            Some(vpn_status_transition::DISCONNECTED)
        );
        // `connected ➔ connected` não é transição.
        assert_eq!(resolve_transition(Some(Connected), Connected), None);
        // `disconnected ➔ awaiting` não descreve nada alertável.
        assert_eq!(resolve_transition(Some(Disconnected), Awaiting), None);
    }

    #[test]
    fn mensagens_incluem_o_tempo_de_silencio() {
        let dataset = build(&VpnPeerFacts {
            peer_name: "filial-01",
            status: Disconnected,
            previous_status: Some(Connected),
            seconds_since_activity: Some(3_720),
        });
        assert_eq!(
            describe(&dataset),
            "Túnel VPN de filial-01 caiu. Sem sinal há 1h02."
        );
        assert!(!is_recovery(&dataset));
    }

    #[test]
    fn retorno_conta_como_recuperacao_e_omite_o_silencio() {
        let dataset = build(&VpnPeerFacts {
            peer_name: "filial-01",
            status: Connected,
            previous_status: Some(Disconnected),
            seconds_since_activity: Some(10),
        });
        assert!(is_recovery(&dataset));
        assert_eq!(
            describe(&dataset),
            "Túnel VPN de filial-01 voltou a responder."
        );
    }

    #[test]
    fn sem_transicao_descreve_o_estado_atual_em_portugues() {
        let dataset = build(&VpnPeerFacts {
            peer_name: "filial-01",
            status: Awaiting,
            previous_status: None,
            seconds_since_activity: None,
        });
        assert_eq!(
            describe(&dataset),
            "Túnel VPN de filial-01 está aguardando primeira conexão."
        );
    }

    #[test]
    fn escalas_de_tempo() {
        assert_eq!(format_seconds(45), "45s");
        assert_eq!(format_seconds(90), "1 min");
        assert_eq!(format_seconds(3_600), "1h00");
    }
}
