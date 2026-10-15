//! Contrato HTTP do gerenciador Docker (ADR 014): toda rota é por host, e o
//! host é um agente — inclusive o desta central. Os testes ligam um agente em
//! memória cuja Engine não responde, para fixar o contrato sem Docker.

use std::sync::Arc;

use async_trait::async_trait;
use backend::{
    app::App,
    models::{probes, users},
    services::{
        agent_runtime::{executor::CommandExecutor, tunnel::TunnelOpener},
        agents::{
            policy::Permission,
            protocol::{Command, ErrorCode, RemoteError},
            tunnel::TunnelStream,
        },
        users::Role,
    },
};
use loco_rs::{app::AppContext, testing::prelude::*};
use sea_orm::{ActiveModelTrait, IntoActiveModel, Set};
use serde_json::Value;
use serial_test::serial;
use tokio::{io::AsyncReadExt, sync::mpsc};
use tokio_util::sync::CancellationToken;

use super::{
    agent_harness::{agent_hello, connect_agent_with, enrolled_agent},
    prepare_data,
};

async fn session_with_role(
    request: &mut loco_rs::TestServer,
    ctx: &loco_rs::app::AppContext,
    role: Role,
) -> users::Model {
    let session = prepare_data::init_operator(ctx).await;
    let mut active = session.user.into_active_model();
    active.role = Set(role.as_str().to_string());
    let user = active.update(&ctx.db).await.unwrap();
    let (header, value) = prepare_data::auth_header(&session.token);
    request.add_header(header, value);
    user
}

/// Agente cujo host não tem Docker: toda chamada responde "indisponível".
struct EngineDown;

#[async_trait]
impl CommandExecutor for EngineDown {
    async fn execute(
        &self,
        _command: Command,
        _chunks: mpsc::UnboundedSender<Value>,
        _cancel: CancellationToken,
    ) -> Result<Value, RemoteError> {
        Err(RemoteError::new(
            ErrorCode::Unavailable,
            "Docker Engine indisponível",
        ))
    }
}

/// O conteúdo de qualquer volume, para o teste da exportação.
const VOLUME_BYTES: &[u8] = b"conteudo do volume em tar";

/// Ponte de mentira: o volume é um buffer em memória.
struct FakeVolumes;

#[async_trait]
impl TunnelOpener for FakeVolumes {
    async fn open(&self, _host: &str, _port: u16) -> Result<TunnelStream, RemoteError> {
        Err(RemoteError::new(ErrorCode::Unsupported, "sem banco"))
    }

    async fn open_volume(&self, name: &str) -> Result<TunnelStream, RemoteError> {
        if name != "dados" {
            return Err(RemoteError::new(ErrorCode::NotFound, "volume inexistente"));
        }
        Ok(Box::new(tokio::io::join(VOLUME_BYTES, tokio::io::sink())))
    }
}

/// O host Docker de um agente em memória com a política dada.
async fn docker_host(ctx: &AppContext, policy: Vec<Permission>) -> (probes::Model, String) {
    let (probe, _) = enrolled_agent(ctx, "srv-docker").await;
    connect_agent_with(
        ctx,
        probe.clone(),
        agent_hello(policy),
        Arc::new(EngineDown),
        Arc::new(FakeVolumes),
    )
    .await;
    let base = format!("/api/docker/hosts/agent-{}", probe.id);
    (probe, base)
}

fn read_only() -> Vec<Permission> {
    vec![Permission::Read, Permission::Lifecycle]
}

#[tokio::test]
#[serial]
async fn rotas_exigem_sessao_e_um_host() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        assert_eq!(request.get("/api/docker/hosts").await.status_code(), 401);

        session_with_role(&mut request, &ctx, Role::Viewer).await;
        let hosts = request.get("/api/docker/hosts").await;
        assert_eq!(hosts.status_code(), 200, "{}", hosts.text());
        // A central não tem mais rota sem host nem o alias `local`.
        assert_eq!(request.get("/api/docker/status").await.status_code(), 404);
        assert_eq!(
            request
                .get("/api/docker/hosts/local/status")
                .await
                .status_code(),
            422
        );
        let offline = request.get("/api/docker/hosts/agent-999/status").await;
        assert_eq!(offline.status_code(), 503, "{}", offline.text());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn status_e_listagens_mantem_contrato_sem_engine() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let (_, base) = docker_host(&ctx, read_only()).await;
        session_with_role(&mut request, &ctx, Role::Viewer).await;

        let status = request.get(&format!("{base}/status")).await;
        assert_eq!(status.status_code(), 200, "{}", status.text());
        let body: Value = serde_json::from_str(&status.text()).unwrap();
        assert_eq!(body["available"], false);
        assert!(body.get("reason").is_some());

        for path in ["/containers", "/volumes", "/networks", "/images"] {
            let response = request.get(&format!("{base}{path}")).await;
            assert_eq!(response.status_code(), 200, "{path}: {}", response.text());
            let body: Value = serde_json::from_str(&response.text()).unwrap();
            assert_eq!(body["available"], false, "{path}: {body}");
            assert!(
                body["data"].as_array().unwrap().is_empty(),
                "{path}: {body}"
            );
        }

        let metrics = request.get(&format!("{base}/metrics")).await;
        assert_eq!(metrics.status_code(), 200, "{}", metrics.text());
        let body: Value = serde_json::from_str(&metrics.text()).unwrap();
        assert_eq!(body["dockerAvailable"], false);
        assert!(body["containers"].is_array());
        assert!(body["collectedAt"].is_string());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn somente_administrador_controla_a_engine() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let (_, base) = docker_host(&ctx, read_only()).await;
        session_with_role(&mut request, &ctx, Role::Operator).await;
        let response = request
            .post(&format!("{base}/networks"))
            .json(&serde_json::json!({ "name": "rede-teste" }))
            .await;
        assert_eq!(response.status_code(), 403, "{}", response.text());

        let export = request.get(&format!("{base}/volumes/dados/export")).await;
        assert_eq!(export.status_code(), 403, "{}", export.text());

        let clear_logs = request
            .delete(&format!("{base}/containers/exemplo/logs"))
            .await;
        assert_eq!(clear_logs.status_code(), 403, "{}", clear_logs.text());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn entradas_invalidas_falham_antes_de_tocar_na_engine() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let (_, base) = docker_host(&ctx, read_only()).await;
        session_with_role(&mut request, &ctx, Role::Admin).await;

        let network = request
            .post(&format!("{base}/networks"))
            .json(&serde_json::json!({ "name": "", "driver": "bridge" }))
            .await;
        assert_eq!(network.status_code(), 422, "{}", network.text());

        let logs = request
            .get(&format!("{base}/containers/exemplo/logs?tail=10001"))
            .await;
        assert_eq!(logs.status_code(), 422, "{}", logs.text());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn exportacao_de_volume_atravessa_a_ponte_do_agente() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let (_, base) = docker_host(&ctx, vec![Permission::Read, Permission::VolumeExport]).await;
        session_with_role(&mut request, &ctx, Role::Admin).await;

        let export = request.get(&format!("{base}/volumes/dados/export")).await;
        assert_eq!(export.status_code(), 200, "{}", export.text());
        let disposition = export
            .header("content-disposition")
            .to_str()
            .unwrap()
            .to_string();
        assert!(disposition.contains("volume-dados-"), "{disposition}");
        let mut unpacked = Vec::new();
        async_compression::tokio::bufread::GzipDecoder::new(export.as_bytes().as_ref())
            .read_to_end(&mut unpacked)
            .await
            .expect("gzip válido");
        assert_eq!(unpacked, VOLUME_BYTES, "o tar chega idêntico");

        // Volume inexistente: a recusa do agente chega como 404.
        let missing = request.get(&format!("{base}/volumes/outro/export")).await;
        assert_eq!(missing.status_code(), 404, "{}", missing.text());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn exportacao_respeita_a_politica_anunciada_pelo_agente() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let (_, base) = docker_host(&ctx, read_only()).await;
        session_with_role(&mut request, &ctx, Role::Admin).await;

        let export = request.get(&format!("{base}/volumes/dados/export")).await;
        assert_eq!(export.status_code(), 403, "{}", export.text());
        assert!(export.text().contains("volume_export"), "{}", export.text());
    })
    .await;
}
