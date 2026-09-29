//! O que é comum aos servidores locais de modelos no molde do Ollama — o
//! próprio Ollama (LLM do chat) e o Ollaya (modelos de decisão do Laya).
//!
//! Os dois falam o mesmo `/api/pull` (NDJSON com `status`, `digest`,
//! `total`, `completed`) e o mesmo `/api/tags`; só muda o nome do campo do
//! modelo no corpo do pull e a autenticação.

use std::time::Duration;

use futures::StreamExt;
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::{
    dtos::ai::ModelPullProgress,
    services::shared::errors::{AppError, AppResult},
};

/// Downloads de modelos grandes podem levar vários minutos.
const PULL_TIMEOUT: Duration = Duration::from_secs(3600);

/// Um pull a iniciar: para onde, com que corpo e em nome de qual serviço
/// (o nome aparece nas mensagens de erro).
pub struct PullTarget<'a> {
    pub url: String,
    pub body: Value,
    pub bearer: Option<&'a str>,
    pub service: &'static str,
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

/// Converte uma linha do NDJSON em progresso. `None` para linha vazia ou
/// que não é JSON.
fn parse_line(line: &str) -> Option<ModelPullProgress> {
    let raw: PullLine = serde_json::from_str(line.trim()).ok()?;
    if let Some(error) = raw.error {
        return Some(ModelPullProgress::failed("Erro", error));
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
                    return;
                }
            };
            line_buffer.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(pos) = line_buffer.find('\n') {
                let line: String = line_buffer.drain(..=pos).collect();
                let Some(progress) = parse_line(&line) else {
                    continue;
                };
                let finished = progress.done;
                if tx.send(progress).await.is_err() || finished {
                    return;
                }
            }
        }

        // O corpo terminou sem `success`.
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
    fn converte_linhas_do_pull() {
        let line = r#"{"status":"pulling 8d32","digest":"sha256:8d32","total":200,"completed":50}"#;
        let progress = parse_line(line).expect("linha válida");
        assert_eq!(progress.percentage, Some(25.0));
        assert!(!progress.done);

        assert!(parse_line(r#"{"status":"success"}"#).is_some_and(|p| p.done));

        let failed = parse_line(r#"{"error":"model not found"}"#).expect("erro");
        assert!(failed.done);
        assert_eq!(failed.error.as_deref(), Some("model not found"));

        assert!(parse_line("").is_none());
        assert!(parse_line("keep-alive").is_none());
    }
}
