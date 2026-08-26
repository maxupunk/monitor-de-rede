//! Agente do probe (§8.11).
//!
//! Roda no processo `probe`/`vpn-probe`, do outro lado da rede: heartbeat →
//! flush do buffer offline → busca tarefas → executa → reporta. Não fala com o
//! banco — só com a API HTTP —, porque o ponto do agente é justamente estar
//! onde o servidor central não alcança.

use std::time::Duration;

use chrono::Utc;
use loco_rs::app::AppContext;
use serde_json::json;

use super::{
    buffer::{BufferedDiscoveryResult, BufferedResult, ProbeBuffer},
    dispatcher::{ProbeDiscoveryTask, ProbeTask},
    DEFAULT_VPN_PROBE_TOKEN,
};
use crate::services::monitoring::{
    contracts::{CheckResult, MonitorStatus},
    runner::{run_monitor, RunOptions},
};

/// Ritmo padrão do ciclo. A janela de `PROBE_OFFLINE_AFTER_SECONDS` (90 s) é
/// dimensionada em cima deste valor.
pub const DEFAULT_INTERVAL_MS: u64 = 5_000;

const DEFAULT_SERVER_URL: &str = "http://localhost:3333";
const AGENT_VERSION: &str = "1.0.0";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Default)]
pub struct ProbeAgentOptions {
    pub server_url: Option<String>,
    pub probe_token: Option<String>,
    pub interval_ms: Option<u64>,
    pub buffer_path: Option<String>,
    pub version: Option<String>,
}

/// Precedência do token: opção da CLI,
/// `PROBE_TOKEN`, `VPN_PROBE_TOKEN` e por fim o token compartilhado do
/// vpn-probe. O último degrau é o que permite subir o container do túnel sem
/// configuração nenhuma.
#[must_use]
pub fn resolve_token(explicit: Option<&str>) -> String {
    explicit
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .or_else(|| non_empty_env("PROBE_TOKEN"))
        .or_else(|| non_empty_env("VPN_PROBE_TOKEN"))
        .unwrap_or_else(|| DEFAULT_VPN_PROBE_TOKEN.to_string())
}

/// URL do servidor central, com as duas variáveis que o compose já define.
#[must_use]
pub fn resolve_server_url(explicit: Option<&str>) -> String {
    explicit
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .or_else(|| non_empty_env("PROBE_SERVER_URL"))
        .or_else(|| non_empty_env("SERVER_URL"))
        .unwrap_or_else(|| DEFAULT_SERVER_URL.to_string())
        .trim_end_matches('/')
        .to_string()
}

fn non_empty_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub struct ProbeAgent {
    server_url: String,
    probe_token: String,
    interval: Duration,
    version: String,
    buffer: ProbeBuffer,
    client: reqwest::Client,
}

impl ProbeAgent {
    /// # Errors
    ///
    /// Falha só se o cliente HTTP não puder ser construído (TLS ausente).
    pub fn new(options: ProbeAgentOptions) -> Result<Self, reqwest::Error> {
        let interval_ms = options
            .interval_ms
            .or_else(|| {
                non_empty_env("PROBE_INTERVAL_MS").and_then(|value| value.parse::<u64>().ok())
            })
            .filter(|value| *value > 0)
            .unwrap_or(DEFAULT_INTERVAL_MS);
        Ok(Self {
            server_url: resolve_server_url(options.server_url.as_deref()),
            probe_token: resolve_token(options.probe_token.as_deref()),
            interval: Duration::from_millis(interval_ms),
            version: options.version.unwrap_or_else(|| AGENT_VERSION.to_string()),
            buffer: options
                .buffer_path
                .map_or_else(ProbeBuffer::default, ProbeBuffer::new),
            client: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()?,
        })
    }

    #[must_use]
    pub fn server_url(&self) -> &str {
        &self.server_url
    }

    /// Laço principal. Só termina por sinal do sistema — é um processo de
    /// longa duração, e não uma tarefa que roda e sai.
    pub async fn start(&self, ctx: &AppContext) {
        tracing::info!(server = %self.server_url, "agente probe inicializado");
        loop {
            self.step(ctx).await;
            tokio::time::sleep(self.interval).await;
        }
    }

    /// Um ciclo completo. Nunca devolve erro: a única resposta correta a uma
    /// falha de rede aqui é tentar de novo no próximo ciclo.
    pub async fn step(&self, ctx: &AppContext) {
        // O flush só acontece com o servidor comprovadamente no ar — reenviar
        // às cegas gastaria o timeout de cada item a cada 5 segundos.
        if self.send_heartbeat().await {
            self.flush_offline_buffer().await;
        }
        let tasks = self.fetch_tasks().await;
        for task in tasks.monitors {
            self.execute_and_report(ctx, &task).await;
        }
        for task in tasks.discovery {
            self.execute_discovery_and_report(ctx, &task).await;
        }
    }

    async fn send_heartbeat(&self) -> bool {
        let body = json!({
            "version": self.version,
            "configuration": {
                "platform": std::env::consts::OS,
                "arch": std::env::consts::ARCH,
            },
        });
        self.post("/api/probes/heartbeat", &body).await.is_some()
    }

    async fn fetch_tasks(&self) -> ProbeTaskBatch {
        let response = self
            .client
            .get(format!("{}/api/probes/tasks", self.server_url))
            .header("X-Probe-Token", &self.probe_token)
            .send()
            .await;
        let Ok(response) = response else {
            return ProbeTaskBatch::default();
        };
        if !response.status().is_success() {
            tracing::warn!(status = %response.status(), "servidor recusou a busca de tarefas");
            return ProbeTaskBatch::default();
        }
        #[derive(serde::Deserialize)]
        struct TasksEnvelope {
            #[serde(default)]
            tasks: Vec<ProbeTask>,
            #[serde(default, rename = "discoveryTasks")]
            discovery_tasks: Vec<ProbeDiscoveryTask>,
        }
        response
            .json::<TasksEnvelope>()
            .await
            .map(|envelope| ProbeTaskBatch {
                monitors: envelope.tasks,
                discovery: envelope.discovery_tasks,
            })
            .unwrap_or_default()
    }

    async fn execute_and_report(&self, ctx: &AppContext, task: &ProbeTask) {
        let result = match run_monitor(
            ctx,
            &task.task_type,
            &task.payload,
            RunOptions {
                timeout_ms: Some(task.timeout_ms.max(1) as u64),
            },
        )
        .await
        {
            Ok(result) => result,
            // Falha de execução vira observação `down` com a explicação, e não
            // silêncio: o operador precisa ver que o probe tentou.
            Err(error) => failed_result(&error.to_string()),
        };

        let payload = json!({
            "results": [{ "monitorId": task.monitor_id, "taskId": task.id, "result": result }],
        });
        if self.post("/api/probes/results", &payload).await.is_none() {
            self.buffer
                .save_result_offline(BufferedResult {
                    task_id: task.id.clone(),
                    monitor_id: task.monitor_id,
                    result: serde_json::to_value(&result).unwrap_or_else(|_| json!({})),
                    timestamp: Utc::now().to_rfc3339(),
                })
                .await;
        }
    }

    async fn execute_discovery_and_report(&self, ctx: &AppContext, task: &ProbeDiscoveryTask) {
        let cancel = tokio_util::sync::CancellationToken::new();
        let outcome = tokio::time::timeout(
            Duration::from_millis(task.timeout_ms.max(1)),
            crate::services::discovery::service::scan_network(ctx, &task.cidr, cancel),
        )
        .await;
        let (hosts, error) = match outcome {
            Ok(Ok(hosts)) => (hosts, None),
            Ok(Err(error)) => (Vec::new(), Some(error.to_string())),
            Err(_) => (
                Vec::new(),
                Some("tempo limite do discovery remoto excedido".into()),
            ),
        };
        let body = json!({
            "discoveryResults": [{
                "runId": task.run_id,
                "taskId": task.id,
                "hosts": hosts,
                "error": error,
            }],
        });
        if self.post("/api/probes/results", &body).await.is_none() {
            self.buffer
                .save_discovery_result_offline(BufferedDiscoveryResult {
                    task_id: task.id.clone(),
                    run_id: task.run_id,
                    hosts,
                    error,
                    timestamp: Utc::now().to_rfc3339(),
                })
                .await;
        }
    }

    async fn flush_offline_buffer(&self) {
        let pending = self.buffer.pending_results().await;
        let pending_discovery = self.buffer.pending_discovery_results().await;
        if pending.is_empty() && pending_discovery.is_empty() {
            return;
        }
        let results: Vec<_> = pending
            .iter()
            .map(|item| {
                json!({
                    "monitorId": item.monitor_id,
                    "taskId": item.task_id,
                    "result": item.result,
                })
            })
            .collect();
        if self
            .post(
                "/api/probes/results",
                &json!({ "results": results, "discoveryResults": pending_discovery }),
            )
            .await
            .is_some()
        {
            tracing::info!(count = pending.len(), "buffer offline do probe reenviado");
            self.buffer.clear_pending_results().await;
            self.buffer.clear_pending_discovery_results().await;
        }
    }

    /// `Some(())` quando o servidor aceitou; `None` em qualquer falha — o
    /// chamador só precisa saber se pode seguir em frente.
    async fn post(&self, path: &str, body: &serde_json::Value) -> Option<()> {
        let response = self
            .client
            .post(format!("{}{path}", self.server_url))
            .header("X-Probe-Token", &self.probe_token)
            .json(body)
            .send()
            .await
            .ok()?;
        if response.status().is_success() {
            Some(())
        } else {
            tracing::warn!(status = %response.status(), path, "servidor recusou a requisição do probe");
            None
        }
    }
}

#[derive(Default)]
struct ProbeTaskBatch {
    monitors: Vec<ProbeTask>,
    discovery: Vec<ProbeDiscoveryTask>,
}

/// Observação de falha na execução pelo probe.
fn failed_result(message: &str) -> CheckResult {
    let now = Utc::now();
    CheckResult {
        success: false,
        status: MonitorStatus::Down,
        started_at: now,
        finished_at: now,
        duration_ms: 0,
        message: Some(format!("Falha na execução pelo probe: {message}")),
        metrics: Vec::new(),
        data: json!({ "error": message }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn o_token_cai_para_o_compartilhado_do_vpn_probe() {
        std::env::remove_var("PROBE_TOKEN");
        std::env::remove_var("VPN_PROBE_TOKEN");
        assert_eq!(resolve_token(None), DEFAULT_VPN_PROBE_TOKEN);
        assert_eq!(resolve_token(Some("explicito")), "explicito");
    }

    #[test]
    #[serial]
    fn a_precedencia_do_token_respeita_cli_e_ambiente() {
        std::env::set_var("PROBE_TOKEN", "do-ambiente");
        std::env::set_var("VPN_PROBE_TOKEN", "do-vpn");
        assert_eq!(resolve_token(None), "do-ambiente");
        std::env::remove_var("PROBE_TOKEN");
        assert_eq!(resolve_token(None), "do-vpn");
        std::env::remove_var("VPN_PROBE_TOKEN");
    }

    #[test]
    #[serial]
    fn a_url_do_servidor_perde_a_barra_final() {
        std::env::remove_var("PROBE_SERVER_URL");
        std::env::remove_var("SERVER_URL");
        assert_eq!(
            resolve_server_url(Some("http://server:3333/")),
            "http://server:3333"
        );
        assert_eq!(resolve_server_url(None), DEFAULT_SERVER_URL);
    }

    #[test]
    fn falha_de_execucao_vira_observacao_e_nao_silencio() {
        let result = failed_result("conexão recusada");
        assert_eq!(result.status, MonitorStatus::Down);
        assert!(!result.success);
        assert_eq!(
            result.message.as_deref(),
            Some("Falha na execução pelo probe: conexão recusada")
        );
        assert_eq!(result.data["error"], "conexão recusada");
    }
}
