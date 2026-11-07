//! Cards do dashboard da área Servidores (`WidgetSpec`, ADR 017).

use netmonitor_core::{
    areas::Area,
    dashboard::{
        DashboardWidgetSize, DashboardWidgetType, DefaultPlacement, WidgetCategory, WidgetSpec,
    },
};

/// O resumo dos containers: os dados vêm da store do Docker na tela.
pub const DOCKER_SUMMARY: DashboardWidgetType = DashboardWidgetType::new("docker_summary");

#[must_use]
pub fn widgets() -> Vec<WidgetSpec> {
    vec![WidgetSpec {
        widget_type: DOCKER_SUMMARY,
        area: Area::Servers,
        order: 90,
        title: "Resumo Docker",
        description: "Resumo dos containers dos hosts Docker (estado e consumo).",
        category: WidgetCategory::Summary,
        icon: "mdi-docker",
        multiple: false,
        default_size: DashboardWidgetSize::Full,
        config_keys: &[],
        compatible_resources: &[],
        default_placement: DefaultPlacement::Visible,
    }]
}
