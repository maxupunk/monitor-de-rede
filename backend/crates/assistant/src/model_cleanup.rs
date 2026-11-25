//! Apagar do Ollama um modelo que ninguém usa mais, para devolver o disco.
//!
//! As regras moram aqui; o [`crate::ollama`] só faz as chamadas HTTP e o
//! controller só extrai, autoriza e audita:
//! - o modelo configurado no chat **não** sai — apagá-lo derrubaria o
//!   Assistente na próxima pergunta; troque o modelo antes;
//! - só sai o que está instalado (o nome exato do `/api/tags`);
//! - o espaço devolvido é o tamanho do modelo, a não ser que outro nome
//!   aponte para o mesmo arquivo (mesmo digest): aí apagar só remove o nome.

use crate::{
    dtos::{OllamaModelItem, OllamaModelRemoval},
    ollama,
    settings::{AiSettings, DEFAULT_OLLAMA_BASE_URL, DEFAULT_OLLAMA_MODEL},
};
use netmonitor_core::shared::errors::{AppError, AppResult};

/// O modelo que o chat usa (o salvo, ou o padrão quando nada foi salvo).
#[must_use]
pub fn configured_model(settings: &AiSettings) -> &str {
    settings
        .ollama_model
        .as_deref()
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .unwrap_or(DEFAULT_OLLAMA_MODEL)
}

/// A URL do Ollama: a da tela, ou a salva, ou a padrão do compose.
#[must_use]
pub fn resolve_base_url(settings: &AiSettings, requested: Option<String>) -> String {
    requested
        .filter(|url| !url.trim().is_empty())
        .or_else(|| settings.ollama_base_url.clone())
        .unwrap_or_else(|| DEFAULT_OLLAMA_BASE_URL.to_string())
}

/// Decide se `model` pode sair e quanto disco isso devolve. Pura: recebe o
/// que o Ollama tem instalado e o modelo do chat.
///
/// # Errors
///
/// `422` sem nome; `409` quando é o modelo do chat; `404` quando não está
/// instalado.
pub fn plan(
    configured: &str,
    installed: &[OllamaModelItem],
    model: &str,
) -> AppResult<OllamaModelRemoval> {
    let model = model.trim();
    if model.is_empty() {
        return Err(AppError::validation("Informe o modelo a apagar."));
    }
    if ollama::same_model(model, configured) {
        return Err(AppError::conflict(format!(
            "'{model}' é o modelo usado pelo Assistente. Escolha e salve outro modelo antes de \
             apagá-lo."
        )));
    }
    let target = installed
        .iter()
        .find(|item| ollama::same_model(&item.name, model))
        .ok_or_else(|| {
            AppError::not_found(format!("O modelo '{model}' não está instalado no Ollama."))
        })?;

    Ok(OllamaModelRemoval {
        name: target.name.clone(),
        size: target.size,
        freed_bytes: if target.shared_with.is_empty() {
            target.size.unwrap_or(0)
        } else {
            0
        },
        shared_with: target.shared_with.clone(),
    })
}

/// Preenche `shared_with` de cada modelo: os outros nomes com o mesmo digest
/// (o mesmo arquivo no disco). Sem digest, nada é compartilhado.
pub fn link_shared(items: &mut [OllamaModelItem]) {
    let digests: Vec<(String, Option<String>)> = items
        .iter()
        .map(|item| {
            let digest = item
                .digest
                .as_deref()
                .map(str::trim)
                .filter(|digest| !digest.is_empty())
                .map(str::to_owned);
            (item.name.clone(), digest)
        })
        .collect();
    for item in items.iter_mut() {
        let Some(own) = digests
            .iter()
            .find(|(name, _)| *name == item.name)
            .and_then(|(_, digest)| digest.as_deref())
        else {
            continue;
        };
        item.shared_with = digests
            .iter()
            .filter(|(name, digest)| *name != item.name && digest.as_deref() == Some(own))
            .map(|(name, _)| name.clone())
            .collect();
    }
}

/// Apaga `model` do Ollama de `base_url` se as regras de [`plan`] deixarem.
///
/// # Errors
///
/// Os de [`plan`]; `503` quando o Ollama não responde ou recusa.
pub async fn remove(
    settings: &AiSettings,
    base_url: Option<String>,
    model: &str,
) -> AppResult<OllamaModelRemoval> {
    let configured = configured_model(settings);
    let base_url = resolve_base_url(settings, base_url);
    let installed = ollama::installed_models(&base_url, configured).await?;
    let removal = plan(configured, &installed, model)?;
    ollama::delete_model(&base_url, &removal.name).await?;
    Ok(removal)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str, size: u64, digest: &str) -> OllamaModelItem {
        OllamaModelItem {
            name: name.into(),
            size: Some(size),
            parameter_size: None,
            modified_at: None,
            context_length: None,
            netmonitor_trained: false,
            digest: Some(digest.into()),
            in_use: false,
            shared_with: Vec::new(),
        }
    }

    fn installed() -> Vec<OllamaModelItem> {
        let mut items = vec![
            item("lfm3-netmonitor:latest", 1_800, "aaa"),
            item("lfm3-netmonitor:q8_0", 1_800, "aaa"),
            item("llama3.2:3b", 2_000, "bbb"),
            item("qwen3:latest", 5_000, "ccc"),
            item("sem-digest:1b", 900, " "),
            item("outro-sem-digest:1b", 900, " "),
        ];
        link_shared(&mut items);
        items
    }

    #[test]
    fn so_o_mesmo_digest_divide_o_arquivo() {
        let items = installed();
        let shared = |name: &str| {
            items
                .iter()
                .find(|item| item.name == name)
                .map(|item| item.shared_with.clone())
                .unwrap()
        };
        assert_eq!(shared("lfm3-netmonitor:latest"), ["lfm3-netmonitor:q8_0"]);
        assert_eq!(shared("lfm3-netmonitor:q8_0"), ["lfm3-netmonitor:latest"]);
        assert!(shared("llama3.2:3b").is_empty());
        assert!(shared("sem-digest:1b").is_empty(), "digest vazio não conta");
    }

    #[test]
    fn o_modelo_do_chat_nao_sai() {
        for pedido in [
            "lfm3-netmonitor:latest",
            "LFM3-NetMonitor",
            " lfm3-netmonitor ",
        ] {
            let erro = plan("lfm3-netmonitor:latest", &installed(), pedido).unwrap_err();
            assert_eq!(erro.status().as_u16(), 409, "{pedido}");
        }
    }

    #[test]
    fn sem_nome_e_nao_instalado_sao_recusados() {
        let erro = plan("qwen3", &installed(), "  ").unwrap_err();
        assert_eq!(erro.status().as_u16(), 422);
        let erro = plan("qwen3", &installed(), "phi4:14b").unwrap_err();
        assert_eq!(erro.status().as_u16(), 404);
    }

    #[test]
    fn modelo_sozinho_devolve_o_tamanho_todo() {
        let plano = plan("lfm3-netmonitor:latest", &installed(), "llama3.2:3b").unwrap();
        assert_eq!(plano.name, "llama3.2:3b");
        assert_eq!(plano.freed_bytes, 2_000);
        assert!(plano.shared_with.is_empty());
    }

    #[test]
    fn o_nome_sem_tag_e_o_latest() {
        let plano = plan("llama3.2:3b", &installed(), "qwen3").unwrap();
        assert_eq!(plano.name, "qwen3:latest", "apaga o nome exato do Ollama");
        assert_eq!(plano.freed_bytes, 5_000);
    }

    #[test]
    fn nome_que_divide_o_arquivo_nao_devolve_disco() {
        let plano = plan("llama3.2:3b", &installed(), "lfm3-netmonitor:q8_0").unwrap();
        assert_eq!(plano.freed_bytes, 0);
        assert_eq!(plano.shared_with, ["lfm3-netmonitor:latest"]);
        assert_eq!(plano.size, Some(1_800));
    }

    #[test]
    fn sem_modelo_salvo_vale_o_padrao() {
        let settings = AiSettings {
            ollama_model: Some("   ".into()),
            ..AiSettings::default()
        };
        assert_eq!(configured_model(&settings), DEFAULT_OLLAMA_MODEL);
        assert_eq!(
            resolve_base_url(&settings, Some("http://10.0.0.5:11434".into())),
            "http://10.0.0.5:11434"
        );
        assert_eq!(
            resolve_base_url(&settings, Some(" ".into())),
            settings.ollama_base_url.clone().unwrap()
        );
    }
}
