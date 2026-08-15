//! Contratos da avaliação de alertas (§8.7).
//!
//! Quem observa a rede (monitor, coleta SNMP, telemetria de túnel) só produz
//! *fatos*; quem decide se aquilo vira alerta — e com qual severidade — são as
//! regras cadastradas. Este contrato é a fronteira entre os dois lados: nenhum
//! produtor de fatos precisa conhecer `alert_events`, notificação ou severidade.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
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
    /// avaliação, os alertas abertos do escopo entram em observação de
    /// estabilidade ou resolvem, conforme a janela de cada regra.
    pub recovered: bool,

    /// `true` quando o alvo respondeu degradado (`warning`: perda parcial de
    /// pacotes, DNS parcial). Não dispara regra, mas conta como problema para
    /// a janela de recuperação dos eventos abertos do escopo (Fase 2).
    pub degraded: bool,
}

/// Ciclo de vida de `alert_events` (§2.2 da análise de monitoramento
/// inteligente).
///
/// A coluna no banco continua string — o enum existe para que cada `match`
/// obrigue o tratamento de todos os estados: acrescentar um status sem
/// atualizar um ponto de decisão vira erro de compilação, não bug silencioso.
/// `Recovering` é o estado da histerese de resolução (Fase 1): o alvo voltou,
/// mas a janela da regra ainda não se esgotou — uma recaída o devolve a
/// `Active`. `Flapping` é o estado da Fase 3: o alvo recaiu tantas vezes dentro
/// da janela de flap da regra que deixou de ser "um problema" e virou
/// "cronicamente instável" — o episódio segue aberto, sem notificar a cada
/// oscilação, até a contagem deslizante decair e a estabilidade voltar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AlertStatus {
    Active,
    Acknowledged,
    Silenced,
    Recovering,
    Flapping,
    Resolved,
}

impl AlertStatus {
    /// Os status que contam como alerta ainda aberto.
    ///
    /// Reconhecer, silenciar, estabilizar ou oscilar não fecha o alerta — só
    /// muda como ele aparece. Por isso os cinco aparecem juntos tanto na
    /// deduplicação quanto na recuperação.
    pub const OPEN: [Self; 5] = [
        Self::Active,
        Self::Acknowledged,
        Self::Silenced,
        Self::Recovering,
        Self::Flapping,
    ];

    /// Forma persistida na coluna `alert_events.status`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Acknowledged => "acknowledged",
            Self::Silenced => "silenced",
            Self::Recovering => "recovering",
            Self::Flapping => "flapping",
            Self::Resolved => "resolved",
        }
    }

    /// Aberto = ainda conta para a deduplicação e para a recuperação
    /// automática.
    #[must_use]
    pub const fn is_open(self) -> bool {
        matches!(
            self,
            Self::Active | Self::Acknowledged | Self::Silenced | Self::Recovering | Self::Flapping
        )
    }
}

impl fmt::Display for AlertStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for AlertStatus {
    /// Status gravado por fora do motor (linha antiga, escrita manual) não
    /// pode derrubar a leitura: quem chama decide o fallback do `Err`.
    type Err = ();

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        match raw {
            "active" => Ok(Self::Active),
            "acknowledged" => Ok(Self::Acknowledged),
            "silenced" => Ok(Self::Silenced),
            "recovering" => Ok(Self::Recovering),
            "flapping" => Ok(Self::Flapping),
            "resolved" => Ok(Self::Resolved),
            _ => Err(()),
        }
    }
}

/// Permite `Column::Status.is_in(AlertStatus::OPEN)` direto no `sea-orm`.
impl From<AlertStatus> for sea_orm::Value {
    fn from(status: AlertStatus) -> Self {
        Self::String(Some(status.as_str().to_owned()))
    }
}

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

    #[test]
    fn status_faz_aida_e_volta_pela_string_persistida() {
        for status in AlertStatus::OPEN {
            assert_eq!(status.as_str().parse::<AlertStatus>(), Ok(status));
            assert!(status.is_open());
        }
        assert_eq!(
            AlertStatus::Resolved.as_str().parse::<AlertStatus>(),
            Ok(AlertStatus::Resolved)
        );
        assert!(!AlertStatus::Resolved.is_open());
        assert!("triggered".parse::<AlertStatus>().is_err());
        assert_eq!(AlertStatus::Recovering.to_string(), "recovering");
        // Flapping é aberto como os demais: entra na dedup e na recuperação.
        assert_eq!(AlertStatus::Flapping.to_string(), "flapping");
        assert!(AlertStatus::OPEN.contains(&AlertStatus::Flapping));
    }

    #[test]
    fn status_serializa_como_a_string_da_coluna() {
        assert_eq!(
            serde_json::to_value(AlertStatus::Recovering).unwrap(),
            serde_json::json!("recovering")
        );
        assert_eq!(
            serde_json::from_value::<AlertStatus>(serde_json::json!("silenced")).unwrap(),
            AlertStatus::Silenced
        );
    }
}
