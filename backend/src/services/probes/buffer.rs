//! Buffer offline do agente (§8.11).
//!
//! Quando o servidor central está inacessível, o resultado já medido não pode
//! evaporar: ele vai para um arquivo e é reenviado no primeiro ciclo em que o
//! heartbeat voltar a passar. É o que faz uma queda de link do lado do agente
//! virar um buraco preenchido, e não um buraco permanente, no histórico.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Um resultado guardado enquanto o servidor não respondia.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BufferedResult {
    pub task_id: String,
    pub monitor_id: i64,
    pub result: serde_json::Value,
    pub timestamp: String,
}

pub struct ProbeBuffer {
    file_path: PathBuf,
}

impl Default for ProbeBuffer {
    fn default() -> Self {
        Self::new(default_path())
    }
}

/// `tmp/probe_buffer.json` no diretório de trabalho — o mesmo caminho do
/// backend anterior, para uma migração não perder o que estiver pendente.
fn default_path() -> PathBuf {
    std::env::var("PROBE_BUFFER_PATH").map_or_else(
        |_| {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join("tmp")
                .join("probe_buffer.json")
        },
        PathBuf::from,
    )
}

impl ProbeBuffer {
    #[must_use]
    pub fn new(file_path: impl Into<PathBuf>) -> Self {
        Self {
            file_path: file_path.into(),
        }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.file_path
    }

    /// Acrescenta um resultado ao buffer.
    ///
    /// Erro de disco só é registrado: o agente precisa continuar rodando o
    /// próximo ciclo mesmo sem conseguir bufferizar este.
    pub async fn save_result_offline(&self, item: BufferedResult) {
        let mut pending = self.pending_results().await;
        pending.push(item);
        if let Some(parent) = self.file_path.parent() {
            if let Err(error) = tokio::fs::create_dir_all(parent).await {
                tracing::warn!(%error, "não foi possível criar o diretório do buffer do probe");
                return;
            }
        }
        match serde_json::to_vec_pretty(&pending) {
            Ok(bytes) => {
                if let Err(error) = tokio::fs::write(&self.file_path, bytes).await {
                    tracing::warn!(%error, "não foi possível gravar o buffer do probe");
                }
            }
            Err(error) => tracing::warn!(%error, "buffer do probe não pôde ser serializado"),
        }
    }

    /// Lê o buffer. Arquivo ausente, vazio ou corrompido devolve lista vazia —
    /// um JSON quebrado não pode travar o agente para sempre.
    pub async fn pending_results(&self) -> Vec<BufferedResult> {
        let Ok(raw) = tokio::fs::read_to_string(&self.file_path).await else {
            return Vec::new();
        };
        if raw.trim().is_empty() {
            return Vec::new();
        }
        serde_json::from_str(&raw).unwrap_or_else(|error| {
            tracing::warn!(%error, "buffer do probe ilegível; descartando o conteúdo");
            Vec::new()
        })
    }

    /// Limpa o buffer depois de um reenvio bem-sucedido.
    pub async fn clear_pending_results(&self) {
        if let Err(error) = tokio::fs::remove_file(&self.file_path).await {
            if error.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(%error, "não foi possível limpar o buffer do probe");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn item(task_id: &str) -> BufferedResult {
        BufferedResult {
            task_id: task_id.into(),
            monitor_id: 3,
            result: json!({ "status": "up" }),
            timestamp: "2026-08-11T10:00:00Z".into(),
        }
    }

    #[tokio::test]
    async fn acumula_e_limpa_o_buffer() {
        let dir = std::env::temp_dir().join(format!("probe-buffer-{}", std::process::id()));
        let buffer = ProbeBuffer::new(dir.join("probe_buffer.json"));
        buffer.clear_pending_results().await;

        assert!(buffer.pending_results().await.is_empty());
        buffer.save_result_offline(item("task-1")).await;
        buffer.save_result_offline(item("task-2")).await;

        let pending = buffer.pending_results().await;
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[1].task_id, "task-2");
        assert_eq!(pending[0].monitor_id, 3);

        buffer.clear_pending_results().await;
        assert!(buffer.pending_results().await.is_empty());
        // Limpar duas vezes não é erro: o arquivo simplesmente não existe.
        buffer.clear_pending_results().await;
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn buffer_corrompido_nao_trava_o_agente() {
        let dir = std::env::temp_dir().join(format!("probe-buffer-bad-{}", std::process::id()));
        tokio::fs::create_dir_all(&dir)
            .await
            .expect("criar diretório");
        let path = dir.join("probe_buffer.json");
        tokio::fs::write(&path, b"{nao e json")
            .await
            .expect("gravar");

        let buffer = ProbeBuffer::new(&path);
        assert!(buffer.pending_results().await.is_empty());
        // E ainda aceita gravar por cima, recomeçando limpo.
        buffer.save_result_offline(item("task-3")).await;
        assert_eq!(buffer.pending_results().await.len(), 1);
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }
}
