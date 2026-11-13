//! O roaming de um aparelho traduzido para o vocabulário das regras.
//!
//! Só publica fatos: quantas trocas de AP na janela e o veredito (ping-pong
//! ou estável) pelo limite das Configurações de Wi-Fi. Se isso vira alerta,
//! e com qual severidade, quem diz é a regra (`wifi_roaming_ping_pong`).

use serde_json::json;

use netmonitor_core::alerts::{
    contracts::{AlertDataset, AlertFacts},
    fields::{self, wifi_roam_status},
};

/// O que se sabe do roaming do aparelho agora.
#[derive(Debug, Clone, Copy)]
pub struct WifiRoamingFacts<'a> {
    /// Como chamar o aparelho.
    pub label: &'a str,
    /// Trocas dentro da janela.
    pub count: u64,
    pub window_min: u32,
    pub ping_pong: bool,
    /// A última troca: de qual AP para qual.
    pub last: Option<(&'a str, &'a str)>,
}

#[must_use]
pub fn build(facts: &WifiRoamingFacts<'_>) -> AlertFacts {
    let mut dataset = AlertFacts::new();
    let status = if facts.ping_pong {
        wifi_roam_status::PING_PONG
    } else {
        wifi_roam_status::STABLE
    };
    dataset.insert(fields::WIFI_ROAM_STATUS.into(), json!(status));
    dataset.insert(fields::WIFI_ROAM_COUNT.into(), json!(facts.count));
    dataset
}

/// Frase legível do que foi observado, usada como mensagem do alerta.
#[must_use]
pub fn describe(facts: &WifiRoamingFacts<'_>) -> String {
    let WifiRoamingFacts {
        label,
        count,
        window_min,
        ..
    } = *facts;
    let trocas = if count == 1 { "vez" } else { "vezes" };
    match facts.last {
        Some((from, to)) => format!(
            "{label} trocou de AP {count} {trocas} em {window_min} min (a última de {from} para {to})."
        ),
        None => format!("{label} trocou de AP {count} {trocas} em {window_min} min."),
    }
}

impl AlertDataset for WifiRoamingFacts<'_> {
    fn event_type(&self) -> &'static str {
        "wifi_roaming"
    }

    fn facts(&self) -> AlertFacts {
        build(self)
    }

    fn describe(&self) -> String {
        describe(self)
    }

    fn is_recovery(&self) -> bool {
        !self.ping_pong
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_pong_publica_contagem_e_veredito() {
        let facts = WifiRoamingFacts {
            label: "iPhone-de-Joao",
            count: 5,
            window_min: 10,
            ping_pong: true,
            last: Some(("AP Sala", "AP Cozinha")),
        };
        let dataset = build(&facts);
        assert_eq!(dataset[fields::WIFI_ROAM_STATUS], json!("ping_pong"));
        assert_eq!(dataset[fields::WIFI_ROAM_COUNT], json!(5));
        assert!(!facts.is_recovery());
        assert_eq!(
            describe(&facts),
            "iPhone-de-Joao trocou de AP 5 vezes em 10 min (a última de AP Sala para AP Cozinha)."
        );
    }

    #[test]
    fn abaixo_do_limite_e_estavel_e_recupera() {
        let facts = WifiRoamingFacts {
            label: "Notebook",
            count: 1,
            window_min: 10,
            ping_pong: false,
            last: None,
        };
        assert_eq!(build(&facts)[fields::WIFI_ROAM_STATUS], json!("stable"));
        assert!(facts.is_recovery());
        assert_eq!(describe(&facts), "Notebook trocou de AP 1 vez em 10 min.");
    }
}
