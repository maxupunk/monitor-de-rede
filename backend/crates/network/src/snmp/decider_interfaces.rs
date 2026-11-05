//! O Decisor indicando o uplink e as interfaces que valem monitorar.
//!
//! Consulta pontual, disparada quando a tela mostra a lista de interfaces
//! (fila interativa: alguém espera). Não grava nada — quem aplica é o
//! operador, no clique.

use loco_rs::app::AppContext;
use serde_json::json;

use netmonitor_core::classifier::{self, interfaces::rank_candidates, ClassifierFeature, Lane};

use crate::dtos::snmp::{InterfaceSuggestions, InterfaceSuggestionsInput};

pub async fn suggest(ctx: &AppContext, input: &InterfaceSuggestionsInput) -> InterfaceSuggestions {
    let classifier = classifier::from_context(ctx);
    let Some(min_confidence) = classifier
        .min_confidence(ctx, ClassifierFeature::Interfaces)
        .await
    else {
        return InterfaceSuggestions::default();
    };
    let ranked = rank_candidates(&input.interfaces);
    if ranked.is_empty() {
        return InterfaceSuggestions::default();
    }
    // O estado é o aparelho; o texto de cada interface vai nas perguntas.
    let state = json!({
        "device": input.device_name,
        "deviceType": input.device_type,
        "sysDescr": input.sys_descr,
        "interfaceCount": input.interfaces.len(),
    })
    .to_string();
    let Some((advice, model)) = classifier
        .advise_interfaces(ctx, Lane::OnDemand, &state, &ranked, min_confidence)
        .await
    else {
        return InterfaceSuggestions::default();
    };
    InterfaceSuggestions {
        available: true,
        uplink: advice.uplink.map(|mut uplink| {
            uplink.model.clone_from(&model);
            uplink
        }),
        monitor: advice.monitor,
        model: Some(model),
    }
}
