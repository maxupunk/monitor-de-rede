//! Teste do Laya pela tela de configurações: o servidor responde? O modelo
//! está instalado? O que ele decide para uma pergunta de exemplo?
//!
//! Devolve sempre um diagnóstico, nunca erro — cada falha vira uma mensagem
//! que diz o que fazer.

use super::{client::LayaClient, config::AiLayaSettings, tool_routing};
use crate::{
    dtos::ai::{LayaGroupScore, TestLayaResponse},
    services::ai::{harness::tools::ToolGroup, local_models},
};

/// Pergunta usada quando a tela não manda uma.
pub const SAMPLE_QUESTION: &str =
    "Por que o link da filial ficou lento ontem à noite? Mostra um gráfico da latência.";

fn failure(online: bool, model_installed: bool, message: String) -> TestLayaResponse {
    TestLayaResponse {
        success: false,
        online,
        model_installed,
        latency_ms: 0.0,
        message,
        model: None,
        groups: Vec::new(),
    }
}

pub async fn test(settings: &AiLayaSettings, question: Option<&str>) -> TestLayaResponse {
    let question = question
        .map(str::trim)
        .filter(|question| !question.is_empty())
        .unwrap_or(SAMPLE_QUESTION);

    let installed = match LayaClient::new(settings).installed_models().await {
        Ok(installed) => installed,
        Err(error) => return failure(false, false, error.to_string()),
    };
    if !local_models::is_installed(&installed, &settings.model) {
        return failure(
            true,
            false,
            format!(
                "O Ollaya respondeu, mas o modelo '{}' não está instalado. Use \"Baixar modelo\".",
                settings.model
            ),
        );
    }

    match tool_routing::score(settings, question, &ToolGroup::DEFERRED).await {
        Ok(outcome) => {
            let threshold = settings.threshold();
            let groups: Vec<LayaGroupScore> = outcome
                .value
                .0
                .iter()
                .map(|(group, probability)| LayaGroupScore {
                    id: group.id().to_string(),
                    purpose: group.purpose().to_string(),
                    probability: *probability,
                    selected: *probability >= threshold,
                })
                .collect();
            let chosen = groups.iter().filter(|group| group.selected).count();
            TestLayaResponse {
                success: true,
                online: true,
                model_installed: true,
                latency_ms: outcome.latency_ms,
                message: format!(
                    "{chosen} de {} grupos de ferramentas acima de {}%.",
                    groups.len(),
                    settings.min_confidence
                ),
                model: Some(outcome.model),
                groups,
            }
        }
        Err(error) => failure(true, true, error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn servidor_fora_do_ar_vira_diagnostico() {
        let settings = AiLayaSettings {
            base_url: "http://127.0.0.1:9".into(),
            timeout_ms: 500,
            ..AiLayaSettings::default()
        };
        let result = test(&settings, None).await;
        assert!(!result.success);
        assert!(!result.online);
        assert!(result.message.contains("127.0.0.1:9"));
    }
}
