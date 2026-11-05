//! Classificação assistida (ADR 017, ponto de extensão `Classifier`).
//!
//! A identificação de dispositivo (descoberta e cadastro), a sugestão de
//! interfaces do SNMP e a categoria dos padrões de log têm heurística própria,
//! que continua decidindo. Um [`Classifier`] só dá o **palpite** onde a
//! heurística ficou em dúvida — e a tela mostra, quem decide é o operador.
//!
//! Quem chama depende só deste trait. O padrão é a [`HeuristicClassifier`]:
//! nenhuma frente ligada, nenhum palpite — fica a heurística de quem chama. O
//! assistente registra a implementação pelo Decisor no ponto de montagem
//! (`src/registries.rs`, via `after_context`), e assim vale também no
//! `scheduler` e nas `task`s.

use std::sync::Arc;

use async_trait::async_trait;
use loco_rs::app::AppContext;

pub mod device_identity;
pub mod interfaces;
pub mod log_category;
pub mod suggestion;

use device_identity::{DeviceFacts, DeviceIdentity, IdentitySuggestion};
use interfaces::{InterfaceAdviceOutput, InterfaceCandidate};
use log_category::LogPattern;
use suggestion::DeciderSuggestion;

/// Quem espera pela resposta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lane {
    /// Alguém pediu esta resposta na tela (testar, sugerir interfaces):
    /// espera o modelo carregar, com a tela avisando pelo SSE.
    OnDemand,
    /// Alguém na tela: passa direto, sem fila nem disjuntor.
    Interactive,
    /// Rotina: um por vez e respeitando o disjuntor.
    Background,
}

/// As frentes de classificação fora do chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClassifierFeature {
    DeviceIdentity,
    Interfaces,
    LogEvents,
}

/// Quem dá o palpite. Toda resposta é opcional: `None` significa "fica a
/// heurística" — frente desligada, classificador fora do ar ou sem confiança.
#[async_trait]
pub trait Classifier: Send + Sync {
    /// A confiança mínima (0–100) da frente; `None` quando ela está desligada.
    async fn min_confidence(&self, ctx: &AppContext, feature: ClassifierFeature) -> Option<u8>;

    /// Tipo e sistema do aparelho, com o modelo que respondeu carimbado.
    async fn identify_device(
        &self,
        ctx: &AppContext,
        lane: Lane,
        facts: &DeviceFacts,
        question: &DeviceIdentity,
    ) -> Option<IdentitySuggestion>;

    /// O uplink e as interfaces que valem monitorar, com o modelo que
    /// respondeu. `state` é o aparelho; as interfaces vão em `ranked`.
    async fn advise_interfaces(
        &self,
        ctx: &AppContext,
        lane: Lane,
        state: &str,
        ranked: &[&InterfaceCandidate],
        min_confidence: u8,
    ) -> Option<(InterfaceAdviceOutput, String)>;

    /// A categoria de um padrão de log, com o modelo que respondeu.
    async fn categorize_log(
        &self,
        ctx: &AppContext,
        lane: Lane,
        pattern: &LogPattern<'_>,
        min_confidence: u8,
    ) -> Option<DeciderSuggestion>;
}

/// O padrão sem assistente: nenhuma frente ligada, nenhum palpite. A
/// identificação e a classificação seguem pela heurística de quem chama.
#[derive(Debug, Clone, Copy, Default)]
pub struct HeuristicClassifier;

#[async_trait]
impl Classifier for HeuristicClassifier {
    async fn min_confidence(&self, _ctx: &AppContext, _feature: ClassifierFeature) -> Option<u8> {
        None
    }

    async fn identify_device(
        &self,
        _ctx: &AppContext,
        _lane: Lane,
        _facts: &DeviceFacts,
        _question: &DeviceIdentity,
    ) -> Option<IdentitySuggestion> {
        None
    }

    async fn advise_interfaces(
        &self,
        _ctx: &AppContext,
        _lane: Lane,
        _state: &str,
        _ranked: &[&InterfaceCandidate],
        _min_confidence: u8,
    ) -> Option<(InterfaceAdviceOutput, String)> {
        None
    }

    async fn categorize_log(
        &self,
        _ctx: &AppContext,
        _lane: Lane,
        _pattern: &LogPattern<'_>,
        _min_confidence: u8,
    ) -> Option<DeciderSuggestion> {
        None
    }
}

/// O classificador registrado no `shared_store`.
#[derive(Clone)]
struct Registered(Arc<dyn Classifier>);

/// Registra o classificador do processo (o ponto de montagem chama).
pub fn install(ctx: &AppContext, classifier: Arc<dyn Classifier>) {
    ctx.shared_store.insert(Registered(classifier));
}

/// Tira o classificador registrado: o processo volta à heurística. É o
/// estado de um processo montado sem o assistente.
pub fn uninstall(ctx: &AppContext) {
    let _ = ctx.shared_store.remove::<Registered>();
}

/// O classificador do processo; sem nenhum registrado, a heurística.
#[must_use]
pub fn from_context(ctx: &AppContext) -> Arc<dyn Classifier> {
    ctx.shared_store.get::<Registered>().map_or_else(
        || Arc::new(HeuristicClassifier) as Arc<dyn Classifier>,
        |registered| registered.0,
    )
}
