//! O que cada área sabe sobre si mesma.

use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::Area;
use crate::shared::errors::AppResult;

/// Sinal persistido de que a instalação usa a área.
///
/// Nunca é estado volátil (online/offline): a queda de um agente não pode
/// fazer uma área "sumir" da evidência no meio de um incidente. E nunca conta
/// o que o próprio sistema cria sozinho — o agente local, o dispositivo do
/// sistema, os DNS padrão.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct Evidence {
    pub present: bool,
    /// Por que há evidência, em uma frase ("2 redes IP cadastradas").
    pub reason: Option<String>,
}

impl Evidence {
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn because(reason: impl Into<String>) -> Self {
        Self {
            present: true,
            reason: Some(reason.into()),
        }
    }
}

/// Efeito concreto de desligar a área, para a confirmação na tela.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct Consequence {
    pub message: String,
}

impl Consequence {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// O que impede desligar a área agora, e como resolver.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct Blocker {
    pub message: String,
}

/// O conhecimento de uma área sobre si mesma. Uma implementação por área
/// opcional, registrada pelo ponto de montagem.
#[async_trait]
pub trait AreaPolicy: Send + Sync {
    fn area(&self) -> Area;

    /// A instalação usa esta área? Serve às sugestões ("Encontramos um host
    /// Docker. Mostrar a área Servidores?"); nunca liga nem desliga sozinha.
    async fn evidence(&self, db: &DatabaseConnection) -> AppResult<Evidence>;

    /// O que muda ao desligar. Lembre que monitores **nunca** param pela área.
    async fn consequences(&self, db: &DatabaseConnection) -> AppResult<Vec<Consequence>>;

    /// Algo impede desligar agora? `None` libera.
    async fn blocker(&self, _db: &DatabaseConnection) -> AppResult<Option<Blocker>> {
        Ok(None)
    }
}

/// Monitores nunca param pela área — a confirmação diz quantos seguem rodando.
#[must_use]
pub fn monitors_keep_running(count: u64, singular: &str, plural_form: &str) -> Option<Consequence> {
    (count > 0).then(|| {
        Consequence::new(format!(
            "{} continua{} rodando e alertando: monitor nunca para pela área.",
            crate::shared::text::count_noun(count, singular, plural_form),
            if count == 1 { "" } else { "m" }
        ))
    })
}
