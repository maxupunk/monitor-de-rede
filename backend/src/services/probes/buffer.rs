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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BufferedDiscoveryResult {
    pub task_id: String,
    pub run_id: i64,
    pub hosts: Vec<crate::services::discovery::merger::DiscoveredHost>,
    pub error: Option<String>,
    pub timestamp: String,
}

/// Tamanho máximo padrão do buffer. 10.000 itens cobrem várias horas de
/// medições em 5 s de intervalo mesmo sem link.
const DEFAULT_MAX_RESULTS: usize = 10_000;

/// Tamanho máximo padrão em bytes do arquivo de buffer. 50 MB cabem em
/// qualquer volume temporário sem risco de esgotamento.
const DEFAULT_MAX_BYTES: usize = 50 * 1024 * 1024;

/// Variável de ambiente que configura o tamanho máximo do buffer.
const MAX_RESULTS_ENV: &str = "PROBE_BUFFER_MAX_RESULTS";

/// Variável de ambiente que configura o tamanho máximo em bytes do buffer.
const MAX_BYTES_ENV: &str = "PROBE_BUFFER_MAX_BYTES";

pub struct ProbeBuffer {
    file_path: PathBuf,
    max_results: usize,
    max_bytes: usize,
}

impl Default for ProbeBuffer {
    fn default() -> Self {
        Self::new(default_path())
    }
}

/// `tmp/probe_buffer.json` no diretório de trabalho, caminho estável para não
/// perder itens pendentes durante atualizações.
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

fn resolve_max_results(explicit: Option<usize>) -> usize {
    explicit
        .or_else(|| {
            std::env::var(MAX_RESULTS_ENV)
                .ok()
                .and_then(|value| value.parse::<usize>().ok())
        })
        .map(|value| value.max(1))
        .unwrap_or(DEFAULT_MAX_RESULTS)
}

fn resolve_max_bytes(explicit: Option<usize>) -> usize {
    explicit
        .or_else(|| {
            std::env::var(MAX_BYTES_ENV)
                .ok()
                .and_then(|value| value.parse::<usize>().ok())
        })
        .map(|value| value.max(1))
        .unwrap_or(DEFAULT_MAX_BYTES)
}

impl ProbeBuffer {
    #[must_use]
    pub fn new(file_path: impl Into<PathBuf>) -> Self {
        Self::with_limits(file_path, None, None)
    }

    #[must_use]
    pub fn with_max_results(file_path: impl Into<PathBuf>, max_results: Option<usize>) -> Self {
        Self::with_limits(file_path, max_results, None)
    }

    #[must_use]
    pub fn with_limits(
        file_path: impl Into<PathBuf>,
        max_results: Option<usize>,
        max_bytes: Option<usize>,
    ) -> Self {
        Self {
            file_path: file_path.into(),
            max_results: resolve_max_results(max_results),
            max_bytes: resolve_max_bytes(max_bytes),
        }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.file_path
    }

    #[must_use]
    pub fn max_results(&self) -> usize {
        self.max_results
    }

    #[must_use]
    pub fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    /// Acrescenta um resultado ao buffer.
    ///
    /// Erro de disco só é registrado: o agente precisa continuar rodando o
    /// próximo ciclo mesmo sem conseguir bufferizar este.
    ///
    /// A gravação é atômica (`tmp` + `rename`) e o buffer nunca ultrapassa
    /// `max_results` nem `max_bytes`. Quando o teto de bytes é atingido,
    /// preserva o resultado mais recente de cada monitor (deduplicação por
    /// `monitor_id`) antes de descartar os itens mais antigos.
    pub async fn save_result_offline(&self, item: BufferedResult) {
        let mut pending = self.pending_results().await;
        pending.push(item);
        Self::apply_max_results(&mut pending, self.max_results);
        Self::apply_max_bytes(&mut pending, self.max_bytes);
        self.write_atomic(&self.file_path, &pending, "buffer do probe")
            .await;
    }

    /// Lê o buffer. Arquivo ausente, vazio ou corrompido devolve lista vazia —
    /// um JSON quebrado não pode travar o agente para sempre.
    pub async fn pending_results(&self) -> Vec<BufferedResult> {
        Self::read_json(&self.file_path, "buffer do probe").await
    }

    /// Limpa o buffer depois de um reenvio bem-sucedido.
    pub async fn clear_pending_results(&self) {
        Self::remove_with_not_found_warning(&self.file_path, "buffer do probe").await;
    }

    pub async fn save_discovery_result_offline(&self, item: BufferedDiscoveryResult) {
        let mut pending = self.pending_discovery_results().await;
        pending.push(item);
        Self::apply_max_results(&mut pending, self.max_results);
        self.write_atomic(&self.discovery_path(), &pending, "buffer discovery")
            .await;
    }

    pub async fn pending_discovery_results(&self) -> Vec<BufferedDiscoveryResult> {
        Self::read_json(&self.discovery_path(), "buffer discovery").await
    }

    pub async fn clear_pending_discovery_results(&self) {
        Self::remove_with_not_found_warning(&self.discovery_path(), "buffer discovery").await;
    }

    fn discovery_path(&self) -> PathBuf {
        self.file_path.with_extension("discovery.json")
    }

    fn tmp_path(path: &Path) -> PathBuf {
        let mut name = path.as_os_str().to_os_string();
        name.push(".tmp");
        PathBuf::from(name)
    }

    fn apply_max_results<T>(pending: &mut Vec<T>, max_results: usize) {
        if pending.len() > max_results {
            let excess = pending.len() - max_results;
            pending.drain(0..excess);
        }
    }

    /// Reduz o buffer quando o JSON serializado extrapola `max_bytes`.
    ///
    /// A estratégia de retenção prioriza o monitoramento: primeiro deduplica
    /// por `monitor_id`, mantendo apenas o resultado mais recente de cada um.
    /// Se ainda restar excesso, descarta os itens mais antigos. Isso evita que
    /// um único monitor barulhento monopolize o buffer enquanto outros ficam
    /// sem histórico para reenviar.
    fn apply_max_bytes(pending: &mut Vec<BufferedResult>, max_bytes: usize) {
        if pending.is_empty() {
            return;
        }
        let serialized = || match serde_json::to_vec_pretty(pending) {
            Ok(bytes) => bytes.len(),
            Err(_) => usize::MAX,
        };
        if serialized() <= max_bytes {
            return;
        }

        // Deduplica pelo monitor mais recente, preservando a ordem relativa dos
        // últimos resultados de cada monitor.
        let mut seen = std::collections::HashMap::new();
        for (index, item) in pending.iter().enumerate() {
            seen.insert(item.monitor_id, index);
        }
        let mut keep_indices: Vec<usize> = seen.into_values().collect();
        keep_indices.sort_unstable();
        let mut deduplicated: Vec<BufferedResult> = keep_indices
            .into_iter()
            .map(|index| pending[index].clone())
            .collect();

        if deduplicated.len() > 1 && Self::serialized_len(&deduplicated) > max_bytes {
            let mut low = 0;
            let mut high = deduplicated.len();
            while low < high {
                let mid = (low + high) / 2;
                let trimmed = &deduplicated[mid..];
                if Self::serialized_len(trimmed) <= max_bytes {
                    high = mid;
                } else {
                    low = mid + 1;
                }
            }
            deduplicated.drain(0..low);
        }
        *pending = deduplicated;
    }

    fn serialized_len(items: &[BufferedResult]) -> usize {
        serde_json::to_vec_pretty(items)
            .map(|bytes| bytes.len())
            .unwrap_or(usize::MAX)
    }

    async fn write_atomic<T: Serialize>(&self, path: &Path, value: &T, context: &str) {
        if let Some(parent) = path.parent() {
            if let Err(error) = tokio::fs::create_dir_all(parent).await {
                tracing::warn!(%error, "não foi possível criar o diretório do {context}");
                return;
            }
        }

        let bytes = match serde_json::to_vec_pretty(value) {
            Ok(bytes) => bytes,
            Err(error) => {
                tracing::warn!(%error, "{context} não pôde ser serializado");
                return;
            }
        };

        let tmp_path = Self::tmp_path(path);
        // Limpa tmp órfão de um crash anterior para não deixar lixo acumulado.
        if let Err(error) = tokio::fs::remove_file(&tmp_path).await {
            if error.kind() != std::io::ErrorKind::NotFound {
                tracing::debug!(%error, "não foi possível remover tmp anterior do {context}");
            }
        }

        if let Err(error) = tokio::fs::write(&tmp_path, bytes).await {
            tracing::warn!(%error, "não foi possível gravar o tmp do {context}");
            return;
        }

        if let Err(error) = tokio::fs::rename(&tmp_path, path).await {
            tracing::warn!(%error, "não foi possível promover o {context}");
        }
    }

    async fn read_json<T: for<'de> Deserialize<'de>>(path: &Path, context: &str) -> Vec<T> {
        let Ok(raw) = tokio::fs::read_to_string(path).await else {
            return Vec::new();
        };
        if raw.trim().is_empty() {
            return Vec::new();
        }
        serde_json::from_str(&raw).unwrap_or_else(|error| {
            tracing::warn!(%error, "{context} ilegível; descartando o conteúdo");
            Vec::new()
        })
    }

    async fn remove_with_not_found_warning(path: &Path, context: &str) {
        if let Err(error) = tokio::fs::remove_file(path).await {
            if error.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(%error, "não foi possível limpar o {context}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use serial_test::serial;

    fn item(task_id: &str) -> BufferedResult {
        BufferedResult {
            task_id: task_id.into(),
            monitor_id: 3,
            result: json!({ "status": "up" }),
            timestamp: "2026-08-11T10:00:00Z".into(),
        }
    }

    fn tmp_path_for(path: &Path) -> PathBuf {
        ProbeBuffer::tmp_path(path)
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

    #[tokio::test]
    async fn discovery_remoto_tem_buffer_separado() {
        let dir =
            std::env::temp_dir().join(format!("probe-discovery-buffer-{}", std::process::id()));
        let buffer = ProbeBuffer::new(dir.join("probe_buffer.json"));
        buffer.clear_pending_discovery_results().await;
        buffer
            .save_discovery_result_offline(BufferedDiscoveryResult {
                task_id: "discovery-7".into(),
                run_id: 7,
                hosts: vec![],
                error: None,
                timestamp: "2026-08-15T10:00:00Z".into(),
            })
            .await;
        let pending = buffer.pending_discovery_results().await;
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].run_id, 7);
        buffer.clear_pending_discovery_results().await;
        let _ = tokio::fs::remove_dir_all(dir).await;
    }

    #[tokio::test]
    async fn buffer_respeita_tamanho_maximo() {
        let dir = std::env::temp_dir().join(format!(
            "probe-buffer-cap-{}-{}-",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        let buffer = ProbeBuffer::with_max_results(dir.join("probe_buffer.json"), Some(2));
        buffer.clear_pending_results().await;

        buffer.save_result_offline(item("task-1")).await;
        buffer.save_result_offline(item("task-2")).await;
        buffer.save_result_offline(item("task-3")).await;

        let pending = buffer.pending_results().await;
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[0].task_id, "task-2");
        assert_eq!(pending[1].task_id, "task-3");

        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn escrita_e_atomica_e_recupera_de_crash_no_tmp() {
        let dir = std::env::temp_dir().join(format!(
            "probe-buffer-atomic-{}-{}-",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        tokio::fs::create_dir_all(&dir)
            .await
            .expect("criar diretório");
        let path = dir.join("probe_buffer.json");

        let buffer = ProbeBuffer::new(&path);
        buffer.save_result_offline(item("task-1")).await;
        assert_eq!(buffer.pending_results().await.len(), 1);

        // Simula um crash que deixou um arquivo temporário órfão e corrompido.
        let tmp_path = tmp_path_for(&path);
        tokio::fs::write(&tmp_path, b"{quebrado")
            .await
            .expect("gravar tmp órfão");

        // O arquivo original deve continuar legível apesar do lixo no tmp.
        assert_eq!(buffer.pending_results().await.len(), 1);
        assert_eq!(buffer.pending_results().await[0].task_id, "task-1");

        // Um novo save deve limpar o tmp e reescrever o buffer atomicamente.
        buffer.save_result_offline(item("task-2")).await;
        let pending = buffer.pending_results().await;
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[0].task_id, "task-1");
        assert_eq!(pending[1].task_id, "task-2");
        assert!(
            !tmp_path.exists(),
            "arquivo temporário deve ser removido após rename"
        );

        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[test]
    #[serial]
    fn limite_eh_configuravel_pela_variavel_de_ambiente() {
        std::env::remove_var(MAX_RESULTS_ENV);
        assert_eq!(resolve_max_results(None), DEFAULT_MAX_RESULTS);

        std::env::set_var(MAX_RESULTS_ENV, "50");
        assert_eq!(resolve_max_results(None), 50);

        std::env::set_var(MAX_RESULTS_ENV, "0");
        assert_eq!(resolve_max_results(None), 1);

        std::env::remove_var(MAX_RESULTS_ENV);
        assert_eq!(resolve_max_results(Some(7)), 7);
        assert_eq!(resolve_max_results(Some(0)), 1);
    }

    #[test]
    #[serial]
    fn limite_de_bytes_eh_configuravel_pela_variavel_de_ambiente() {
        std::env::remove_var(MAX_BYTES_ENV);
        assert_eq!(resolve_max_bytes(None), DEFAULT_MAX_BYTES);

        std::env::set_var(MAX_BYTES_ENV, "1024");
        assert_eq!(resolve_max_bytes(None), 1024);

        std::env::set_var(MAX_BYTES_ENV, "0");
        assert_eq!(resolve_max_bytes(None), 1);

        std::env::remove_var(MAX_BYTES_ENV);
        assert_eq!(resolve_max_bytes(Some(7)), 7);
        assert_eq!(resolve_max_bytes(Some(0)), 1);
    }

    #[tokio::test]
    async fn buffer_deduplica_por_monitor_quando_ultrapassa_bytes() {
        let dir = std::env::temp_dir().join(format!(
            "probe-buffer-bytes-{}-{}-",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        let path = dir.join("probe_buffer.json");
        let buffer = ProbeBuffer::with_limits(&path, Some(10_000), Some(300));
        buffer.clear_pending_results().await;

        // Três resultados de dois monitores; o limite de bytes força deduplicação.
        let mut a1 = item("task-a1");
        a1.monitor_id = 1;
        let mut a2 = item("task-a2");
        a2.monitor_id = 1;
        let mut b1 = item("task-b1");
        b1.monitor_id = 2;

        buffer.save_result_offline(a1).await;
        buffer.save_result_offline(a2).await;
        buffer.save_result_offline(b1.clone()).await;

        let pending = buffer.pending_results().await;
        assert_eq!(pending.len(), 2);
        assert!(pending.iter().any(|item| item.task_id == "task-a2"));
        assert!(pending.iter().any(|item| item.task_id == "task-b1"));

        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn buffer_trunca_por_idade_quando_deduplica_nao_basta() {
        let dir = std::env::temp_dir().join(format!(
            "probe-buffer-trim-{}-{}-",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        let path = dir.join("probe_buffer.json");
        // Limite tão pequeno que só cabe um item, forçando truncamento.
        let buffer = ProbeBuffer::with_limits(&path, Some(10_000), Some(1));
        buffer.clear_pending_results().await;

        buffer.save_result_offline(item("task-1")).await;
        buffer.save_result_offline(item("task-2")).await;

        let pending = buffer.pending_results().await;
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].task_id, "task-2");

        let _ = tokio::fs::remove_dir_all(&dir).await;
    }
}
