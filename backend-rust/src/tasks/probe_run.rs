//! `backend_rust-cli task probe_run` — o agente de coleta da LAN.

use loco_rs::prelude::*;

use crate::services::probes::agent::{ProbeAgent, ProbeAgentOptions};

/// Processo de longa duração: o laço só termina por sinal do sistema.
pub struct ProbeRun;

#[async_trait]
impl Task for ProbeRun {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "probe_run".into(),
            detail: "Inicia o agente do probe de rede local ou remoto".into(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &task::Vars) -> Result<()> {
        let agent = ProbeAgent::new(ProbeAgentOptions {
            server_url: vars.cli_arg("server_url").ok().map(ToString::to_string),
            probe_token: vars.cli_arg("token").ok().map(ToString::to_string),
            interval_ms: vars
                .cli_arg("interval_ms")
                .ok()
                .and_then(|value| value.parse().ok()),
            buffer_path: vars.cli_arg("buffer_path").ok().map(ToString::to_string),
            version: None,
        })
        .map_err(|error| Error::Message(format!("agente do probe não pôde iniciar: {error}")))?;

        tracing::info!(server = agent.server_url(), "processo probe inicializado");
        agent.start(ctx).await;
        Ok(())
    }
}
