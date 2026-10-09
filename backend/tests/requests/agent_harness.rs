//! Central e agente ligados em memória — o mesmo `connection::serve` e o mesmo
//! `agent_runtime::session::run` do socket real, sem rede no meio.

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use backend::{
    models::probes,
    services::{
        agent_runtime::{
            executor::CommandExecutor,
            outbox::Outbox,
            session::{self as agent_session, NoLiveSource, SessionDeps},
            tunnel::{parse_targets, LocalTunnelOpener, TunnelOpener},
        },
        agents::{
            connection,
            hub::AgentHub,
            policy::{Permission, Policy},
            protocol::{
                Capability, Command, ErrorCode, Hello, RemoteError, WireFrame, PROTOCOL_VERSION,
            },
            service::AgentService,
        },
    },
};
use loco_rs::app::AppContext;
use sea_orm::EntityTrait;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Cadastra um agente e troca o código pelo token.
pub async fn enrolled_agent(ctx: &AppContext, name: &str) -> (probes::Model, String) {
    let service = AgentService::new(&ctx.db);
    let (probe, code) = service
        .create(&serde_json::from_value(json!({ "name": name })).expect("entrada"))
        .await
        .expect("agente cadastrado");
    let (_, token) = service
        .enroll(&serde_json::from_value(json!({ "code": code })).expect("enroll"))
        .await
        .expect("enroll");
    (
        probes::Entity::find_by_id(probe.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap(),
        token,
    )
}

/// Liga a central a um agente em memória e espera o hub registrar a sessão.
pub async fn connect_agent_with(
    ctx: &AppContext,
    probe: probes::Model,
    hello: Hello,
    executor: Arc<dyn CommandExecutor>,
    tunnels: Arc<dyn TunnelOpener>,
) -> tokio::task::JoinHandle<()> {
    let (to_server, server_in) = mpsc::channel::<WireFrame>(64);
    let (server_out, to_agent) = mpsc::channel::<WireFrame>(64);
    let probe_id = probe.id;
    let server_ctx = ctx.clone();
    tokio::spawn(async move {
        let _ = connection::serve(server_ctx, probe, server_in, server_out).await;
    });
    let agent = tokio::spawn(async move {
        let deps = SessionDeps {
            executor,
            outbox: Arc::new(Outbox::in_memory(100)),
            live: Arc::new(NoLiveSource),
            tunnels,
        };
        let _ = agent_session::run(deps, hello, to_agent, to_server).await;
    });
    AgentHub::from_context(ctx)
        .expect("hub")
        .wait_for(probe_id, Duration::from_secs(2))
        .await
        .expect("o agente não se registrou no hub");
    agent
}

/// Apresentação de um agente atual, com a política dada.
pub fn agent_hello(policy: Vec<Permission>) -> Hello {
    Hello {
        protocol: PROTOCOL_VERSION,
        agent_version: "0.1.0".into(),
        hostname: "srv-filial".into(),
        os: "Debian".into(),
        arch: "x86_64".into(),
        in_container: false,
        policy,
        docker: Capability::default(),
        compose: Capability::default(),
    }
}

/// Executor que recusa tudo: para testes em que só a ponte importa.
pub struct NoCommands;

#[async_trait]
impl CommandExecutor for NoCommands {
    async fn execute(
        &self,
        _command: Command,
        _chunks: mpsc::UnboundedSender<Value>,
        _cancel: CancellationToken,
    ) -> Result<Value, RemoteError> {
        Err(RemoteError::new(
            ErrorCode::Unsupported,
            "sem comandos neste teste",
        ))
    }
}

/// Agente em memória que só faz ponte, liberada para `targets`.
pub async fn tunnel_agent(ctx: &AppContext, name: &str, targets: &str) -> probes::Model {
    let (probe, _) = enrolled_agent(ctx, name).await;
    connect_tunnel_agent(ctx, &probe, targets).await;
    probe
}

/// Conecta (ou reconecta) o agente de ponte de `probe`. Abortar o handle
/// derruba o canal, como uma queda de rede.
pub async fn connect_tunnel_agent(
    ctx: &AppContext,
    probe: &probes::Model,
    targets: &str,
) -> tokio::task::JoinHandle<()> {
    let policy = Policy::from_permissions([Permission::Database]);
    connect_agent_with(
        ctx,
        probe.clone(),
        agent_hello(policy.permissions()),
        Arc::new(NoCommands),
        Arc::new(LocalTunnelOpener::new(
            policy,
            parse_targets(targets).expect("destinos"),
        )),
    )
    .await
}
