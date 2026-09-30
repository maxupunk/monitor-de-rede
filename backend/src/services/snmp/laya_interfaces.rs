//! O Laya indicando o uplink e as interfaces que valem monitorar.
//!
//! Consulta pontual, disparada quando a tela mostra a lista de interfaces
//! (fila interativa: alguém espera). Não grava nada — quem aplica é o
//! operador, no clique.

use loco_rs::app::AppContext;
use serde_json::json;

use crate::{
    dtos::snmp::{InterfaceSuggestions, InterfaceSuggestionsInput},
    services::ai::laya::{
        config::LayaFeature,
        decisions::interfaces::{rank_candidates, InterfaceAdvice},
        runtime::{Lane, LayaRuntime},
    },
};

pub async fn suggest(ctx: &AppContext, input: &InterfaceSuggestionsInput) -> InterfaceSuggestions {
    let runtime = LayaRuntime::from_context(ctx);
    let Some(settings) = runtime.settings_for(&ctx.db, LayaFeature::Interfaces).await else {
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
    let advice = InterfaceAdvice {
        ranked,
        min_confidence: settings.min_confidence,
    };
    let Some(outcome) = runtime
        .decide_with(&settings, Lane::OnDemand, &state, &advice)
        .await
    else {
        return InterfaceSuggestions::default();
    };
    InterfaceSuggestions {
        available: true,
        uplink: outcome.value.uplink.map(|mut uplink| {
            uplink.model.clone_from(&outcome.model);
            uplink
        }),
        monitor: outcome.value.monitor,
        model: Some(outcome.model),
    }
}
