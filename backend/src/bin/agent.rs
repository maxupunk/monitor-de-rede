//! `netmonitor-agent` — o agente que roda nos servidores remotos (ADR 011).
//!
//! Não sobe o Loco nem abre banco: lê o ambiente, obtém o token (enrollment
//! na primeira vez), amostra host e containers e mantém o canal de saída com
//! a central. Ver `services::agent_runtime`.

use backend::registries;

use netmonitor_core::{
    agents::targets::DeviceNetworks, monitoring::CheckDeps,
    network_tools::ping_client::PingClientCell,
};
use netmonitor_nucleo::services::agent_runtime::{
    client::{self, AgentHost},
    config::AgentConfig,
};
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
    // As peças das áreas (ponto de montagem do agente): os tipos de monitor
    // com a Engine local, os comandos e o Docker deste host.
    let deps = CheckDeps {
        ping: Some(PingClientCell::open()),
        checkers: registries::agent_monitor_checkers(),
    };
    let host = AgentHost {
        handlers: registries::agent_command_handlers(&deps, DeviceNetworks::from_env()),
        docker: registries::agent_host_docker(),
        deps,
    };
    let shutdown = tokio_util::sync::CancellationToken::new();
    let signal_token = shutdown.clone();
    tokio::spawn(async move {
        wait_for_shutdown_signal().await;
        tracing::info!("sinal de encerramento recebido; finalizando o agente");
        signal_token.cancel();
    });

    client::run_until_shutdown(config, host, shutdown).await;
}

async fn wait_for_shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut term = match signal(SignalKind::terminate()) {
            Ok(sig) => Some(sig),
            Err(error) => {
                tracing::warn!(%error, "não foi possível registrar SIGTERM");
                None
            }
        };
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = async {
                if let Some(sig) = term.as_mut() {
                    sig.recv().await;
                } else {
                    std::future::pending::<()>().await;
                }
            } => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
