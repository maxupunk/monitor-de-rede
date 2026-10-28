//! Um cliente Wi-Fi vigiado traduzido para o vocabulário das regras.
//!
//! Só publica fatos: onde o aparelho está e como está o sinal. Quanto tempo
//! fora é grave, e com qual severidade, quem diz são as regras
//! (`wifi_watched_offline`, `wifi_watched_poor_signal`).

use serde_json::json;

use super::format_seconds;
use crate::{
    dtos::wifi::WifiHealth,
    services::alerts::{
        contracts::AlertDataset,
        fields::{self, wifi_client_status},
    },
};

/// Onde o aparelho foi visto nesta leitura.
#[derive(Debug, Clone, PartialEq)]
pub enum WifiClientPresence {
    Connected {
        device_name: String,
        signal_dbm: i32,
        health: WifiHealth,
        health_reason: Option<String>,
    },
    /// Leitura completa de todos os roteadores e o aparelho em nenhum.
    Offline { seconds: i64 },
}

/// O que se sabe do aparelho neste ciclo.
#[derive(Debug, Clone, Copy)]
pub struct WifiClientFacts<'a> {
    pub name: &'a str,
    pub presence: &'a WifiClientPresence,
}

#[must_use]
pub fn build(facts: &WifiClientFacts<'_>) -> AlertDataset {
    let mut dataset = AlertDataset::new();
    dataset.insert(fields::WIFI_CLIENT_NAME.into(), json!(facts.name));
    match facts.presence {
        WifiClientPresence::Connected {
            signal_dbm, health, ..
        } => {
            dataset.insert(
                fields::WIFI_CLIENT_STATUS.into(),
                json!(wifi_client_status::CONNECTED),
            );
            dataset.insert(fields::WIFI_CLIENT_SIGNAL_DBM.into(), json!(signal_dbm));
            dataset.insert(fields::WIFI_CLIENT_HEALTH.into(), json!(health));
        }
        WifiClientPresence::Offline { seconds } => {
            dataset.insert(
                fields::WIFI_CLIENT_STATUS.into(),
                json!(wifi_client_status::OFFLINE),
            );
            dataset.insert(fields::WIFI_CLIENT_OFFLINE_SECONDS.into(), json!(seconds));
        }
    }
    dataset
}

/// Conectado e sem problema de sinal: os alertas abertos podem fechar.
#[must_use]
pub const fn is_recovery(presence: &WifiClientPresence) -> bool {
    matches!(
        presence,
        WifiClientPresence::Connected { health, .. }
            if !matches!(health, WifiHealth::Critical | WifiHealth::HighRetries)
    )
}

/// Frase legível do que foi observado, usada como mensagem do alerta.
#[must_use]
pub fn describe(facts: &WifiClientFacts<'_>) -> String {
    let name = facts.name;
    match facts.presence {
        WifiClientPresence::Offline { seconds } => {
            format!(
                "{name} está fora do Wi-Fi há {}: não aparece em nenhum roteador.",
                format_seconds(*seconds)
            )
        }
        WifiClientPresence::Connected {
            device_name,
            health_reason: Some(reason),
            ..
        } => format!("{name} em {device_name}: {reason}."),
        WifiClientPresence::Connected {
            device_name,
            signal_dbm,
            ..
        } => format!("{name} conectado em {device_name}, sinal de {signal_dbm} dBm."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conectado(health: WifiHealth, reason: Option<&str>) -> WifiClientPresence {
        WifiClientPresence::Connected {
            device_name: "AP Portaria".into(),
            signal_dbm: -82,
            health,
            health_reason: reason.map(str::to_owned),
        }
    }

    #[test]
    fn fora_do_wifi_publica_ha_quanto_tempo() {
        let presence = WifiClientPresence::Offline { seconds: 720 };
        let facts = WifiClientFacts {
            name: "Câmera Portaria",
            presence: &presence,
        };
        let dataset = build(&facts);
        assert_eq!(dataset[fields::WIFI_CLIENT_STATUS], json!("offline"));
        assert_eq!(dataset[fields::WIFI_CLIENT_OFFLINE_SECONDS], json!(720));
        assert!(!dataset.contains_key(fields::WIFI_CLIENT_SIGNAL_DBM));
        assert!(!is_recovery(&presence));
        assert_eq!(
            describe(&facts),
            "Câmera Portaria está fora do Wi-Fi há 12 min: não aparece em nenhum roteador."
        );
    }

    #[test]
    fn conectado_publica_sinal_e_saude_sem_tempo_fora() {
        let presence = conectado(WifiHealth::Critical, Some("Sinal fraco: -82 dBm"));
        let facts = WifiClientFacts {
            name: "Câmera Portaria",
            presence: &presence,
        };
        let dataset = build(&facts);
        assert_eq!(dataset[fields::WIFI_CLIENT_HEALTH], json!("critical"));
        assert_eq!(dataset[fields::WIFI_CLIENT_SIGNAL_DBM], json!(-82));
        assert!(!dataset.contains_key(fields::WIFI_CLIENT_OFFLINE_SECONDS));
        assert_eq!(
            describe(&facts),
            "Câmera Portaria em AP Portaria: Sinal fraco: -82 dBm."
        );
    }

    #[test]
    fn so_conectado_sem_problema_de_sinal_e_recuperacao() {
        assert!(is_recovery(&conectado(WifiHealth::Healthy, None)));
        assert!(is_recovery(&conectado(WifiHealth::PowerSave, None)));
        assert!(is_recovery(&conectado(WifiHealth::Fair, None)));
        assert!(!is_recovery(&conectado(WifiHealth::Critical, None)));
        assert!(!is_recovery(&conectado(WifiHealth::HighRetries, None)));
    }
}
