//! O que é comum aos servidores locais de modelos no molde do Ollama — o
//! próprio Ollama (LLM do chat) e o Ollaya (modelos de decisão do Decisor).
//!
//! Os dois falam o mesmo `/api/pull` (NDJSON com `status`, `digest`,
//! `total`, `completed`) e o mesmo `/api/tags`; só muda o nome do campo do
//! modelo no corpo do pull e a autenticação. Para modelos ajustados do próprio
//! NetMonitor hospedados nas Releases do GitHub (`CHAT_MODELS_URL` e
//! `DECIDER_MODELS_URL`), este módulo baixa os blobs sob demanda com
//! verificação SHA-256 em streaming e barra de progresso em tempo real:
//! - No **Ollama**, via API HTTP (`/api/blobs/:digest` + `/api/create`);
//! - No **Ollaya**, gravando os blobs e o manifesto OCI no volume compartilhado
//!   (`OLLAYA_MODELS_DIR`).

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use futures::StreamExt;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::{io::AsyncWriteExt, sync::mpsc};
use tokio_stream::wrappers::ReceiverStream;

use crate::{
    dtos::ai::ModelPullProgress,
    services::shared::errors::{AppError, AppResult},
};

/// Downloads de modelos grandes podem levar vários minutos.
const PULL_TIMEOUT: Duration = Duration::from_secs(3600);

/// Passo mínimo entre notificações de progresso do download HTTP (1 MiB).
const PROGRESS_STEP_BYTES: u64 = 1024 * 1024;

/// Um pull a iniciar: para onde, com que corpo e em nome de qual serviço
/// (o nome aparece nas mensagens de erro).
pub struct PullTarget<'a> {
    pub url: String,
    pub body: Value,
    pub bearer: Option<&'a str>,
    pub service: &'static str,
}

/// Verifica se `model` corresponde ao nome canônico ou a algum apelido
/// (com `:latest` implícito e sem diferenciar caixa).
#[must_use]
pub fn matches_model_or_alias(
    canonical_name: &str,
    aliases: &[&str],
    model: &str,
) -> bool {
    let normalize = |tag: &str| {
        let clean = tag.trim().to_ascii_lowercase();
        clean
            .strip_suffix(":latest")
            .map(str::to_string)
            .unwrap_or(clean)
    };
    let target = normalize(model);
    if target.is_empty() {
        return false;
    }
    normalize(canonical_name) == target || aliases.iter().any(|alias| normalize(alias) == target)
}

/// Resolve a URL base de uma release a partir da variável de ambiente ou do padrão.
#[must_use]
pub fn resolve_release_base_url(env_var: &str, default_url: &str) -> String {
    std::env::var(env_var)
        .ok()
        .map(|val| val.trim().trim_end_matches('/').to_string())
        .filter(|val| !val.is_empty())
        .unwrap_or_else(|| default_url.trim_end_matches('/').to_string())
}

/// Receita de um modelo ajustado do NetMonitor cujos pesos GGUF ficam numa
/// release do GitHub (ou espelho configurado por variável de ambiente),
/// instalado no Ollama via `/api/blobs/:digest` + `/api/create`.
pub struct BundledOllamaModel {
    pub canonical_name: &'static str,
    pub aliases: &'static [&'static str],
    pub release_url_env: &'static str,
    pub default_release_url: &'static str,
    pub gguf_filename: &'static str,
    pub gguf_digest: &'static str,
    pub gguf_size: u64,
    pub template: &'static str,
    pub renderer: &'static str,
    pub parser: &'static str,
    pub create_parameters: fn() -> Value,
}

impl BundledOllamaModel {
    #[must_use]
    pub fn matches(&self, model: &str) -> bool {
        matches_model_or_alias(self.canonical_name, self.aliases, model)
    }

    #[must_use]
    pub fn release_base_url(&self) -> String {
        resolve_release_base_url(self.release_url_env, self.default_release_url)
    }

    #[must_use]
    pub fn blob_download_url(&self) -> String {
        let hex = self
            .gguf_digest
            .strip_prefix("sha256:")
            .unwrap_or(self.gguf_digest);
        format!("{}/sha256-{hex}", self.release_base_url())
    }
}

/// Camada de um modelo OCI do Ollaya empacotado pelo NetMonitor.
pub struct BundledStoreLayer {
    pub digest: &'static str,
    pub size: u64,
    /// `None` baixa de `{release_base_url}/sha256-<hex>`; `Some(url)` baixa da URL direta.
    pub explicit_url: Option<&'static str>,
}

/// Pacote OCI de um modelo do Ollaya instalado sob demanda no volume (`OLLAYA_MODELS_DIR`).
pub struct BundledStoreModel {
    pub canonical_name: &'static str,
    pub aliases: &'static [&'static str],
    pub manifest_rel_path: &'static str,
    pub manifest_json: &'static str,
    pub release_url_env: &'static str,
    pub default_release_url: &'static str,
    pub store_dir_env: &'static str,
    pub default_store_dir: &'static str,
    pub layers: &'static [BundledStoreLayer],
}

impl BundledStoreModel {
    #[must_use]
    pub fn matches(&self, model: &str) -> bool {
        matches_model_or_alias(self.canonical_name, self.aliases, model)
    }

    #[must_use]
    pub fn release_base_url(&self) -> String {
        resolve_release_base_url(self.release_url_env, self.default_release_url)
    }

    #[must_use]
    pub fn store_root(&self) -> PathBuf {
        std::env::var(self.store_dir_env)
            .ok()
            .map(|val| val.trim().to_string())
            .filter(|val| !val.is_empty())
            .map_or_else(|| PathBuf::from(self.default_store_dir), PathBuf::from)
    }

    #[must_use]
    pub fn layer_url(&self, layer: &BundledStoreLayer) -> String {
        if let Some(url) = layer.explicit_url {
            return url.to_string();
        }
        let hex = layer.digest.strip_prefix("sha256:").unwrap_or(layer.digest);
        format!("{}/sha256-{hex}", self.release_base_url())
    }
}

#[derive(Debug, Deserialize)]
struct PullLine {
    status: Option<String>,
    digest: Option<String>,
    total: Option<u64>,
    completed: Option<u64>,
    error: Option<String>,
}

/// O modelo pedido está entre os instalados? Aceita a tag implícita
/// (`laya` casa `laya:latest`) e qualquer variante (`laya` casa `laya:en`).
#[must_use]
pub fn is_installed<S: AsRef<str>>(installed: &[S], wanted: &str) -> bool {
    let wanted = wanted.trim().to_lowercase();
    let bare = wanted.strip_suffix(":latest").unwrap_or(&wanted);
    installed.iter().any(|name| {
        let name = name.as_ref().to_lowercase();
        name == wanted || name == bare || name.starts_with(&format!("{bare}:"))
    })
}

/// Percentual com uma casa decimal, quando o servidor informa o tamanho.
fn percentage(completed: Option<u64>, total: Option<u64>) -> Option<f64> {
    match (completed, total) {
        (Some(done), Some(total)) if total > 0 => {
            #[allow(clippy::cast_precision_loss)]
            let pct = (done as f64 / total as f64) * 100.0;
            Some((pct * 10.0).round() / 10.0)
        }
        _ => None,
    }
}

/// O erro do servidor, com o que fazer quando ele é velho demais para o
/// modelo ("… which this version of ollaya cannot run; upgrade ollaya"): a
/// imagem do compose é `:latest`, mas só muda com um `pull` explícito.
fn explain_pull_error(service: &str, error: String) -> String {
    if error.to_lowercase().contains("upgrade") {
        format!(
            "{error} — o {service} desta instalação é anterior a este modelo. Atualize a imagem no \
             servidor (`docker compose pull` e `docker compose up -d`) ou escolha outro modelo."
        )
    } else {
        error
    }
}

/// Converte uma linha do NDJSON em progresso. `None` para linha vazia ou
/// que não é JSON.
fn parse_line(line: &str, service: &str) -> Option<ModelPullProgress> {
    let raw: PullLine = serde_json::from_str(line.trim()).ok()?;
    if let Some(error) = raw.error {
        return Some(ModelPullProgress::failed(
            "Erro",
            explain_pull_error(service, error),
        ));
    }
    let status = raw.status.unwrap_or_else(|| "Processando...".to_string());
    let done = status.eq_ignore_ascii_case("success");
    Some(ModelPullProgress {
        status,
        digest: raw.digest,
        total: raw.total,
        completed: raw.completed,
        percentage: percentage(raw.completed, raw.total),
        done,
        error: None,
    })
}

/// Consome uma resposta NDJSON (`/api/pull` ou `/api/create`) e encaminha cada
/// evento ao canal `tx`. Retorna `true` se o fluxo encerrou (por `done` ou
/// receptor fechado).
async fn pipe_ndjson_response(
    res: reqwest::Response,
    service: &'static str,
    tx: &mpsc::Sender<ModelPullProgress>,
    emit_final_on_eof: bool,
) -> bool {
    let mut byte_stream = res.bytes_stream();
    let mut line_buffer = String::new();

    while let Some(item) = byte_stream.next().await {
        let bytes = match item {
            Ok(bytes) => bytes,
            Err(e) => {
                let _ = tx
                    .send(ModelPullProgress::failed(
                        "Falha de rede",
                        format!("Erro na transmissão de dados: {e}"),
                    ))
                    .await;
                return true;
            }
        };
        line_buffer.push_str(&String::from_utf8_lossy(&bytes));

        while let Some(pos) = line_buffer.find('\n') {
            let line: String = line_buffer.drain(..=pos).collect();
            let Some(mut progress) = parse_line(&line, service) else {
                continue;
            };
            if progress.done && !emit_final_on_eof && progress.error.is_none() {
                progress.done = false;
            }
            let finished = progress.done;
            if tx.send(progress).await.is_err() || finished {
                return true;
            }
        }
    }

    if emit_final_on_eof {
        let _ = tx
            .send(ModelPullProgress {
                status: "Finalizado".to_string(),
                digest: None,
                total: None,
                completed: None,
                percentage: Some(100.0),
                done: true,
                error: None,
            })
            .await;
        return true;
    }
    false
}

/// Inicia o pull e transmite o progresso até `success`, erro ou fim do corpo.
pub async fn stream_pull(target: PullTarget<'_>) -> AppResult<ReceiverStream<ModelPullProgress>> {
    let service = target.service;
    let client = reqwest::Client::builder()
        .timeout(PULL_TIMEOUT)
        .build()
        .map_err(|e| AppError::service_unavailable(format!("Erro ao criar cliente HTTP: {e}")))?;

    let mut request = client.post(&target.url).json(&target.body);
    if let Some(token) = target.bearer.filter(|token| !token.trim().is_empty()) {
        request = request.bearer_auth(token.trim());
    }
    let res = request.send().await.map_err(|e| {
        AppError::service_unavailable(format!(
            "Não foi possível conectar ao {service} em '{}': {e}",
            target.url
        ))
    })?;

    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        return Err(AppError::service_unavailable(format!(
            "{service} retornou status {status} ao tentar baixar o modelo: {body}"
        )));
    }

    let (tx, rx) = mpsc::channel::<ModelPullProgress>(100);

    tokio::spawn(async move {
        let _ = pipe_ndjson_response(res, service, &tx, true).await;
    });

    Ok(ReceiverStream::new(rx))
}

/// Instala no Ollama um modelo empacotado pelo NetMonitor: se o blob GGUF ainda
/// não estiver em `/api/blobs/:digest`, baixa-o da release do repositório do
/// NetMonitor (`CHAT_MODELS_URL`) retransmitindo o fluxo diretamente ao Ollama
/// com conferência de SHA-256 e barra de progresso em tempo real; em seguida
/// registra o modelo via `/api/create`.
pub async fn stream_bundled_ollama_install(
    ollama_root: String,
    requested_name: String,
    spec: &'static BundledOllamaModel,
) -> AppResult<ReceiverStream<ModelPullProgress>> {
    let client = reqwest::Client::builder()
        .timeout(PULL_TIMEOUT)
        .build()
        .map_err(|e| AppError::service_unavailable(format!("Erro ao criar cliente HTTP: {e}")))?;

    let root = ollama_root.trim_end_matches('/').to_string();
    let blob_endpoint = format!("{root}/api/blobs/{}", spec.gguf_digest);

    let head_res = client.head(&blob_endpoint).send().await.map_err(|e| {
        AppError::service_unavailable(format!(
            "Não foi possível conectar ao Ollama em '{root}': {e}"
        ))
    })?;
    let blob_already_present = head_res.status().is_success();

    let (tx, rx) = mpsc::channel::<ModelPullProgress>(100);

    tokio::spawn(async move {
        if !blob_already_present {
            let download_url = spec.blob_download_url();
            let _ = tx
                .send(ModelPullProgress {
                    status: "Baixando pesos do repositório do NetMonitor...".to_string(),
                    digest: Some(spec.gguf_digest.to_string()),
                    total: Some(spec.gguf_size),
                    completed: Some(0),
                    percentage: Some(0.0),
                    done: false,
                    error: None,
                })
                .await;

            let dl_res = match client.get(&download_url).send().await {
                Ok(res) if res.status().is_success() => res,
                Ok(res) => {
                    let status = res.status();
                    let _ = tx
                        .send(ModelPullProgress::failed(
                            "Falha ao baixar release",
                            format!(
                                "O repositório do NetMonitor ('{download_url}') respondeu HTTP {status}."
                            ),
                        ))
                        .await;
                    return;
                }
                Err(e) => {
                    let _ = tx
                        .send(ModelPullProgress::failed(
                            "Falha de rede",
                            format!(
                                "Não foi possível baixar os pesos em '{download_url}': {e}"
                            ),
                        ))
                        .await;
                    return;
                }
            };

            let total_bytes = dl_res.content_length().unwrap_or(spec.gguf_size);
            let hasher = Arc::new(Mutex::new(Sha256::new()));
            let hasher_for_stream = Arc::clone(&hasher);
            let tx_for_stream = tx.clone();
            let digest_label = spec.gguf_digest.to_string();

            let mut completed_bytes: u64 = 0;
            let mut last_reported_bytes: u64 = 0;

            let upload_stream = dl_res.bytes_stream().map(move |chunk_res| {
                if let Ok(ref bytes) = chunk_res {
                    if let Ok(mut h) = hasher_for_stream.lock() {
                        h.update(bytes);
                    }
                    completed_bytes = completed_bytes.saturating_add(bytes.len() as u64);
                    if completed_bytes.saturating_sub(last_reported_bytes) >= PROGRESS_STEP_BYTES
                        || completed_bytes >= total_bytes
                    {
                        last_reported_bytes = completed_bytes;
                        let _ = tx_for_stream.try_send(ModelPullProgress {
                            status: "Baixando e enviando pesos para o Ollama...".to_string(),
                            digest: Some(digest_label.clone()),
                            total: Some(total_bytes),
                            completed: Some(completed_bytes),
                            percentage: percentage(Some(completed_bytes), Some(total_bytes)),
                            done: false,
                            error: None,
                        });
                    }
                }
                chunk_res
            });

            let upload_res = client
                .post(&blob_endpoint)
                .header(reqwest::header::CONTENT_LENGTH, total_bytes)
                .body(reqwest::Body::wrap_stream(upload_stream))
                .send()
                .await;

            match upload_res {
                Ok(res) if res.status().is_success() => {}
                Ok(res) => {
                    let status = res.status();
                    let body = res.text().await.unwrap_or_default();
                    let _ = tx
                        .send(ModelPullProgress::failed(
                            "Erro no Ollama",
                            format!("Ollama recusou o blob GGUF (HTTP {status}): {body}"),
                        ))
                        .await;
                    return;
                }
                Err(e) => {
                    let _ = tx
                        .send(ModelPullProgress::failed(
                            "Erro no envio ao Ollama",
                            format!("Falha ao transferir os pesos para o Ollama: {e}"),
                        ))
                        .await;
                    return;
                }
            }

            let actual_digest = {
                let mut guard = match hasher.lock() {
                    Ok(g) => g,
                    Err(poisoned) => poisoned.into_inner(),
                };
                let digest_bytes = std::mem::replace(&mut *guard, Sha256::new()).finalize();
                format!("sha256:{}", hex::encode(digest_bytes))
            };

            if actual_digest != spec.gguf_digest {
                let _ = tx
                    .send(ModelPullProgress::failed(
                        "SHA-256 inválido",
                        format!(
                            "O hash do arquivo baixado ({actual_digest}) não confere com o esperado ({}).",
                            spec.gguf_digest
                        ),
                    ))
                    .await;
                return;
            }
        }

        let mut targets = vec![spec.canonical_name.to_string()];
        let trimmed_req = requested_name.trim();
        if !trimmed_req.is_empty()
            && !trimmed_req.eq_ignore_ascii_case(spec.canonical_name)
            && !trimmed_req
                .strip_suffix(":latest")
                .is_some_and(|b| spec.canonical_name.eq_ignore_ascii_case(&format!("{b}:latest")))
        {
            targets.push(trimmed_req.to_string());
        }

        let create_url = format!("{root}/api/create");
        let total_targets = targets.len();
        for (idx, model_target) in targets.into_iter().enumerate() {
            let is_last = idx + 1 == total_targets;
            let create_body = json!({
                "model": model_target,
                "files": {
                    spec.gguf_filename: spec.gguf_digest,
                },
                "template": spec.template,
                "renderer": spec.renderer,
                "parser": spec.parser,
                "parameters": (spec.create_parameters)(),
                "stream": true,
            });

            let create_res = match client.post(&create_url).json(&create_body).send().await {
                Ok(res) if res.status().is_success() => res,
                Ok(res) => {
                    let status = res.status();
                    let body = res.text().await.unwrap_or_default();
                    let _ = tx
                        .send(ModelPullProgress::failed(
                            "Erro ao registrar modelo",
                            format!("Ollama retornou HTTP {status} em /api/create: {body}"),
                        ))
                        .await;
                    return;
                }
                Err(e) => {
                    let _ = tx
                        .send(ModelPullProgress::failed(
                            "Falha de rede",
                            format!("Erro ao chamar /api/create no Ollama: {e}"),
                        ))
                        .await;
                    return;
                }
            };

            if pipe_ndjson_response(create_res, "Ollama", &tx, is_last).await && is_last {
                return;
            }
        }
    });

    Ok(ReceiverStream::new(rx))
}

/// Verifica se um arquivo existe com o tamanho esperado.
async fn blob_exists_with_size(path: &Path, expected_size: u64) -> bool {
    tokio::fs::metadata(path)
        .await
        .is_ok_and(|meta| meta.is_file() && meta.len() == expected_size)
}

/// Baixa e instala sob demanda um modelo OCI do Ollaya (`BundledStoreModel`)
/// diretamente da release do NetMonitor (`DECIDER_MODELS_URL`) para o volume
/// compartilhado (`OLLAYA_MODELS_DIR`), conferindo o SHA-256 de cada camada
/// e gravando o manifesto por último.
pub async fn stream_bundled_store_install(
    spec: &'static BundledStoreModel,
) -> AppResult<ReceiverStream<ModelPullProgress>> {
    let client = reqwest::Client::builder()
        .timeout(PULL_TIMEOUT)
        .build()
        .map_err(|e| AppError::service_unavailable(format!("Erro ao criar cliente HTTP: {e}")))?;

    let store_root = spec.store_root();
    let blobs_dir = store_root.join("blobs");
    let manifest_path = store_root.join(spec.manifest_rel_path);
    let Some(manifest_parent) = manifest_path.parent().map(Path::to_path_buf) else {
        return Err(AppError::validation("Caminho de manifesto inválido"));
    };

    tokio::fs::create_dir_all(&blobs_dir).await.map_err(|e| {
        AppError::service_unavailable(format!(
            "Não foi possível acessar o diretório de modelos do Ollaya em '{}': {e}",
            blobs_dir.display()
        ))
    })?;
    tokio::fs::create_dir_all(&manifest_parent)
        .await
        .map_err(|e| {
            AppError::service_unavailable(format!(
                "Não foi possível criar diretório de manifestos em '{}': {e}",
                manifest_parent.display()
            ))
        })?;

    let (tx, rx) = mpsc::channel::<ModelPullProgress>(100);

    tokio::spawn(async move {
        let total_bytes: u64 = spec.layers.iter().map(|l| l.size).sum();
        let mut completed_bytes: u64 = 0;
        let mut last_reported_bytes: u64 = 0;

        let _ = tx
            .send(ModelPullProgress {
                status: format!("Verificando camadas de {}...", spec.canonical_name),
                digest: None,
                total: Some(total_bytes),
                completed: Some(0),
                percentage: Some(0.0),
                done: false,
                error: None,
            })
            .await;

        for layer in spec.layers {
            let hex = layer.digest.strip_prefix("sha256:").unwrap_or(layer.digest);
            let blob_path = blobs_dir.join(format!("sha256-{hex}"));

            if blob_exists_with_size(&blob_path, layer.size).await {
                completed_bytes = completed_bytes.saturating_add(layer.size);
                last_reported_bytes = completed_bytes;
                let _ = tx
                    .send(ModelPullProgress {
                        status: format!("Camada sha256-{} já presente", &hex[..12.min(hex.len())]),
                        digest: Some(layer.digest.to_string()),
                        total: Some(total_bytes),
                        completed: Some(completed_bytes),
                        percentage: percentage(Some(completed_bytes), Some(total_bytes)),
                        done: false,
                        error: None,
                    })
                    .await;
                continue;
            }

            let url = spec.layer_url(layer);
            let res = match client.get(&url).send().await {
                Ok(r) if r.status().is_success() => r,
                Ok(r) => {
                    let status = r.status();
                    let _ = tx
                        .send(ModelPullProgress::failed(
                            "Falha ao baixar camada",
                            format!("Download de '{url}' retornou HTTP {status}."),
                        ))
                        .await;
                    return;
                }
                Err(e) => {
                    let _ = tx
                        .send(ModelPullProgress::failed(
                            "Falha de rede",
                            format!("Não foi possível baixar '{url}': {e}"),
                        ))
                        .await;
                    return;
                }
            };

            let partial_path = blobs_dir.join(format!("sha256-{hex}-partial"));
            let mut file = match tokio::fs::File::create(&partial_path).await {
                Ok(f) => f,
                Err(e) => {
                    let _ = tx
                        .send(ModelPullProgress::failed(
                            "Erro de disco",
                            format!("Falha ao criar '{}': {e}", partial_path.display()),
                        ))
                        .await;
                    return;
                }
            };

            let mut hasher = Sha256::new();
            let mut stream = res.bytes_stream();
            while let Some(chunk_res) = stream.next().await {
                let bytes = match chunk_res {
                    Ok(b) => b,
                    Err(e) => {
                        let _ = tokio::fs::remove_file(&partial_path).await;
                        let _ = tx
                            .send(ModelPullProgress::failed(
                                "Falha de rede",
                                format!("Erro durante download de '{url}': {e}"),
                            ))
                            .await;
                        return;
                    }
                };
                hasher.update(&bytes);
                if let Err(e) = file.write_all(&bytes).await {
                    let _ = tokio::fs::remove_file(&partial_path).await;
                    let _ = tx
                        .send(ModelPullProgress::failed(
                            "Erro de disco",
                            format!("Erro ao gravar camada no volume: {e}"),
                        ))
                        .await;
                    return;
                }
                completed_bytes = completed_bytes.saturating_add(bytes.len() as u64);
                if completed_bytes.saturating_sub(last_reported_bytes) >= PROGRESS_STEP_BYTES
                    || completed_bytes >= total_bytes
                {
                    last_reported_bytes = completed_bytes;
                    let _ = tx
                        .send(ModelPullProgress {
                            status: format!(
                                "Baixando sha256-{}...",
                                &hex[..12.min(hex.len())]
                            ),
                            digest: Some(layer.digest.to_string()),
                            total: Some(total_bytes),
                            completed: Some(completed_bytes),
                            percentage: percentage(Some(completed_bytes), Some(total_bytes)),
                            done: false,
                            error: None,
                        })
                        .await;
                }
            }

            if let Err(e) = file.flush().await {
                let _ = tokio::fs::remove_file(&partial_path).await;
                let _ = tx
                    .send(ModelPullProgress::failed(
                        "Erro de disco",
                        format!("Erro ao finalizar arquivo: {e}"),
                    ))
                    .await;
                return;
            }
            drop(file);

            let actual_hex = hex::encode(hasher.finalize());
            if actual_hex != hex {
                let _ = tokio::fs::remove_file(&partial_path).await;
                let _ = tx
                    .send(ModelPullProgress::failed(
                        "SHA-256 inválido",
                        format!(
                            "Camada corrompida: esperado sha256:{hex}, obtido sha256:{actual_hex}."
                        ),
                    ))
                    .await;
                return;
            }

            if let Err(e) = tokio::fs::rename(&partial_path, &blob_path).await {
                let _ = tokio::fs::remove_file(&partial_path).await;
                let _ = tx
                    .send(ModelPullProgress::failed(
                        "Erro de disco",
                        format!("Não foi possível mover blob para '{}': {e}", blob_path.display()),
                    ))
                    .await;
                return;
            }
        }

        if let Err(e) = tokio::fs::write(&manifest_path, spec.manifest_json).await {
            let _ = tx
                .send(ModelPullProgress::failed(
                    "Erro ao gravar manifesto",
                    format!("Não foi possível gravar '{}': {e}", manifest_path.display()),
                ))
                .await;
            return;
        }

        let _ = tx
            .send(ModelPullProgress {
                status: "success".to_string(),
                digest: None,
                total: Some(total_bytes),
                completed: Some(total_bytes),
                percentage: Some(100.0),
                done: true,
                error: None,
            })
            .await;
    });

    Ok(ReceiverStream::new(rx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconhece_modelo_instalado_por_tag_e_variante() {
        let installed = ["laya:en", "ornith-1.5:9b", "llama3.2:latest"];
        assert!(is_installed(&installed, "laya"));
        assert!(is_installed(&installed, "ornith-1.5:9b"));
        assert!(is_installed(&installed, "llama3.2"));
        assert!(is_installed(&installed, "LLAMA3.2:latest"));
        assert!(!is_installed(&installed, "laya-typed"));
        assert!(!is_installed(&installed, "ornith-1.5:3b"));
    }

    #[test]
    fn reconhece_modelo_empacotado_e_monta_url_da_release() {
        static TEST_SPEC: BundledOllamaModel = BundledOllamaModel {
            canonical_name: "lfm3-netmonitor:latest",
            aliases: &["netmonitor/lfm3-netmonitor", "netmonitor/lfm3-netmonitor:latest"],
            release_url_env: "CHAT_MODELS_URL_TEST_NONEXISTENT",
            default_release_url: "https://github.com/maxupunk/monitor-de-rede/releases/download/chat-models",
            gguf_filename: "lfm3-netmonitor-q8_0.gguf",
            gguf_digest: "sha256:a1ff35a978ca9f9dc09d1ecf062a66596c1dd1c4c835c4b127778fe5581cd00d",
            gguf_size: 1_846_237_728,
            template: "{{ .Prompt }}",
            renderer: "lfm3",
            parser: "lfm3",
            create_parameters: || serde_json::json!({ "temperature": 0.1 }),
        };

        assert!(TEST_SPEC.matches("lfm3-netmonitor"));
        assert!(TEST_SPEC.matches("lfm3-netmonitor:latest"));
        assert!(TEST_SPEC.matches("LFM3-NETMONITOR:LATEST"));
        assert!(TEST_SPEC.matches("netmonitor/lfm3-netmonitor"));
        assert!(TEST_SPEC.matches("netmonitor/lfm3-netmonitor:latest"));
        assert!(!TEST_SPEC.matches("lfm3:latest"));
        assert!(!TEST_SPEC.matches(""));
        assert_eq!(
            TEST_SPEC.blob_download_url(),
            "https://github.com/maxupunk/monitor-de-rede/releases/download/chat-models/sha256-a1ff35a978ca9f9dc09d1ecf062a66596c1dd1c4c835c4b127778fe5581cd00d"
        );
    }

    #[test]
    fn converte_linhas_do_pull() {
        let line = r#"{"status":"pulling 8d32","digest":"sha256:8d32","total":200,"completed":50}"#;
        let progress = parse_line(line, "Ollaya").expect("linha válida");
        assert_eq!(progress.percentage, Some(25.0));
        assert!(!progress.done);

        assert!(parse_line(r#"{"status":"success"}"#, "Ollaya").is_some_and(|p| p.done));

        let failed = parse_line(r#"{"error":"model not found"}"#, "Ollaya").expect("erro");
        assert!(failed.done);
        assert_eq!(failed.error.as_deref(), Some("model not found"));

        assert!(parse_line("", "Ollaya").is_none());
        assert!(parse_line("keep-alive", "Ollaya").is_none());

        let old = parse_line(
            r#"{"error":"jeb:4b uses the jebadiah-v1 layout, which this version of ollaya cannot run; upgrade ollaya"}"#,
            "Ollaya",
        )
        .expect("erro");
        assert!(old
            .error
            .as_deref()
            .is_some_and(|e| e.contains("docker compose pull")));
    }
}
