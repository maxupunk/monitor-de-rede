//! `netmonitor-agent` — o agente que roda nos servidores remotos (ADR 011).
//!
//! Não sobe o Loco nem abre banco: lê o ambiente, obtém o token (enrollment
//! na primeira vez), amostra host e containers e mantém o canal de saída com
//! a central. Ver `services::agent_runtime`.

use backend::services::agent_runtime::{client, config::AgentConfig};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_env("AGENT_LOG").unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = match AgentConfig::from_env() {
        Ok(config) => config,
        Err(error) => {
            tracing::error!(%error, "configuração inválida do agente");
            std::process::exit(2);
        }
    };
    tracing::info!(
        versao = env!("CARGO_PKG_VERSION"),
        central = %config.server_url,
        politica = ?config.policy.permissions(),
        "iniciando o agente NetMonitor"
    );
    client::run_forever(config).await;
}
