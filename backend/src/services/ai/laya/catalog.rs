//! Modelos do Laya para a tela escolher: os conhecidos, com o que cada um é,
//! e os que o Ollaya já tem instalados.

use super::{client::LayaClient, config::AiLayaSettings};
use crate::{
    dtos::ai::{LayaModelOption, LayaModelsResponse},
    services::ai::local_models,
};

/// Modelos publicados no Ollaya, o recomendado primeiro.
pub const KNOWN_MODELS: &[(&str, &str)] = &[
    (
        "laya",
        "Roteador (recomendado): responde com laya:en ou laya:multilingual conforme a língua da pergunta",
    ),
    (
        "laya:multilingual",
        "mmBERT 322M, mais de 100 línguas — o que de fato responde perguntas em português",
    ),
    ("laya:en", "ModernBERT-large 421M, só inglês, o mais rápido"),
    (
        "laya:typed-decisions",
        "ModernBERT-large ajustado para decisões tipadas (inglês)",
    ),
];

/// Nunca falha: fora do ar vira `online: false` com o motivo.
pub async fn list(settings: &AiLayaSettings) -> LayaModelsResponse {
    let (installed, error_message) = match LayaClient::new(settings).installed_models().await {
        Ok(installed) => (Some(installed), None),
        Err(error) => (None, Some(error.to_string())),
    };
    let online = installed.is_some();
    // Na memória agora; falha aqui (Ollaya antigo) só deixa a lista vazia.
    let loaded = if online {
        LayaClient::new(settings)
            .loaded_models()
            .await
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let installed = installed.unwrap_or_default();

    let mut options: Vec<LayaModelOption> = KNOWN_MODELS
        .iter()
        .map(|&(name, description)| LayaModelOption {
            name: name.to_string(),
            description: Some(description.to_string()),
            installed: local_models::is_installed(&installed, name),
        })
        .collect();
    // Instalado fora do catálogo (um modelo próprio, uma tag nova) também aparece.
    for name in &installed {
        if !options
            .iter()
            .any(|option| option.name.eq_ignore_ascii_case(name))
        {
            options.push(LayaModelOption {
                name: name.clone(),
                description: None,
                installed: true,
            });
        }
    }

    LayaModelsResponse {
        online,
        error_message,
        installed,
        loaded,
        options,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fora_do_ar_ainda_oferece_o_catalogo() {
        let settings = AiLayaSettings {
            base_url: "http://127.0.0.1:9".into(),
            timeout_ms: 500,
            ..AiLayaSettings::default()
        };
        let models = list(&settings).await;
        assert!(!models.online);
        assert!(models.error_message.is_some());
        assert_eq!(models.options.len(), KNOWN_MODELS.len());
        assert!(models.options.iter().all(|option| !option.installed));
    }
}
