//! O [`Classifier`] pelo Decisor: o palpite onde a heurística do núcleo ficou
//! em dúvida (identificação de dispositivo, interfaces do SNMP, categoria de
//! padrão de log). O ponto de montagem o registra; sem ele, vale a
//! heurística (`netmonitor_core::classifier::HeuristicClassifier`).
//!
//! Cada consulta lê a frente na configuração do Decisor (em cache no
//! [`DeciderRuntime`]) e devolve `None` com ela desligada, fora do ar ou sem
//! confiança — exatamente o que quem chama já tratava como "fica a
//! heurística".

use async_trait::async_trait;
use loco_rs::app::AppContext;
use netmonitor_core::classifier::{
    device_identity::{DeviceFacts, DeviceIdentity, IdentitySuggestion},
    interfaces::{InterfaceAdviceOutput, InterfaceCandidate},
    log_category::LogPattern,
    suggestion::DeciderSuggestion,
    Classifier, ClassifierFeature, Lane,
};

use crate::decider::{
    config::DeciderFeature,
    decisions::{interfaces::InterfaceAdvice, log_category::LogCategory},
    runtime::DeciderRuntime,
};

const fn feature_of(feature: ClassifierFeature) -> DeciderFeature {
    match feature {
        ClassifierFeature::DeviceIdentity => DeciderFeature::DeviceIdentity,
        ClassifierFeature::Interfaces => DeciderFeature::Interfaces,
        ClassifierFeature::LogEvents => DeciderFeature::LogEvents,
    }
}

/// O classificador do assistente.
#[derive(Debug, Clone, Copy, Default)]
pub struct DeciderClassifier;

#[async_trait]
impl Classifier for DeciderClassifier {
    async fn min_confidence(&self, ctx: &AppContext, feature: ClassifierFeature) -> Option<u8> {
        DeciderRuntime::from_context(ctx)
            .settings_for(&ctx.db, feature_of(feature))
            .await
            .map(|settings| settings.min_confidence)
    }

    async fn identify_device(
        &self,
        ctx: &AppContext,
        lane: Lane,
        facts: &DeviceFacts,
        question: &DeviceIdentity,
    ) -> Option<IdentitySuggestion> {
        let runtime = DeciderRuntime::from_context(ctx);
        let settings = runtime
            .settings_for(&ctx.db, DeciderFeature::DeviceIdentity)
            .await?;
        let outcome = runtime
            .decide_with(&settings, lane, &facts.state(), question)
            .await?;
        Some(outcome.value.answered_by(&outcome.model))
    }

    async fn advise_interfaces(
        &self,
        ctx: &AppContext,
        lane: Lane,
        state: &str,
        ranked: &[&InterfaceCandidate],
        min_confidence: u8,
    ) -> Option<(InterfaceAdviceOutput, String)> {
        let runtime = DeciderRuntime::from_context(ctx);
        let settings = runtime
            .settings_for(&ctx.db, DeciderFeature::Interfaces)
            .await?;
        let advice = InterfaceAdvice {
            ranked: ranked.to_vec(),
            min_confidence,
        };
        let outcome = runtime.decide_with(&settings, lane, state, &advice).await?;
        Some((outcome.value, outcome.model))
    }

    async fn categorize_log(
        &self,
        ctx: &AppContext,
        lane: Lane,
        pattern: &LogPattern<'_>,
        min_confidence: u8,
    ) -> Option<DeciderSuggestion> {
        let runtime = DeciderRuntime::from_context(ctx);
        let settings = runtime
            .settings_for(&ctx.db, DeciderFeature::LogEvents)
            .await?;
        let decision = LogCategory { min_confidence };
        let outcome = runtime
            .decide_with(&settings, lane, &pattern.state(), &decision)
            .await?;
        outcome.value.map(|mut suggestion| {
            suggestion.model = outcome.model;
            suggestion
        })
    }
}
