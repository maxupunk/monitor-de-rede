//! Gráfico produzido por uma ferramenta da IA (`ToolOutput::chart`) e
//! desenhado no chat com o mesmo componente das telas de monitor e interface.

use serde::Serialize;
use ts_rs::TS;

/// Grandeza do eixo Y de um gráfico da IA — o frontend escolhe o formatador por ela.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum AiChartUnit {
    Latency,
    Bandwidth,
    Percentage,
    Generic,
}

/// O que o eixo X representa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum AiChartAxis {
    /// `time` de cada ponto é um instante RFC 3339 — a tela formata a data.
    Time,
    /// `time` é um rótulo pronto ("14h") — a tela mostra como veio.
    Label,
}

/// Amostra de uma série: instante RFC 3339 (ou rótulo, conforme o eixo) e
/// valor na unidade do gráfico.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AiChartPoint {
    pub time: String,
    #[ts(type = "number")]
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AiChartSeries {
    pub id: String,
    pub label: String,
    pub points: Vec<AiChartPoint>,
}

/// Gráfico produzido por uma ferramenta da IA e desenhado no chat com o
/// mesmo componente das telas de monitor e interface.
///
/// Os pontos vão só para a tela; a IA recebe o resumo estatístico.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AiChart {
    pub title: String,
    pub subtitle: Option<String>,
    pub unit: AiChartUnit,
    pub x_axis: AiChartAxis,
    pub series: Vec<AiChartSeries>,
    #[ts(type = "number | null")]
    pub avg_value: Option<f64>,
}
