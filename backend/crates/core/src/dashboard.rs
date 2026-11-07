//! Cards do dashboard como ponto de extensão (ADR 017, `WidgetSpec`).
//!
//! Cada área declara os próprios cards ([`WidgetSpec`]) e o ponto de montagem
//! junta tudo num catálogo único com [`install_widgets`], uma vez por
//! processo — o mesmo arranjo das ferramentas da IA. O núcleo lê o catálogo
//! sem saber de que área veio cada card: o `area` do spec é que decide se o
//! card aparece com a área desligada.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize, Serializer};
use ts_rs::TS;

use crate::areas::Area;

/// Tipo de card: o id que a área declara. É o `type` gravado no layout e,
/// nos cards únicos, também o `id` da instância.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, TS)]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DashboardWidgetType(#[ts(type = "string")] &'static str);

impl DashboardWidgetType {
    /// Só para quem declara o card (o spec da área).
    #[must_use]
    pub const fn new(id: &'static str) -> Self {
        Self(id)
    }

    #[must_use]
    pub const fn id(self) -> &'static str {
        self.0
    }

    /// O spec do tipo no catálogo do processo.
    #[must_use]
    pub fn from_id(id: &str) -> Option<&'static WidgetSpec> {
        widgets().iter().find(|spec| spec.widget_type.id() == id)
    }
}

impl Serialize for DashboardWidgetType {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.0)
    }
}

/// Largura de um card. Vira as colunas do Vuetify gravadas no layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum DashboardWidgetSize {
    Third,
    Half,
    TwoThirds,
    Full,
}

/// Colunas do Vuetify por quebra de tela, como o layout as grava.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridColumns {
    pub cols: i64,
    pub sm: i64,
    pub md: i64,
    pub lg: i64,
}

impl DashboardWidgetSize {
    pub const ALL: [Self; 4] = [Self::Third, Self::Half, Self::TwoThirds, Self::Full];

    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Third => "third",
            Self::Half => "half",
            Self::TwoThirds => "two_thirds",
            Self::Full => "full",
        }
    }

    /// Como o usuário lê, na confirmação.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Third => "um terço da largura",
            Self::Half => "metade da largura",
            Self::TwoThirds => "dois terços da largura",
            Self::Full => "largura total",
        }
    }

    #[must_use]
    pub const fn columns(self) -> GridColumns {
        let (md, lg) = match self {
            Self::Third => (6, 4),
            Self::Half => (6, 6),
            Self::TwoThirds => (12, 8),
            Self::Full => (12, 12),
        };
        GridColumns {
            cols: 12,
            sm: 12,
            md,
            lg,
        }
    }

    /// O tamanho de colunas já gravadas. Vale o `lg` (a tela grande, onde a
    /// diferença aparece); sem ele, o `md`; sem nenhum, a tela usa 12 — largura
    /// total. Valores fora da tabela caem no tamanho mais próximo por cima.
    #[must_use]
    pub fn from_columns(lg: Option<i64>, md: Option<i64>) -> Self {
        match (lg, md) {
            (Some(lg), _) if lg > 0 => match lg {
                ..=4 => Self::Third,
                5..=6 => Self::Half,
                7..=8 => Self::TwoThirds,
                _ => Self::Full,
            },
            (_, Some(md)) if md > 0 && md <= 6 => Self::Half,
            _ => Self::Full,
        }
    }
}

/// Grupo do card na tela.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetCategory {
    Summary,
    Lists,
    Charts,
}

impl WidgetCategory {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Summary => "summary",
            Self::Lists => "lists",
            Self::Charts => "charts",
        }
    }
}

/// O card no layout de fábrica.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultPlacement {
    /// Só pelo catálogo ("Adicionar card").
    Absent,
    Visible,
    /// Nasce no layout desligado: a um clique na edição.
    Hidden,
}

/// O que uma área declara de um tipo de card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WidgetSpec {
    pub widget_type: DashboardWidgetType,
    pub area: Area,
    /// Posição entre os cards de todas as áreas: a ordem do catálogo, da IA e
    /// do layout de fábrica.
    pub order: u16,
    pub title: &'static str,
    /// Para a IA escolher o card certo.
    pub description: &'static str,
    pub category: WidgetCategory,
    pub icon: &'static str,
    /// Mais de uma instância (cada uma com a sua configuração).
    pub multiple: bool,
    pub default_size: DashboardWidgetSize,
    /// Chaves de `config` que o card usa.
    pub config_keys: &'static [&'static str],
    /// Que tipo de recurso o card sabe desenhar — o diálogo "Adicionar card"
    /// usa para oferecer só o que serve. Vazio nos cards sem configuração.
    pub compatible_resources: &'static [&'static str],
    pub default_placement: DefaultPlacement,
}

static WIDGETS: OnceLock<Vec<WidgetSpec>> = OnceLock::new();

/// Instala o catálogo do processo (ordenado por [`WidgetSpec::order`]). Só o
/// primeiro vale: o ponto de montagem chama uma vez por processo.
pub fn install_widgets(mut widgets: Vec<WidgetSpec>) {
    widgets.sort_by_key(|spec| spec.order);
    let _ = WIDGETS.set(widgets);
}

/// O catálogo instalado; vazio sem instalação.
#[must_use]
pub fn widgets() -> &'static [WidgetSpec] {
    WIDGETS.get().map_or(&[], Vec::as_slice)
}

/// Os cards do layout de fábrica com a visibilidade de cada um: primeiro os
/// visíveis, depois os ocultos, cada grupo na ordem do catálogo.
#[must_use]
pub fn default_layout() -> Vec<(&'static WidgetSpec, bool)> {
    let pick = |placement: DefaultPlacement| {
        widgets()
            .iter()
            .filter(move |spec| spec.default_placement == placement)
    };
    pick(DefaultPlacement::Visible)
        .map(|spec| (spec, true))
        .chain(pick(DefaultPlacement::Hidden).map(|spec| (spec, false)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tamanho_vira_colunas_do_vuetify() {
        let colunas = |size: DashboardWidgetSize| {
            let c = size.columns();
            (c.cols, c.sm, c.md, c.lg)
        };
        assert_eq!(colunas(DashboardWidgetSize::Third), (12, 12, 6, 4));
        assert_eq!(colunas(DashboardWidgetSize::Half), (12, 12, 6, 6));
        assert_eq!(colunas(DashboardWidgetSize::TwoThirds), (12, 12, 12, 8));
        assert_eq!(colunas(DashboardWidgetSize::Full), (12, 12, 12, 12));
    }

    #[test]
    fn colunas_gravadas_voltam_ao_tamanho() {
        for size in DashboardWidgetSize::ALL {
            let c = size.columns();
            assert_eq!(
                DashboardWidgetSize::from_columns(Some(c.lg), Some(c.md)),
                size
            );
        }
        assert_eq!(
            DashboardWidgetSize::from_columns(None, Some(6)),
            DashboardWidgetSize::Half
        );
        assert_eq!(
            DashboardWidgetSize::from_columns(None, None),
            DashboardWidgetSize::Full,
            "sem colunas a tela usa 12"
        );
        assert_eq!(
            DashboardWidgetSize::from_columns(Some(3), None),
            DashboardWidgetSize::Third
        );
    }

    #[test]
    fn tipo_e_tamanho_serializam_no_texto_do_layout() {
        assert_eq!(
            serde_json::to_value(DashboardWidgetType::new("cpu_usage")).unwrap(),
            "cpu_usage"
        );
        assert_eq!(
            serde_json::to_value(DashboardWidgetSize::TwoThirds).unwrap(),
            "two_thirds"
        );
    }
}
