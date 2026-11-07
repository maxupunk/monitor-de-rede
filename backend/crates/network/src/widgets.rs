//! Cards do dashboard da área Rede (`WidgetSpec`, ADR 017).

use netmonitor_core::{
    areas::Area,
    dashboard::{
        DashboardWidgetSize::{Full, Half},
        DashboardWidgetType, DefaultPlacement,
        WidgetCategory::{Charts, Lists},
        WidgetSpec,
    },
};

#[must_use]
pub fn widgets() -> Vec<WidgetSpec> {
    let base = WidgetSpec {
        widget_type: DashboardWidgetType::new(""),
        area: Area::Network,
        order: 0,
        title: "",
        description: "",
        category: Charts,
        icon: "",
        multiple: true,
        default_size: Half,
        config_keys: &[],
        compatible_resources: &[],
        default_placement: DefaultPlacement::Absent,
    };
    vec![
        WidgetSpec {
            widget_type: DashboardWidgetType::new("dns_latency"),
            order: 100,
            title: "Tempo de Consulta e Benchmark DNS",
            description: "Ranking e histórico do tempo de consulta de resolvedores DNS.",
            icon: "mdi-dns-outline",
            config_keys: &["dnsServerIds", "timeframe"],
            compatible_resources: &["dns-resolvers", "numeric"],
            default_placement: DefaultPlacement::Hidden,
            ..base
        },
        WidgetSpec {
            widget_type: DashboardWidgetType::new("saas_services"),
            order: 110,
            title: "Serviços SaaS, Bancos & Nuvem",
            description:
                "Latência e qualidade de experiência de bancos, fintechs e provedores SaaS.",
            category: Lists,
            icon: "mdi-bank-check",
            multiple: false,
            default_size: Full,
            compatible_resources: &["numeric"],
            default_placement: DefaultPlacement::Hidden,
            ..base
        },
        WidgetSpec {
            widget_type: DashboardWidgetType::new("saas_heatmap"),
            order: 120,
            title: "Mapa de Calor de Latência SaaS",
            description: "Matriz de latência por hora do dia de um serviço SaaS.",
            icon: "mdi-chart-scatter-plot-hexbin",
            default_size: Full,
            config_keys: &["monitorId"],
            compatible_resources: &["numeric"],
            default_placement: DefaultPlacement::Hidden,
            ..base
        },
        WidgetSpec {
            widget_type: DashboardWidgetType::new("ether_bandwidth"),
            order: 130,
            title: "Consumo de Banda de Ether",
            description: "Tráfego de entrada e saída (Rx/Tx) de uma interface de rede.",
            icon: "mdi-swap-horizontal-bold",
            config_keys: &["deviceId", "interfaceId", "interfaceName", "timeframe"],
            compatible_resources: &["bandwidth"],
            ..base
        },
        WidgetSpec {
            widget_type: DashboardWidgetType::new("bandwidth_vs_latency"),
            order: 140,
            title: "Consumo de Banda vs Latência",
            description:
                "Eixo duplo: tráfego (Mbps) de um dispositivo contra a latência de um monitor.",
            icon: "mdi-chart-multiaxis",
            default_size: Full,
            config_keys: &["deviceId", "interfaceId", "monitorId", "timeframe"],
            compatible_resources: &["dual-axis", "bandwidth", "numeric"],
            ..base
        },
    ]
}
