//! Quanto a IA pode fazer sozinha — o modo que a política das ferramentas
//! ([`super::tools::ToolPolicy`]) aplica a cada família.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Quanto a IA pode fazer sozinha numa família de ferramentas (ações,
/// containers).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum AiAutomationMode {
    /// A IA não usa essas ferramentas.
    Off,
    /// A IA propõe e o usuário confirma no chat, sempre.
    #[default]
    Confirm,
    /// Automático com critério: o que é de risco baixo ou médio roda na hora
    /// (aparece no chat e na auditoria); o de risco alto — ou o que o Decisor
    /// julgar perigoso no contexto — volta a pedir confirmação
    /// (`harness::tools::risk`).
    Auto,
}
