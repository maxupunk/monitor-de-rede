//! Os modelos de decisão que vêm com o NetMonitor e o próprio servidor
//! instala no Ollama — sem container instalador nem passo manual.
//!
//! O modelo é um GGUF (família Laya, `decision.type = laya`) publicado na
//! release `decider-models` do projeto. Não está no registro do Ollama, então
//! não há `ollama pull`: o arquivo vai direto da release para o Ollama
//! (`POST /api/blobs/sha256:…`, em fluxo, sem passar pelo disco daqui), e o
//! `POST /api/create` registra o modelo com o nome dele. O Ollama confere o
//! sha256 do que recebeu; um arquivo corrompido ou trocado é recusado lá.

use std::time::Duration;

use futures::StreamExt;
use serde_json::json;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use super::catalog;
use crate::{
    dtos::ai::ModelPullProgress,
    services::shared::errors::{AppError, AppResult},
};

/// Um modelo publicado na release do projeto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BundledModel {
    /// Nome no Ollama.
    pub name: &'static str,
    /// Arquivo na release.
    pub file: &'static str,
    /// `sha256:<hex>` do arquivo — é também o blob no Ollama.
    pub digest: &'static str,
    pub size: u64,
}

/// O que vem com o NetMonitor.
pub const BUNDLED: &[BundledModel] = &[BundledModel {
    name: "netmonitor/laya-network:v2",
    file: "netmonitor-laya-network-v2.gguf",
    digest: "sha256:e517fc60eb72b500446d4e0df09ae4ffd190978f2ffd7903bf555bd38bf9f92c",
    size: 658_414_976,
}];

/// Onde estão os arquivos: a release do projeto ou um espelho
/// (`DECIDER_MODELS_URL`, para instalação sem acesso ao GitHub).
pub const MODELS_URL_ENV: &str = "DECIDER_MODELS_URL";
pub const DEFAULT_MODELS_URL: &str =
    "https://github.com/maxupunk/monitor-de-rede/releases/download/decider-models";

/// Janela do modelo criado — a mesma da Laya da biblioteca do Ollama.
const NUM_CTX: u32 = 8_192;
/// Baixar ~650 MB numa conexão lenta.
const INSTALL_TIMEOUT: Duration = Duration::from_secs(3_600);
/// Um evento de progresso a cada tanto, para o SSE não virar enxurrada.
const PROGRESS_STEP: u64 = 4 * 1024 * 1024;
const SERVICE: &str = "Ollama";

/// O modelo, se ele vem com o NetMonitor.
#[must_use]
pub fn find(name: &str) -> Option<&'static BundledModel> {
    BUNDLED
        .iter()
        .find(|model| catalog::same_tag(model.name, name))
}

fn models_url() -> String {
    std::env::var(MODELS_URL_ENV)
        .ok()
        .map(|url| url.trim().trim_end_matches('/').to_string())
        .filter(|url| !url.is_empty())
        .unwrap_or_else(|| DEFAULT_MODELS_URL.to_string())
}

fn progress(status: &str, completed: u64, total: u64) -> ModelPullProgress {
    ModelPullProgress {
        status: status.to_string(),
        digest: None,
        total: Some(total),
        completed: Some(completed),
        percentage: (total > 0).then(|| (completed as f64 / total as f64 * 100.0).min(100.0)),
        done: false,
        error: None,
    }
}

fn finished() -> ModelPullProgress {
    ModelPullProgress {
        status: "success".to_string(),
        digest: None,
        total: None,
        completed: None,
        percentage: Some(100.0),
        done: true,
        error: None,
    }
}

fn client() -> AppResult<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(INSTALL_TIMEOUT)
        .build()
        .map_err(|e| AppError::service_unavailable(format!("Erro ao criar cliente HTTP: {e}")))
}

fn with_bearer(request: reqwest::RequestBuilder, bearer: Option<&str>) -> reqwest::RequestBuilder {
    match bearer.map(str::trim).filter(|token| !token.is_empty()) {
        Some(token) => request.bearer_auth(token),
        None => request,
    }
}

/// O Ollama já tem o arquivo (de uma instalação que parou no `create`)?
async fn has_blob(
    http: &reqwest::Client,
    base_url: &str,
    model: &BundledModel,
    bearer: Option<&str>,
) -> bool {
    let request = http.head(format!("{base_url}/api/blobs/{}", model.digest));
    with_bearer(request, bearer)
        .send()
        .await
        .is_ok_and(|res| res.status().is_success())
}

/// Release → Ollama, em fluxo, avisando o progresso.
async fn upload(
    http: &reqwest::Client,
    base_url: &str,
    model: &BundledModel,
    bearer: Option<&str>,
    tx: &mpsc::Sender<ModelPullProgress>,
) -> Result<(), String> {
    let source = format!("{}/{}", models_url(), model.file);
    let download = http
        .get(&source)
        .send()
        .await
        .map_err(|e| format!("não foi possível baixar {source}: {e}"))?;
    if !download.status().is_success() {
        return Err(format!(
            "{source} respondeu {}: confira a release (ou {MODELS_URL_ENV})",
            download.status()
        ));
    }
    let total = download.content_length().unwrap_or(model.size);
    let reporter = tx.clone();
    let mut sent = 0_u64;
    let mut reported = 0_u64;
    let body = download.bytes_stream().inspect(move |chunk| {
        if let Ok(bytes) = chunk {
            sent += bytes.len() as u64;
            if sent - reported >= PROGRESS_STEP || sent == total {
                reported = sent;
                // Sem espera: perder um evento de progresso não importa.
                let _ = reporter.try_send(progress("Baixando o modelo", sent, total));
            }
        }
    });
    let request = http
        .post(format!("{base_url}/api/blobs/{}", model.digest))
        .header(reqwest::header::CONTENT_LENGTH, total)
        .body(reqwest::Body::wrap_stream(body));
    let res = with_bearer(request, bearer)
        .send()
        .await
        .map_err(|e| format!("o {SERVICE} não recebeu o arquivo: {e}"))?;
    if !res.status().is_success() {
        let status = res.status();
        let detail = res.text().await.unwrap_or_default();
        return Err(format!(
            "o {SERVICE} recusou o arquivo ({status}): {detail}"
        ));
    }
    Ok(())
}

async fn create(
    http: &reqwest::Client,
    base_url: &str,
    model: &BundledModel,
    bearer: Option<&str>,
) -> Result<(), String> {
    let request = http.post(format!("{base_url}/api/create")).json(&json!({
        "model": model.name,
        "files": { model.file: model.digest },
        // A Laya monta a própria entrada (template `systemone` no GGUF).
        "template": "{{ .Prompt }}",
        "parameters": { "num_ctx": NUM_CTX },
        "stream": false,
    }));
    let res = with_bearer(request, bearer)
        .send()
        .await
        .map_err(|e| format!("o {SERVICE} não respondeu ao criar o modelo: {e}"))?;
    if !res.status().is_success() {
        let status = res.status();
        let detail = res.text().await.unwrap_or_default();
        return Err(format!(
            "o {SERVICE} não criou o modelo ({status}): {detail}"
        ));
    }
    Ok(())
}

/// Instala o modelo no Ollama em `base_url`, transmitindo o progresso no
/// mesmo formato do `/api/pull` (a tela mostra os dois do mesmo jeito).
///
/// # Errors
///
/// Só a criação do cliente HTTP; as falhas da instalação vão no último
/// evento do fluxo.
pub fn install(
    model: &'static BundledModel,
    base_url: &str,
    bearer: Option<String>,
) -> AppResult<ReceiverStream<ModelPullProgress>> {
    let http = client()?;
    let base_url = base_url.trim_end_matches('/').to_string();
    let (tx, rx) = mpsc::channel::<ModelPullProgress>(64);
    tokio::spawn(async move {
        let bearer = bearer.as_deref();
        if !has_blob(&http, &base_url, model, bearer).await {
            let _ = tx.send(progress("Baixando o modelo", 0, model.size)).await;
            if let Err(message) = upload(&http, &base_url, model, bearer, &tx).await {
                let _ = tx
                    .send(ModelPullProgress::failed("Falha ao instalar", message))
                    .await;
                return;
            }
        }
        let _ = tx
            .send(progress("Registrando o modelo", model.size, model.size))
            .await;
        match create(&http, &base_url, model, bearer).await {
            Ok(()) => {
                tracing::info!(
                    model = model.name,
                    "decisor: modelo do NetMonitor instalado no Ollama"
                );
                let _ = tx.send(finished()).await;
            }
            Err(message) => {
                let _ = tx
                    .send(ModelPullProgress::failed("Falha ao instalar", message))
                    .await;
            }
        }
    });
    Ok(ReceiverStream::new(rx))
}

/// Instala e espera o fim. Devolve a mensagem de erro, se houve.
///
/// # Errors
///
/// A instalação falhou (download, envio ou criação).
pub async fn install_and_wait(
    model: &'static BundledModel,
    base_url: &str,
    bearer: Option<String>,
    mut on_progress: impl FnMut(&ModelPullProgress),
) -> AppResult<()> {
    let mut events = install(model, base_url, bearer)?;
    while let Some(event) = events.next().await {
        on_progress(&event);
        if event.done {
            return match event.error {
                Some(error) => Err(AppError::service_unavailable(error)),
                None => Ok(()),
            };
        }
    }
    Err(AppError::service_unavailable(
        "A instalação do modelo terminou sem resposta do Ollama",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todo_modelo_que_vem_junto_esta_no_catalogo_marcado_como_tal() {
        for model in BUNDLED {
            assert!(catalog::is_bundled(model.name), "{}", model.name);
            assert!(model.digest.starts_with("sha256:"), "{}", model.name);
            assert_eq!(model.digest.len(), "sha256:".len() + 64, "{}", model.name);
            assert!(model.file.ends_with(".gguf"), "{}", model.name);
        }
        assert!(find("NETMONITOR/laya-network:v2").is_some());
        assert!(find("laya:322m-multilingual-bf16").is_none());
    }

    #[test]
    fn progresso_em_porcentagem() {
        let half = progress("Baixando o modelo", 50, 200);
        assert_eq!(half.percentage, Some(25.0));
        assert!(!half.done);
        assert!(finished().done);
    }
}
