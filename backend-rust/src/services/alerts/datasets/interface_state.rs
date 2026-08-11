//! Estado de uma interface SNMP traduzido para o vocabulário das regras (§8.7).
//!
//! Só publica fatos: a decisão de alertar — e com qual severidade — pertence às
//! regras cadastradas em "Regras Configuradas".

use serde_json::{json, Value};

use crate::services::{
    alerts::{
        contracts::AlertDataset,
        fields::{self, interface_speed_transition, interface_status_transition},
    },
    monitoring::link_speed::{format_speed, normalize_speed},
};

/// O que mudou numa interface entre a coleta anterior e a atual.
#[derive(Debug, Clone)]
pub struct InterfaceFacts<'a> {
    pub name: &'a str,
    pub oper_status: Option<&'a str>,
    pub speed: Option<i64>,
    pub previous_oper_status: Option<&'a str>,
    pub previous_speed: Option<i64>,
}

/// Monta os fatos da interface, incluindo as transições do ciclo.
#[must_use]
pub fn build(facts: &InterfaceFacts<'_>) -> AlertDataset {
    let mut dataset = AlertDataset::new();
    dataset.insert(fields::INTERFACE_NAME.into(), json!(facts.name));
    dataset.insert(
        fields::INTERFACE_OPER_STATUS.into(),
        facts.oper_status.map_or(Value::Null, |value| json!(value)),
    );

    if let (Some(previous), Some(current)) = (facts.previous_oper_status, facts.oper_status) {
        if previous != current {
            dataset.insert(
                fields::INTERFACE_STATUS_TRANSITION.into(),
                json!(format!("{previous}_to_{current}")),
            );
            dataset.insert(
                fields::INTERFACE_PREVIOUS_OPER_STATUS.into(),
                json!(previous),
            );
        }
    }

    let current = normalize_speed(facts.speed);
    let previous = normalize_speed(facts.previous_speed);
    if let Some(value) = current {
        dataset.insert(fields::INTERFACE_SPEED_BPS.into(), json!(value));
    }
    if let Some(value) = previous {
        dataset.insert(fields::INTERFACE_PREVIOUS_SPEED_BPS.into(), json!(value));
    }

    // Compara pela velocidade **formatada**: variações irrelevantes de leitura
    // não são renegociação de link, e alertar nelas geraria ruído diário.
    if let (Some(current), Some(previous)) = (current, previous) {
        if current != previous && format_speed(Some(previous)) != format_speed(Some(current)) {
            let is_downgrade = current < previous;
            dataset.insert(
                fields::INTERFACE_SPEED_TRANSITION.into(),
                json!(if is_downgrade {
                    interface_speed_transition::DOWNGRADE
                } else {
                    interface_speed_transition::UPGRADE
                }),
            );
            if is_downgrade {
                #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
                let percent =
                    (((previous - current) as f64 / previous as f64) * 100.0).round() as i64;
                dataset.insert(fields::INTERFACE_SPEED_DROP_PERCENT.into(), json!(percent));
            }
        }
    }

    dataset
}

/// `true` quando o dataset descreve alguma transição (queda, retorno ou
/// renegociação). Só nesses casos vale publicar no feed em tempo real.
#[must_use]
pub fn has_transition(dataset: &AlertDataset) -> bool {
    dataset.contains_key(fields::INTERFACE_STATUS_TRANSITION)
        || dataset.contains_key(fields::INTERFACE_SPEED_TRANSITION)
}

/// `true` quando a interface melhorou no ciclo (voltou a operar ou renegociou
/// para cima). Sinaliza ao motor que os alertas abertos podem ser normalizados.
#[must_use]
pub fn is_recovery(dataset: &AlertDataset) -> bool {
    dataset.get(fields::INTERFACE_STATUS_TRANSITION)
        == Some(&json!(interface_status_transition::CAME_BACK))
        || dataset.get(fields::INTERFACE_SPEED_TRANSITION)
            == Some(&json!(interface_speed_transition::UPGRADE))
}

/// Frase legível do que foi observado, usada como mensagem do alerta.
#[must_use]
pub fn describe(dataset: &AlertDataset) -> String {
    let name = dataset
        .get(fields::INTERFACE_NAME)
        .and_then(Value::as_str)
        .unwrap_or("desconhecida");
    let mut parts: Vec<String> = Vec::new();

    if dataset.contains_key(fields::INTERFACE_STATUS_TRANSITION) {
        let previous = text_of(dataset, fields::INTERFACE_PREVIOUS_OPER_STATUS).to_uppercase();
        let current = text_of(dataset, fields::INTERFACE_OPER_STATUS).to_uppercase();
        parts.push(format!(
            "Interface {name} alterou status: {previous} ➔ {current}"
        ));
    }

    if let Some(transition) = dataset
        .get(fields::INTERFACE_SPEED_TRANSITION)
        .and_then(Value::as_str)
    {
        let previous = format_speed(speed_of(dataset, fields::INTERFACE_PREVIOUS_SPEED_BPS));
        let current = format_speed(speed_of(dataset, fields::INTERFACE_SPEED_BPS));
        parts.push(if transition == interface_speed_transition::DOWNGRADE {
            format!("Interface {name} sofreu downgrade de velocidade: {previous} ➔ {current}")
        } else {
            format!("Interface {name} renegociou velocidade: {previous} ➔ {current}")
        });
    }

    if parts.is_empty() {
        let status = dataset
            .get(fields::INTERFACE_OPER_STATUS)
            .and_then(Value::as_str)
            .unwrap_or("desconhecido")
            .to_uppercase();
        let speed = format_speed(speed_of(dataset, fields::INTERFACE_SPEED_BPS));
        parts.push(format!("Interface {name} em {status} negociada a {speed}"));
    }

    parts.join(" | ")
}

fn text_of<'a>(dataset: &'a AlertDataset, key: &str) -> &'a str {
    dataset.get(key).and_then(Value::as_str).unwrap_or("")
}

fn speed_of(dataset: &AlertDataset, key: &str) -> Option<i64> {
    dataset.get(key).and_then(Value::as_i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts<'a>(
        oper: Option<&'a str>,
        previous_oper: Option<&'a str>,
        speed: Option<i64>,
        previous_speed: Option<i64>,
    ) -> InterfaceFacts<'a> {
        InterfaceFacts {
            name: "Gi0/1",
            oper_status: oper,
            speed,
            previous_oper_status: previous_oper,
            previous_speed,
        }
    }

    #[test]
    fn queda_de_link_vira_transicao_up_to_down() {
        let dataset = build(&facts(Some("down"), Some("up"), None, None));
        assert_eq!(
            dataset[fields::INTERFACE_STATUS_TRANSITION],
            json!(interface_status_transition::WENT_DOWN)
        );
        assert!(has_transition(&dataset));
        assert!(!is_recovery(&dataset));
        assert_eq!(
            describe(&dataset),
            "Interface Gi0/1 alterou status: UP ➔ DOWN"
        );
    }

    #[test]
    fn retorno_de_link_conta_como_recuperacao() {
        let dataset = build(&facts(Some("up"), Some("down"), None, None));
        assert!(is_recovery(&dataset));
    }

    #[test]
    fn primeira_coleta_nao_produz_transicao() {
        let dataset = build(&facts(Some("up"), None, Some(1_000_000_000), None));
        assert!(!has_transition(&dataset));
        assert_eq!(
            describe(&dataset),
            "Interface Gi0/1 em UP negociada a 1 Gbps"
        );
    }

    #[test]
    fn downgrade_registra_o_percentual_de_queda() {
        let dataset = build(&facts(
            Some("up"),
            Some("up"),
            Some(100_000_000),
            Some(1_000_000_000),
        ));
        assert_eq!(
            dataset[fields::INTERFACE_SPEED_TRANSITION],
            json!(interface_speed_transition::DOWNGRADE)
        );
        assert_eq!(dataset[fields::INTERFACE_SPEED_DROP_PERCENT], json!(90));
        assert_eq!(
            describe(&dataset),
            "Interface Gi0/1 sofreu downgrade de velocidade: 1 Gbps ➔ 100 Mbps"
        );
    }

    #[test]
    fn upgrade_nao_registra_percentual_e_conta_como_recuperacao() {
        let dataset = build(&facts(
            Some("up"),
            Some("up"),
            Some(1_000_000_000),
            Some(100_000_000),
        ));
        assert_eq!(
            dataset[fields::INTERFACE_SPEED_TRANSITION],
            json!(interface_speed_transition::UPGRADE)
        );
        assert!(!dataset.contains_key(fields::INTERFACE_SPEED_DROP_PERCENT));
        assert!(is_recovery(&dataset));
    }

    #[test]
    fn ruido_de_leitura_nao_vira_renegociacao() {
        // As duas leituras formatam como "1.3 Gbps": não houve renegociação.
        let dataset = build(&facts(
            Some("up"),
            Some("up"),
            Some(1_250_000_000),
            Some(1_260_000_000),
        ));
        assert!(!dataset.contains_key(fields::INTERFACE_SPEED_TRANSITION));
    }

    #[test]
    fn leitura_saturada_de_32_bits_nao_produz_falso_downgrade() {
        // Matriz de paridade #18: o teto do contador não é velocidade.
        let dataset = build(&facts(
            Some("up"),
            Some("up"),
            Some(1_000_000_000),
            Some(crate::services::monitoring::link_speed::IF_SPEED_SATURATED),
        ));
        assert!(!dataset.contains_key(fields::INTERFACE_SPEED_TRANSITION));
    }
}
