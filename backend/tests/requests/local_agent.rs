//! O agente local (ADR 014): o Docker desta central atendido por um agente
//! ao lado dela. Cadastro zero-config pelo arquivo do token, adoção do que
//! era da central e a guarda do ciclo de vida.

use backend::{
    app::App,
    models::{monitors, users},
    services::{
        agents::{local, service::AgentService},
        monitoring::creation,
        shared::secret_file,
        telemetry::{
            rollup::{HostRollup, MetricsRollup},
            store,
        },
        users::Role,
    },
    views::telemetry::MetricsRange,
};
use chrono::Utc;
use loco_rs::{app::AppContext, testing::prelude::*};
use sea_orm::{ActiveModelTrait, EntityTrait, IntoActiveModel, Set};
use serde_json::{json, Value};
use serial_test::serial;

use super::{agent_harness::enrolled_agent, prepare_data};

async fn session_with_role(
    request: &mut loco_rs::TestServer,
    ctx: &AppContext,
    role: Role,
) -> users::Model {
    let session = prepare_data::init_operator(ctx).await;
    let mut active = session.user.into_active_model();
    active.role = Set(role.as_str().to_string());
    let user = active.update(&ctx.db).await.expect("papel");
    let (header, value) = prepare_data::auth_header(&session.token);
    request.add_header(header, value);
    user
}

fn container_monitor(name: &str) -> backend::dtos::resources::MonitorInput {
    serde_json::from_value(json!({ "type": "container", "name": name, "target": name }))
        .expect("entrada")
}

#[tokio::test]
#[serial]
async fn token_vai_ao_arquivo_e_sobrevive_ao_reboot() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let dir = tempfile::tempdir().expect("diretório");
        let path = dir.path().join("token");
        let service = AgentService::new(&ctx.db);

        let probe = local::ensure(&ctx.db, &path).await.expect("agente local");
        assert!(local::is_local(&probe));
        assert_eq!(probe.name, local::LOCAL_AGENT_NAME);
        let token = secret_file::read(&path).expect("token gravado");
        assert_eq!(
            service
                .authenticate(&token, None)
                .await
                .expect("autentica")
                .id,
            probe.id
        );

        // Reboot da API: mesmo cadastro e mesmo token — o agente não cai.
        let again = local::ensure(&ctx.db, &path).await.expect("de novo");
        assert_eq!(again.id, probe.id);
        assert_eq!(secret_file::read(&path).as_deref(), Some(token.as_str()));

        // Volume perdido: token novo, e o antigo deixa de valer.
        std::fs::remove_file(&path).expect("remove");
        let rotated = local::ensure(&ctx.db, &path).await.expect("rotaciona");
        assert_eq!(rotated.id, probe.id);
        let fresh = secret_file::read(&path).expect("token novo");
        assert_ne!(fresh, token);
        assert!(service.authenticate(&token, None).await.is_err());
        assert!(service.authenticate(&fresh, None).await.is_ok());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn adota_monitores_de_container_e_o_historico_da_central() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let before = creation::create(&ctx.db, container_monitor("web"))
            .await
            .expect("monitor");
        assert_eq!(before.probe_id, None, "sem agente local, sem probe");
        let ping = creation::create(
            &ctx.db,
            serde_json::from_value(json!({ "type": "ping", "name": "gw", "target": "127.0.0.1" }))
                .expect("entrada"),
        )
        .await
        .expect("ping");
        let bucket = Utc::now() - chrono::Duration::minutes(2);
        store::save(
            &ctx.db,
            "local",
            &MetricsRollup {
                bucket_at: bucket,
                samples: 6,
                host: Some(HostRollup {
                    cpu_avg: 12.0,
                    cpu_max: 20.0,
                    memory_used_avg: 1_000.0,
                    memory_total: 4_000,
                    load1: 0.5,
                    net_rx_bps: 10.0,
                    net_tx_bps: 20.0,
                    disks: Vec::new(),
                }),
                containers: Vec::new(),
            },
        )
        .await
        .expect("histórico antigo");

        let dir = tempfile::tempdir().expect("diretório");
        let probe = local::ensure(&ctx.db, &dir.path().join("token"))
            .await
            .expect("agente local");

        let adopted = monitors::Entity::find_by_id(before.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(adopted.probe_id, Some(probe.id));
        let untouched = monitors::Entity::find_by_id(ping.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(untouched.probe_id, None, "só container é do Docker");

        let after = creation::create(&ctx.db, container_monitor("api"))
            .await
            .expect("monitor novo");
        assert_eq!(after.probe_id, Some(probe.id), "nasce no agente local");

        let key = format!("agent-{}", probe.id);
        let history = store::host_history(&ctx.db, &key, MetricsRange::default(), Utc::now())
            .await
            .expect("histórico");
        assert!(!history.is_empty(), "o histórico `local` passou ao agente");
        assert!(
            store::host_history(&ctx.db, "local", MetricsRange::default(), Utc::now())
                .await
                .expect("legado")
                .is_empty()
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn agente_local_e_o_primeiro_host_e_nao_sai_pela_tela() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let (remote, _) = enrolled_agent(&ctx, "aaa-remoto").await;
        let dir = tempfile::tempdir().expect("diretório");
        let probe = local::ensure(&ctx.db, &dir.path().join("token"))
            .await
            .expect("agente local");
        session_with_role(&mut request, &ctx, Role::Admin).await;

        let hosts: Value =
            serde_json::from_str(&request.get("/api/docker/hosts").await.text()).unwrap();
        let hosts = hosts.as_array().expect("lista");
        assert_eq!(hosts[0]["key"], format!("agent-{}", probe.id));
        assert_eq!(hosts[0]["kind"], "local");
        assert_eq!(hosts[1]["key"], format!("agent-{}", remote.id));
        assert_eq!(hosts[1]["kind"], "agent");

        let agents: Value = serde_json::from_str(&request.get("/api/agents").await.text()).unwrap();
        let listed = agents
            .as_array()
            .unwrap()
            .iter()
            .find(|agent| agent["id"] == probe.id)
            .expect("agente local na lista");
        assert_eq!(listed["local"], true);

        for response in [
            request
                .post(&format!("/api/agents/{}/revoke", probe.id))
                .await,
            request.delete(&format!("/api/agents/{}", probe.id)).await,
            request
                .post(&format!("/api/agents/{}/enrollment", probe.id))
                .await,
        ] {
            assert_eq!(response.status_code(), 400, "{}", response.text());
            assert!(
                response.text().contains("agente local"),
                "{}",
                response.text()
            );
        }
        // Renomear continua livre.
        let renamed = request
            .put(&format!("/api/agents/{}", probe.id))
            .json(&json!({ "name": "Docker da central" }))
            .await;
        assert_eq!(renamed.status_code(), 200, "{}", renamed.text());
    })
    .await;
}
