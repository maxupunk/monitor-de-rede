//! Entrada das rotas de áreas do produto (ADR 017).

use serde::Deserialize;
use ts_rs::TS;

/// Corpo do `PUT /api/areas/{area}`.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AreaToggleInput {
    pub enabled: bool,
}

/// Corpo do `POST /api/settings/onboarding/intent`: o que a instalação vai
/// gerenciar, escolhido na primeira etapa do assistente.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct OnboardingIntentInput {
    pub network: bool,
    pub servers: bool,
    pub vpn: bool,
}
