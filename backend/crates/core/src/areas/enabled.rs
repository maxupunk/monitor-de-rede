//! O formato gravado do estado das áreas (`system_settings`, chave `areas`).
//!
//! Não é contrato HTTP: a tela recebe [`super::AreasSnapshot`].

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::Area;

/// Quais áreas opcionais estão ligadas. O núcleo está sempre.
///
/// Área ausente no JSON vale **ligada**: banco novo, chave apagada ou área
/// criada depois — em todos os casos o comportamento é o de antes de existirem
/// áreas, sem nada sumir da tela em silêncio.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EnabledAreas(BTreeMap<Area, bool>);

impl EnabledAreas {
    #[must_use]
    pub fn all_on() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn is_enabled(&self, area: Area) -> bool {
        !area.is_optional() || self.0.get(&area).copied().unwrap_or(true)
    }

    pub fn set(&mut self, area: Area, enabled: bool) {
        if area.is_optional() {
            self.0.insert(area, enabled);
        }
    }

    /// As áreas ligadas, núcleo incluído.
    #[must_use]
    pub fn enabled(&self) -> Vec<Area> {
        Area::ALL
            .into_iter()
            .filter(|area| self.is_enabled(*area))
            .collect()
    }
}
