//! Catálogo de cards do dashboard: o que existe, quantos de cada e o tamanho.
//!
//! É a fonte que a IA lê para propor mudanças e que as operações de
//! [`super::layout`] usam para validar o tipo pedido. Os textos de título,
//! categoria e ícone seguem os da tela (`stores/dashboard.ts`), para que um
//! card criado aqui apareça igual a um criado pelo catálogo da tela.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Tipo de card do dashboard. O texto em `snake_case` é o `type` gravado no
/// layout e, nos cards únicos, também o `id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum DashboardWidgetType {
    StatCards,
    HealthGauge,
    ActiveAlerts,
    LatencyTimeSeries,
    EventDistribution,
    UnstableTargets,
    EventsFeed,
    NetworkMonitors,
    DockerSummary,
    DnsLatency,
    SaasServices,
    SaasHeatmap,
    EtherBandwidth,
    BandwidthVsLatency,
    CpuUsage,
    RamUsage,
    BinaryStatus,
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
        match self {
            Self::Third => GridColumns {
                cols: 12,
                sm: 12,
                md: 6,
                lg: 4,
            },
            Self::Half => GridColumns {
                cols: 12,
                sm: 12,
                md: 6,
                lg: 6,
            },
            Self::TwoThirds => GridColumns {
                cols: 12,
                sm: 12,
                md: 12,
                lg: 8,
            },
            Self::Full => GridColumns {
                cols: 12,
                sm: 12,
                md: 12,
                lg: 12,
            },
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

/// O que o catálogo sabe de um tipo de card.
#[derive(Debug, Clone, Copy)]
pub struct WidgetSpec {
    pub widget_type: DashboardWidgetType,
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
}

impl DashboardWidgetType {
    pub const ALL: [Self; 17] = [
        Self::StatCards,
        Self::HealthGauge,
        Self::ActiveAlerts,
        Self::LatencyTimeSeries,
        Self::EventDistribution,
        Self::UnstableTargets,
        Self::EventsFeed,
        Self::NetworkMonitors,
        Self::DockerSummary,
        Self::DnsLatency,
        Self::SaasServices,
        Self::SaasHeatmap,
        Self::EtherBandwidth,
        Self::BandwidthVsLatency,
        Self::CpuUsage,
        Self::RamUsage,
        Self::BinaryStatus,
    ];

    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::StatCards => "stat_cards",
            Self::HealthGauge => "health_gauge",
            Self::ActiveAlerts => "active_alerts",
            Self::LatencyTimeSeries => "latency_time_series",
            Self::EventDistribution => "event_distribution",
            Self::UnstableTargets => "unstable_targets",
            Self::EventsFeed => "events_feed",
            Self::NetworkMonitors => "network_monitors",
            Self::DockerSummary => "docker_summary",
            Self::DnsLatency => "dns_latency",
            Self::SaasServices => "saas_services",
            Self::SaasHeatmap => "saas_heatmap",
            Self::EtherBandwidth => "ether_bandwidth",
            Self::BandwidthVsLatency => "bandwidth_vs_latency",
            Self::CpuUsage => "cpu_usage",
            Self::RamUsage => "ram_usage",
            Self::BinaryStatus => "binary_status",
        }
    }

    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.id() == id)
    }

    #[must_use]
    pub const fn allows_multiple(self) -> bool {
        self.spec().multiple
    }

    #[allow(clippy::too_many_lines)]
    #[must_use]
    pub const fn spec(self) -> WidgetSpec {
        use DashboardWidgetSize::{Full, Half};
        use WidgetCategory::{Charts, Lists, Summary};
        let (title, description, category, icon, multiple, default_size, config_keys): (
            &'static str,
            &'static str,
            WidgetCategory,
            &'static str,
            bool,
            DashboardWidgetSize,
            &'static [&'static str],
        ) = match self {
            Self::StatCards => (
                "Cards de Resumo Estatístico",
                "Números do topo: dispositivos, monitores, disponibilidade e alertas.",
                Summary,
                "mdi-view-dashboard-outline",
                false,
                Full,
                &[],
            ),
            Self::HealthGauge => (
                "Saúde Global & Status dos Ativos",
                "Rosca com a disponibilidade global e a distribuição dos status dos monitores.",
                Charts,
                "mdi-gauge",
                false,
                Half,
                &[],
            ),
            Self::ActiveAlerts => (
                "Alertas Críticos Ativos",
                "Lista dos alertas ativos com ações (reconhecer, silenciar, verificar).",
                Lists,
                "mdi-bell-outline",
                false,
                Half,
                &[],
            ),
            Self::LatencyTimeSeries => (
                "Latência & Perda de Pacotes",
                "Série temporal de latência e perda dos monitores.",
                Charts,
                "mdi-chart-timeline-variant",
                false,
                Half,
                &["timeframe"],
            ),
            Self::EventDistribution => (
                "Distribuição de Eventos por Hora",
                "Histograma por hora dos eventos, por severidade.",
                Charts,
                "mdi-chart-bar",
                false,
                Half,
                &["timeframe"],
            ),
            Self::UnstableTargets => (
                "Alvos Instáveis",
                "Ranking dos alvos que mais caíram e voltaram (oscilação).",
                Lists,
                "mdi-sine-wave",
                false,
                Half,
                &[],
            ),
            Self::EventsFeed => (
                "Feed de Eventos Realtime",
                "Fluxo em tempo real dos eventos e mudanças de status.",
                Lists,
                "mdi-pulse",
                false,
                Half,
                &[],
            ),
            Self::NetworkMonitors => (
                "Monitores de Rede",
                "Lista de monitores com barras de histórico.",
                Lists,
                "mdi-chart-timeline-variant",
                false,
                Full,
                &[],
            ),
            Self::DockerSummary => (
                "Resumo Docker",
                "Resumo dos containers dos hosts Docker (estado e consumo).",
                Summary,
                "mdi-docker",
                false,
                Full,
                &[],
            ),
            Self::DnsLatency => (
                "Tempo de Consulta e Benchmark DNS",
                "Ranking e histórico do tempo de consulta de resolvedores DNS.",
                Charts,
                "mdi-dns-outline",
                true,
                Half,
                &["dnsServerIds", "timeframe"],
            ),
            Self::SaasServices => (
                "Serviços SaaS, Bancos & Nuvem",
                "Latência e qualidade de experiência de bancos, fintechs e provedores SaaS.",
                Lists,
                "mdi-bank-check",
                false,
                Full,
                &[],
            ),
            Self::SaasHeatmap => (
                "Mapa de Calor de Latência SaaS",
                "Matriz de latência por hora do dia de um serviço SaaS.",
                Charts,
                "mdi-chart-scatter-plot-hexbin",
                true,
                Full,
                &["monitorId"],
            ),
            Self::EtherBandwidth => (
                "Consumo de Banda de Ether",
                "Tráfego de entrada e saída (Rx/Tx) de uma interface de rede.",
                Charts,
                "mdi-swap-horizontal-bold",
                true,
                Half,
                &["deviceId", "interfaceId", "interfaceName", "timeframe"],
            ),
            Self::BandwidthVsLatency => (
                "Consumo de Banda vs Latência",
                "Eixo duplo: tráfego (Mbps) de um dispositivo contra a latência de um monitor.",
                Charts,
                "mdi-chart-multiaxis",
                true,
                Full,
                &["deviceId", "interfaceId", "monitorId", "timeframe"],
            ),
            Self::CpuUsage => (
                "Uso de CPU",
                "Uso de CPU (%) de um dispositivo com limites de aviso e crítico.",
                Charts,
                "mdi-cpu-64-bit",
                true,
                Half,
                &[
                    "deviceId",
                    "warningThreshold",
                    "criticalThreshold",
                    "timeframe",
                ],
            ),
            Self::RamUsage => (
                "Uso de RAM",
                "Memória RAM usada de um dispositivo com limites de aviso e crítico.",
                Charts,
                "mdi-memory",
                true,
                Half,
                &[
                    "deviceId",
                    "warningThreshold",
                    "criticalThreshold",
                    "timeframe",
                ],
            ),
            Self::BinaryStatus => (
                "Status Binário (Up/Down)",
                "Estado ligado/desligado de um monitor (online/offline, link up/down).",
                Lists,
                "mdi-checkbox-blank-circle-outline",
                true,
                Half,
                &["monitorId"],
            ),
        };
        WidgetSpec {
            widget_type: self,
            title,
            description,
            category,
            icon,
            multiple,
            default_size,
            config_keys,
        }
    }
}

/// O layout que vale quando nada foi salvo, na ordem da tela.
pub const DEFAULT_LAYOUT: [(DashboardWidgetType, DashboardWidgetSize); 9] = [
    (DashboardWidgetType::StatCards, DashboardWidgetSize::Full),
    (DashboardWidgetType::HealthGauge, DashboardWidgetSize::Half),
    (DashboardWidgetType::ActiveAlerts, DashboardWidgetSize::Half),
    (
        DashboardWidgetType::LatencyTimeSeries,
        DashboardWidgetSize::Half,
    ),
    (
        DashboardWidgetType::EventDistribution,
        DashboardWidgetSize::Half,
    ),
    (
        DashboardWidgetType::UnstableTargets,
        DashboardWidgetSize::Half,
    ),
    (DashboardWidgetType::EventsFeed, DashboardWidgetSize::Half),
    (
        DashboardWidgetType::NetworkMonitors,
        DashboardWidgetSize::Full,
    ),
    (
        DashboardWidgetType::DockerSummary,
        DashboardWidgetSize::Full,
    ),
];

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
    fn tipos_serializam_no_texto_do_layout() {
        for kind in DashboardWidgetType::ALL {
            assert_eq!(serde_json::to_value(kind).unwrap(), kind.id());
            assert_eq!(DashboardWidgetType::from_id(kind.id()), Some(kind));
            assert_eq!(kind.spec().widget_type, kind);
        }
        assert_eq!(
            serde_json::to_value(DashboardWidgetSize::TwoThirds).unwrap(),
            "two_thirds"
        );
    }

    #[test]
    fn so_os_cards_configuraveis_aceitam_varias_instancias() {
        let multiplos: Vec<&str> = DashboardWidgetType::ALL
            .into_iter()
            .filter(|kind| kind.allows_multiple())
            .map(DashboardWidgetType::id)
            .collect();
        assert_eq!(
            multiplos,
            vec![
                "dns_latency",
                "saas_heatmap",
                "ether_bandwidth",
                "bandwidth_vs_latency",
                "cpu_usage",
                "ram_usage",
                "binary_status"
            ]
        );
    }
}
