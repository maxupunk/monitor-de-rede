//! Contratos da avaliação de alertas (§8.7).
//!
//! Quem observa a rede (monitor, coleta SNMP, telemetria de túnel) só produz
//! *fatos*; quem decide se aquilo vira alerta — e com qual severidade — são as
//! regras cadastradas. Este contrato é a fronteira entre os dois lados: nenhum
//! produtor de fatos precisa conhecer `alert_events`, notificação ou severidade.

use serde_json::{Map, Value};

/// Fatos observados no vocabulário avaliado pelas regras (`condition.field`).
///
/// É um `Map<String, Value>` e não um struct porque o conjunto de campos é
/// aberto: um checker novo publica chaves novas sem que o avaliador mude.
pub type AlertDataset = Map<String, Value>;

/// Delimita quais regras se aplicam ao fato observado (`None` = regra global).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AlertEvaluationScope {
    pub site_id: Option<i64>,
    pub device_id: Option<i64>,
    pub monitor_id: Option<i64>,
}

/// Tudo que o motor precisa para decidir sobre um fato observado.
#[derive(Debug, Clone)]
pub struct AlertEvaluationContext {
    pub scope: AlertEvaluationScope,

    /// Identidade do alvo avaliado (`monitor:12`, `interface:34`). Deduplica os
    /// eventos ativos e delimita a normalização automática.
    pub scope_key: String,

    /// Rótulo do alvo exibido no título do alerta.
    pub target_label: String,

    pub dataset: AlertDataset,

    /// Descrição legível do fato; cai para o nome da regra quando ausente.
    pub message: Option<String>,

    /// Conteúdo extra persistido em `alert_events.data`.
    pub data: Map<String, Value>,

    /// `true` quando o alvo voltou ao normal. Se nenhuma regra disparar nesta
    /// avaliação, os alertas abertos do escopo são resolvidos.
    pub recovered: bool,
}

/// Status de `alert_events` que contam como alerta ainda aberto.
///
/// Reconhecer ou silenciar não fecha o alerta — só muda como ele aparece. Por
/// isso os três aparecem juntos tanto na deduplicação quanto na recuperação.
pub const OPEN_STATUSES: [&str; 3] = ["active", "acknowledged", "silenced"];

pub const STATUS_ACTIVE: &str = "active";
pub const STATUS_ACKNOWLEDGED: &str = "acknowledged";
pub const STATUS_SILENCED: &str = "silenced";
pub const STATUS_RESOLVED: &str = "resolved";

/// Chaves de escopo — centralizadas para produtor e consumidor não divergirem.
pub struct AlertScopeKey;

impl AlertScopeKey {
    #[must_use]
    pub fn monitor(monitor_id: i64) -> String {
        format!("monitor:{monitor_id}")
    }

    #[must_use]
    pub fn interface(interface_id: i64) -> String {
        format!("interface:{interface_id}")
    }

    #[must_use]
    pub fn vpn_peer(vpn_peer_id: i64) -> String {
        format!("vpn_peer:{vpn_peer_id}")
    }

    /// Extrai o id quando a chave é de monitor.
    ///
    /// Existe por causa da recuperação: alertas antigos foram gravados só com
    /// `monitor_id` preenchido, antes de `scope_key` existir. Fechar apenas por
    /// chave deixaria esses eventos abertos para sempre.
    #[must_use]
    pub fn monitor_id_of(scope_key: &str) -> Option<i64> {
        scope_key.strip_prefix("monitor:")?.parse().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chaves_de_escopo_usam_o_formato_tipo_dois_pontos_id() {
        assert_eq!(AlertScopeKey::monitor(12), "monitor:12");
        assert_eq!(AlertScopeKey::interface(34), "interface:34");
        assert_eq!(AlertScopeKey::vpn_peer(7), "vpn_peer:7");
    }

    #[test]
    fn so_extrai_id_de_chave_de_monitor() {
        assert_eq!(AlertScopeKey::monitor_id_of("monitor:12"), Some(12));
        assert_eq!(AlertScopeKey::monitor_id_of("interface:12"), None);
        assert_eq!(AlertScopeKey::monitor_id_of("monitor:abc"), None);
    }
}
