//! Avisos derivados de VPN + ping (§8.10.4).

use serde::Serialize;

use crate::models::{monitors, vpn_peers};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerHints {
    /// Túnel ativo, mas o ping falha: provável firewall bloqueando na interface WG.
    pub needs_firewall_hint: bool,
    /// Túnel ativo e ping falhando, mas o monitor **não** roda no `vpn-probe`: o
    /// ICMP sai da máquina da API, que não tem rota para dentro do túnel. O
    /// pacote nem chega ao equipamento — acusar o firewall dele seria
    /// diagnóstico falso.
    pub ping_outside_tunnel: bool,
    /// Monitor de ping provisionado automaticamente para o peer — usado para
    /// navegar ao histórico de conectividade.
    pub ping_monitor_id: Option<i64>,
}

/// Único ponto de cálculo dos avisos — usado tanto no `GET /api/vpn/peers`
/// (carga inicial) quanto no snapshot publicado via SSE (`vpn:peers_updated`),
/// para que os dois caminhos nunca divirjam.
///
/// A régua aqui é `has_fresh_proof_of_life`, **não**
/// `connection_status == connected` (matriz de paridade #39). O ping falha em
/// segundos e vira `down` no primeiro erro, enquanto a janela de "conectado"
/// tolera minutos de propósito: quem desconectasse o equipamento caía na brecha
/// entre as duas e via "túnel conectado, mas não responde a ping" — afirmando
/// justamente o contrário do que havia acontecido.
#[must_use]
pub fn compute_peer_hints(peer: &vpn_peers::Model, monitor: Option<&monitors::Model>) -> PeerHints {
    let silent_tunnel =
        peer.has_fresh_proof_of_life() && monitor.is_some_and(|item| item.status == "down");
    let outside_tunnel = silent_tunnel && monitor.is_some_and(|item| item.probe_id.is_none());

    PeerHints {
        needs_firewall_hint: silent_tunnel && !outside_tunnel,
        ping_outside_tunnel: outside_tunnel,
        ping_monitor_id: monitor.map(|item| item.id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    fn peer(seen_ago: Option<i64>) -> vpn_peers::Model {
        let now = Utc::now();
        vpn_peers::Model {
            id: 1,
            vpn_server_id: 1,
            device_id: 1,
            public_key: "PUB".into(),
            preshared_key_encrypted: None,
            device_profile: "linux".into(),
            persistent_keepalive: 25,
            last_handshake_at: None,
            last_seen_at: seen_ago.map(|s| (now - Duration::seconds(s)).into()),
            bytes_rx: 0,
            bytes_tx: 0,
            enabled: true,
            last_connection_status: None,
            created_at: now.into(),
            updated_at: now.into(),
        }
    }

    fn monitor(status: &str, probe_id: Option<i64>) -> monitors::Model {
        let now = Utc::now();
        monitors::Model {
            id: 7,
            device_id: Some(1),
            probe_id,
            r#type: "ping".into(),
            name: "Ping".into(),
            configuration: serde_json::json!({}),
            interval_seconds: 60,
            timeout_seconds: 5,
            retry_count: 3,
            enabled: true,
            last_run_at: None,
            next_run_at: None,
            status: status.into(),
            created_at: now.into(),
            updated_at: now.into(),
        }
    }

    #[test]
    fn tunel_vivo_com_ping_caido_no_vpn_probe_acusa_o_firewall() {
        let hints = compute_peer_hints(&peer(Some(10)), Some(&monitor("down", Some(3))));
        assert!(hints.needs_firewall_hint);
        assert!(!hints.ping_outside_tunnel);
        assert_eq!(hints.ping_monitor_id, Some(7));
    }

    #[test]
    fn ping_fora_do_tunel_nao_acusa_o_firewall_do_equipamento() {
        // Matriz de paridade #40: sem `probeId`, o ICMP sai da API e nem chega.
        let hints = compute_peer_hints(&peer(Some(10)), Some(&monitor("down", None)));
        assert!(!hints.needs_firewall_hint);
        assert!(hints.ping_outside_tunnel);
    }

    #[test]
    fn tunel_sem_prova_de_vida_recente_nao_gera_aviso() {
        // 100 s: ainda "conectado" (janela de 150 s), mas fora da prova de vida
        // (80 s). É a brecha que o #39 fecha.
        let hints = compute_peer_hints(&peer(Some(100)), Some(&monitor("down", Some(3))));
        assert!(!hints.needs_firewall_hint);
        assert!(!hints.ping_outside_tunnel);
    }

    #[test]
    fn ping_saudavel_nunca_gera_aviso() {
        let hints = compute_peer_hints(&peer(Some(10)), Some(&monitor("up", Some(3))));
        assert!(!hints.needs_firewall_hint);
        assert!(!hints.ping_outside_tunnel);
    }

    #[test]
    fn sem_monitor_de_ping_nao_ha_diagnostico() {
        let hints = compute_peer_hints(&peer(Some(10)), None);
        assert!(!hints.needs_firewall_hint);
        assert!(!hints.ping_outside_tunnel);
        assert_eq!(hints.ping_monitor_id, None);
    }

    #[test]
    fn serializa_em_camel_case_para_o_frontend() {
        let json = serde_json::to_value(compute_peer_hints(&peer(None), None)).unwrap();
        assert!(json.get("needsFirewallHint").is_some());
        assert!(json.get("pingOutsideTunnel").is_some());
        assert!(json.get("pingMonitorId").is_some());
    }
}
