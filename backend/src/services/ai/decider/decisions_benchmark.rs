//! O resultado de cada modelo no benchmark das **decisões do NetMonitor**.
//!
//! Fonte: o Laya-tuning (`laya-tuning netmonitor-benchmark`), que manda ao
//! Ollama as perguntas reais das seis decisões — ferramentas do chat,
//! identidade de dispositivo, categoria de log, triagem de alerta, revisão de
//! ação e interfaces —, montadas como o backend as monta, e confere contra
//! rótulos curados à mão. Todos os modelos nos mesmos casos e na mesma CPU
//! (`decisions_benchmark.json`, gerado lá e copiado para cá).
//!
//! É o primeiro critério da recomendação e da ordem da lista: mede o que o
//! NetMonitor pergunta, em português, no hardware de uma instalação comum. O
//! teste público ([`super::benchmark`]) fica para quem não foi medido aqui.

use std::sync::LazyLock;

use serde::Deserialize;

use super::catalog;
use crate::dtos::ai::{
    DeciderBenchmarkReport, DeciderBenchmarkRow, DeciderBenchmarkTask, DeciderBenchmarkTaskScore,
    DeciderDecisionsScoreView,
};

/// Nome de cada decisão na tela, na ordem em que aparece.
const TASK_LABELS: &[(&str, &str)] = &[
    ("tool_routing", "Ferramentas do chat"),
    ("device_identity", "Identificação de dispositivos"),
    ("log_category", "Categoria de log"),
    ("incident_triage", "Triagem de alertas"),
    ("action_review", "Revisão de ações"),
    ("interfaces", "Interfaces e uplink"),
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub cases: u32,
    pub measured_on: String,
    pub hardware: String,
    pub timeout_ms: u32,
    pub models: Vec<ModelResult>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelResult {
    pub model: String,
    pub overall: f64,
    pub overall_low: f64,
    pub overall_high: f64,
    pub screen: f64,
    pub tasks: std::collections::BTreeMap<String, f64>,
    /// `None` nos medidos só na GPU: o acerto vale, o tempo não.
    pub p50_ms: Option<f64>,
    pub p95_ms: Option<f64>,
    pub over_timeout: Option<f64>,
}

static SNAPSHOT: LazyLock<Option<Snapshot>> =
    LazyLock::new(|| serde_json::from_str(include_str!("decisions_benchmark.json")).ok());

/// O snapshot embutido; `None` só se o JSON não abrir (um teste garante que abre).
#[must_use]
pub fn snapshot() -> Option<&'static Snapshot> {
    SNAPSHOT.as_ref()
}

/// O resultado de um modelo, pelo nome ou por um apelido do catálogo.
#[must_use]
pub fn result(model: &str) -> Option<&'static ModelResult> {
    let models = &snapshot()?.models;
    match catalog::spec(model) {
        Some(spec) => models
            .iter()
            .find(|row| catalog::spec(&row.model).is_some_and(|other| other.name == spec.name)),
        None => models.iter().find(|row| row.model == model),
    }
}

impl ModelResult {
    /// Nota geral em milésimos, na escala do teste público.
    #[must_use]
    pub fn permille(&self) -> u16 {
        // 0–1 vira 0–1000: cabe com folga num u16.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let permille = (self.overall.clamp(0.0, 1.0) * 1_000.0).round() as u16;
        permille
    }

    #[must_use]
    pub fn score_view(&self) -> DeciderDecisionsScoreView {
        DeciderDecisionsScoreView {
            overall: self.overall,
            p95_ms: self.p95_ms,
        }
    }

    fn row(&self) -> DeciderBenchmarkRow {
        let spec = catalog::spec(&self.model);
        DeciderBenchmarkRow {
            model: spec.map_or_else(|| self.model.clone(), |spec| spec.name.to_string()),
            family: spec.map(|spec| spec.family.to_string()),
            description: spec.map(|spec| spec.description.to_string()),
            recommended: spec.is_some_and(|spec| spec.recommended),
            gpu_recommended: spec.is_some_and(catalog::ModelSpec::gpu_recommended),
            bundled: spec.is_some_and(|spec| spec.bundled),
            overall: self.overall,
            overall_low: self.overall_low,
            overall_high: self.overall_high,
            screen: self.screen,
            tasks: TASK_LABELS
                .iter()
                .filter_map(|(task, _)| {
                    self.tasks
                        .get(*task)
                        .map(|score| DeciderBenchmarkTaskScore {
                            task: (*task).to_string(),
                            score: *score,
                        })
                })
                .collect(),
            p50_ms: self.p50_ms,
            p95_ms: self.p95_ms,
            over_timeout: self.over_timeout,
        }
    }
}

/// A página de benchmark: todos os modelos medidos, do mais preciso ao menos.
#[must_use]
pub fn report() -> Option<DeciderBenchmarkReport> {
    let snapshot = snapshot()?;
    let mut models: Vec<DeciderBenchmarkRow> =
        snapshot.models.iter().map(ModelResult::row).collect();
    models.sort_by(|a, b| b.overall.total_cmp(&a.overall));
    Some(DeciderBenchmarkReport {
        measured_on: snapshot.measured_on.clone(),
        hardware: snapshot.hardware.clone(),
        cases: snapshot.cases,
        timeout_ms: snapshot.timeout_ms,
        tasks: TASK_LABELS
            .iter()
            .map(|(id, label)| DeciderBenchmarkTask {
                id: (*id).to_string(),
                label: (*label).to_string(),
            })
            .collect(),
        models,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::ai::decider::config::{DEFAULT_DECIDER_MODEL, DEFAULT_DECIDER_TIMEOUT_MS};

    #[test]
    fn o_snapshot_embutido_abre_e_cobre_as_seis_decisoes() {
        let snapshot = snapshot().expect("decisions_benchmark.json inválido");
        assert!(snapshot.cases > 0 && !snapshot.models.is_empty());
        for row in &snapshot.models {
            assert!((0.0..=1.0).contains(&row.overall), "{}", row.model);
            assert!(row.overall_low <= row.overall && row.overall <= row.overall_high);
            for (task, _) in TASK_LABELS {
                assert!(row.tasks.contains_key(*task), "{} sem {task}", row.model);
            }
        }
    }

    #[test]
    fn todo_modelo_medido_esta_no_catalogo() {
        for row in &snapshot().expect("snapshot").models {
            assert!(
                catalog::spec(&row.model).is_some(),
                "{} fora do catálogo",
                row.model
            );
        }
    }

    #[test]
    fn o_padrao_lidera_e_cabe_no_tempo_maximo() {
        let report = report().expect("relatório");
        let first = &report.models[0];
        assert_eq!(first.model, DEFAULT_DECIDER_MODEL);
        assert!(first.recommended && first.bundled);
        assert!(first
            .p95_ms
            .is_some_and(|p95| p95 <= f64::from(DEFAULT_DECIDER_TIMEOUT_MS)));
        assert_eq!(report.tasks.len(), first.tasks.len());
    }

    #[test]
    fn acha_pelo_apelido() {
        let by_alias = result("laya-network").expect("pelo apelido");
        assert_eq!(by_alias.model, DEFAULT_DECIDER_MODEL);
        assert!(result("modelo-que-nao-existe").is_none());
    }
}
