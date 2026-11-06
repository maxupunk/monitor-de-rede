//! Catálogo de cards do dashboard: o que existe, quantos de cada e o tamanho.
//!
//! Cada área declara os próprios cards (`WidgetSpec`, no `core`); o ponto de
//! montagem junta todos com `install_widgets`. Aqui ficam os cards do Núcleo
//! e a visão do catálogo instalado que a tela (`GET /api/dashboard/catalog`)
//! e a IA leem — o frontend não mantém lista própria de tipos (ARQ-04).

use netmonitor_core::{
    areas::Area,
    dashboard::{
        default_layout, widgets,
        DashboardWidgetSize::{Full, Half},
        DefaultPlacement::{Absent, Visible},
        WidgetCategory::{Charts, Lists, Summary},
    },
};
use serde::Serialize;
use ts_rs::TS;

pub use netmonitor_core::dashboard::{
    DashboardWidgetSize, DashboardWidgetType, DefaultPlacement, WidgetCategory, WidgetSpec,
};

/// Os cards do Núcleo.
#[allow(clippy::too_many_lines)]
#[must_use]
pub fn core_widgets() -> Vec<WidgetSpec> {
    let base = WidgetSpec {
        widget_type: DashboardWidgetType::new(""),
        area: Area::Core,
        order: 0,
        title: "",
        description: "",
        category: Lists,
        icon: "",
        multiple: false,
        default_size: Half,
        config_keys: &[],
        compatible_resources: &[],
        default_placement: Visible,
    };
    let device_chart = |id, order, title, description, icon| WidgetSpec {
        widget_type: DashboardWidgetType::new(id),
        order,
        title,
        description,
        category: Charts,
        icon,
        multiple: true,
        config_keys: &[
            "deviceId",
            "warningThreshold",
            "criticalThreshold",
            "timeframe",
        ],
        compatible_resources: &["numeric"],
        default_placement: Absent,
        ..base
    };
    vec![
        WidgetSpec {
            widget_type: DashboardWidgetType::new("stat_cards"),
            order: 10,
            title: "Cards de Resumo Estatístico",
            description: "Números do topo: dispositivos, monitores, disponibilidade e alertas.",
            category: Summary,
            icon: "mdi-view-dashboard-outline",
            default_size: Full,
            ..base
        },
        WidgetSpec {
            widget_type: DashboardWidgetType::new("health_gauge"),
            order: 20,
            title: "Saúde Global & Status dos Ativos",
            description:
                "Rosca com a disponibilidade global e a distribuição dos status dos monitores.",
            category: Charts,
            icon: "mdi-gauge",
            ..base
        },
        WidgetSpec {
            widget_type: DashboardWidgetType::new("active_alerts"),
            order: 30,
            title: "Alertas Críticos Ativos",
            description: "Lista dos alertas ativos com ações (reconhecer, silenciar, verificar).",
            icon: "mdi-bell-outline",
            ..base
        },
        WidgetSpec {
            widget_type: DashboardWidgetType::new("latency_time_series"),
            order: 40,
            title: "Latência & Perda de Pacotes",
            description: "Série temporal de latência e perda dos monitores.",
            category: Charts,
            icon: "mdi-chart-timeline-variant",
            config_keys: &["timeframe"],
            ..base
        },
        WidgetSpec {
            widget_type: DashboardWidgetType::new("event_distribution"),
            order: 50,
            title: "Distribuição de Eventos por Hora",
            description: "Histograma por hora dos eventos, por severidade.",
            category: Charts,
            icon: "mdi-chart-bar",
            config_keys: &["timeframe"],
            ..base
        },
        WidgetSpec {
            widget_type: DashboardWidgetType::new("unstable_targets"),
            order: 60,
            title: "Alvos Instáveis",
            description: "Ranking dos alvos que mais caíram e voltaram (oscilação).",
            icon: "mdi-sine-wave",
            ..base
        },
        WidgetSpec {
            widget_type: DashboardWidgetType::new("events_feed"),
            order: 70,
            title: "Feed de Eventos Realtime",
            description: "Fluxo em tempo real dos eventos e mudanças de status.",
            icon: "mdi-pulse",
            ..base
        },
        WidgetSpec {
            widget_type: DashboardWidgetType::new("network_monitors"),
            order: 80,
            title: "Monitores de Rede",
            description: "Lista de monitores com barras de histórico.",
            icon: "mdi-chart-timeline-variant",
            default_size: Full,
            ..base
        },
        device_chart(
            "cpu_usage",
            150,
            "Uso de CPU",
            "Uso de CPU (%) de um dispositivo com limites de aviso e crítico.",
            "mdi-cpu-64-bit",
        ),
        device_chart(
            "ram_usage",
            160,
            "Uso de RAM",
            "Memória RAM usada de um dispositivo com limites de aviso e crítico.",
            "mdi-memory",
        ),
        WidgetSpec {
            widget_type: DashboardWidgetType::new("binary_status"),
            order: 170,
            title: "Status Binário (Up/Down)",
            description: "Estado ligado/desligado de um monitor (online/offline, link up/down).",
            icon: "mdi-checkbox-blank-circle-outline",
            multiple: true,
            config_keys: &["monitorId"],
            compatible_resources: &["binary"],
            default_placement: Absent,
            ..base
        },
    ]
}

/// Um tipo de card como a tela o vê — o catálogo único (ARQ-04): o frontend
/// não mantém lista própria de tipos, títulos, ícones nem tamanhos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct WidgetCatalogEntry {
    #[serde(rename = "type")]
    #[ts(rename = "type")]
    pub widget_type: DashboardWidgetType,
    pub title: String,
    pub description: String,
    pub category: String,
    pub icon: String,
    pub allow_multiple: bool,
    pub default_size: DashboardWidgetSize,
    pub config_keys: Vec<String>,
    pub compatible_resources: Vec<String>,
    pub area: Area,
}

/// Um card do layout padrão.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DefaultWidget {
    #[serde(rename = "type")]
    #[ts(rename = "type")]
    pub widget_type: DashboardWidgetType,
    pub size: DashboardWidgetSize,
    /// Os ocultos nascem no layout desligados: estão a um clique na edição.
    pub visible: bool,
}

/// Corpo do `GET /api/dashboard/catalog`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DashboardCatalog {
    pub widgets: Vec<WidgetCatalogEntry>,
    pub default_layout: Vec<DefaultWidget>,
}

impl From<&WidgetSpec> for WidgetCatalogEntry {
    fn from(spec: &WidgetSpec) -> Self {
        let owned = |items: &[&str]| items.iter().map(|item| (*item).to_owned()).collect();
        Self {
            widget_type: spec.widget_type,
            title: spec.title.to_owned(),
            description: spec.description.to_owned(),
            category: spec.category.id().to_owned(),
            icon: spec.icon.to_owned(),
            allow_multiple: spec.multiple,
            default_size: spec.default_size,
            config_keys: owned(spec.config_keys),
            compatible_resources: owned(spec.compatible_resources),
            area: spec.area,
        }
    }
}

/// O catálogo instalado, para a tela.
#[must_use]
pub fn dashboard_catalog() -> DashboardCatalog {
    DashboardCatalog {
        widgets: widgets().iter().map(WidgetCatalogEntry::from).collect(),
        default_layout: default_layout()
            .into_iter()
            .map(|(spec, visible)| DefaultWidget {
                widget_type: spec.widget_type,
                size: spec.default_size,
                visible,
            })
            .collect(),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use netmonitor_core::dashboard::install_widgets;

    use super::*;

    /// Catálogo dos testes do Núcleo: os cards dele e dois de área com o mesmo
    /// id e a mesma área dos de verdade — o Núcleo não conhece os crates das
    /// áreas, só o que eles declaram.
    pub(crate) fn install_test_catalog() {
        let mut all = core_widgets();
        let area_card = |id, title, order, area, placement| WidgetSpec {
            widget_type: DashboardWidgetType::new(id),
            area,
            order,
            title,
            description: id,
            category: Lists,
            icon: "mdi-card",
            multiple: false,
            default_size: Full,
            config_keys: &[],
            compatible_resources: &[],
            default_placement: placement,
        };
        all.push(area_card(
            "docker_summary",
            "Resumo Docker",
            90,
            Area::Servers,
            Visible,
        ));
        all.push(area_card(
            "saas_services",
            "Serviços SaaS, Bancos & Nuvem",
            110,
            Area::Network,
            DefaultPlacement::Hidden,
        ));
        install_widgets(all);
    }

    #[test]
    fn catalogo_mostra_a_area_que_cada_card_declarou() {
        install_test_catalog();
        let catalog = dashboard_catalog();
        let area_of = |id: &str| {
            catalog
                .widgets
                .iter()
                .find(|entry| entry.widget_type.id() == id)
                .map(|entry| entry.area)
        };
        assert_eq!(area_of("docker_summary"), Some(Area::Servers));
        assert_eq!(area_of("saas_services"), Some(Area::Network));
        assert_eq!(area_of("cpu_usage"), Some(Area::Core));
    }

    #[test]
    fn catalogo_segue_a_ordem_declarada_e_nao_repete_tipo() {
        install_test_catalog();
        let ordens: Vec<u16> = widgets().iter().map(|spec| spec.order).collect();
        let mut ordenadas = ordens.clone();
        ordenadas.sort_unstable();
        ordenadas.dedup();
        assert_eq!(ordens, ordenadas);
    }

    #[test]
    fn layout_padrao_tem_visiveis_antes_dos_ocultos() {
        install_test_catalog();
        let layout = dashboard_catalog().default_layout;
        let primeiro_oculto = layout.iter().position(|w| !w.visible).expect("oculto");
        assert!(layout[..primeiro_oculto].iter().all(|w| w.visible));
        assert!(layout[primeiro_oculto..].iter().all(|w| !w.visible));
        assert_eq!(layout[0].widget_type.id(), "stat_cards");
    }

    #[test]
    fn json_usa_type_como_o_layout_salvo() {
        install_test_catalog();
        let spec = DashboardWidgetType::from_id("binary_status").expect("binary_status");
        let json = serde_json::to_value(WidgetCatalogEntry::from(spec)).expect("json");
        assert_eq!(json["type"], "binary_status");
        assert_eq!(json["area"], "core");
        assert_eq!(json["compatibleResources"], serde_json::json!(["binary"]));
    }

    #[test]
    fn so_os_cards_configuraveis_do_nucleo_aceitam_varias_instancias() {
        let multiplos: Vec<&str> = core_widgets()
            .iter()
            .filter(|spec| spec.multiple)
            .map(|spec| spec.widget_type.id())
            .collect();
        assert_eq!(multiplos, vec!["cpu_usage", "ram_usage", "binary_status"]);
    }
}
