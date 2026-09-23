//! Agentes remotos (ADR 011): cadastro, enrollment, canal, Docker por host,
//! pump de tarefas e histórico — com a central e um agente em memória.

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use backend::{
    app::App,
    models::{devices, monitor_results, monitors, probes, users},
    services::{
        agent_runtime::{
            executor::CommandExecutor,
            outbox::Outbox,
            session::{self as agent_session, NoLiveSource, SessionDeps},
        },
        agents::{
            background, connection,
            hub::AgentHub,
            inbound,
            policy::Permission,
            protocol::{
                AgentEvent, Capability, Command, DockerCall, Hello, RemoteError, PROTOCOL_VERSION,
            },
            service::AgentService,
        },
        docker::{
            engine,
            hosts::{self, HostKey},
        },
        events::EventBus,
        probes::{dispatcher, DEFAULT_VPN_PROBE_TOKEN},
        shared::crypto::sha256_hex,
        telemetry::rollup::{ContainerRollup, HostRollup, MetricsRollup},
        users::Role,
    },
    views::docker::{DockerLiveSnapshot, DockerStatusResponse},
};
use chrono::Utc;
use loco_rs::{app::AppContext, testing::prelude::*};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, Set};
use serde_json::{json, Value};
use serial_test::serial;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::prepare_data;

fn json_of(text: &str) -> Value {
    serde_json::from_str(text).expect("resposta JSON")
}

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

/// Executor falso do lado do agente: responde como um host com dois
/// containers e mede monitores sempre `up`.
struct FakeExecutor;

#[async_trait]
impl CommandExecutor for FakeExecutor {
    async fn execute(
        &self,
        command: Command,
        _chunks: mpsc::UnboundedSender<Value>,
        _cancel: CancellationToken,
    ) -> Result<Value, RemoteError> {
        Ok(match command {
            Command::Docker {
                call: DockerCall::ListContainers,
            } => json!([
                { "Id": "b", "Names": ["/web"], "State": "running", "Labels": {} },
                { "Id": "a", "Names": ["/api"], "State": "exited", "Labels": {} }
            ]),
            Command::Docker {
                call: DockerCall::Status,
            } => json!({
                "version": { "Version": "27.1.0" },
                "info": { "Name": "srv-remoto", "NCPU": 8 }
            }),
            Command::Monitor { .. } => json!({
                "success": true, "status": "up",
                "startedAt": Utc::now(), "finishedAt": Utc::now(),
                "durationMs": 3, "message": "ok", "metrics": [], "data": {}
            }),
            _ => Value::Null,
        })
    }
}

fn hello() -> Hello {
    Hello {
        protocol: PROTOCOL_VERSION,
        agent_version: "0.1.0".into(),
        hostname: "srv-remoto".into(),
        os: "Debian".into(),
        arch: "x86_64".into(),
        in_container: false,
        policy: vec![Permission::Read, Permission::Lifecycle, Permission::Monitor],
        docker: Capability {
            available: true,
            version: Some("27.1.0".into()),
            reason: None,
        },
        compose: Capability::default(),
    }
}

/// Cadastra um agente e troca o código pelo token.
async fn enrolled_agent(ctx: &AppContext, name: &str) -> (probes::Model, String) {
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

/// Liga a central (`connection::serve`) a um agente em memória
/// (`agent_session::run`) e espera o hub registrar a sessão.
async fn connect_agent(ctx: &AppContext, probe: probes::Model) -> tokio::task::JoinHandle<()> {
    let (to_server, server_in) = mpsc::channel::<String>(64);
    let (server_out, to_agent) = mpsc::channel::<String>(64);
    let probe_id = probe.id;
    let server_ctx = ctx.clone();
    tokio::spawn(async move {
        let _ = connection::serve(server_ctx, probe, server_in, server_out).await;
    });
    let agent = tokio::spawn(async move {
        let deps = SessionDeps {
            executor: Arc::new(FakeExecutor),
            outbox: Arc::new(Outbox::in_memory(100)),
            live: Arc::new(NoLiveSource),
        };
        let _ = agent_session::run(deps, hello(), to_agent, to_server).await;
    });
    let hub = AgentHub::from_context(ctx).expect("hub");
    for _ in 0..100 {
        if hub.get(probe_id).is_some() {
            return agent;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("o agente não se registrou no hub");
}

#[tokio::test]
#[serial]
async fn cadastro_emite_codigo_de_uso_unico_trocado_por_token() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Admin).await;
        let created = request
            .post("/api/agents")
            .json(&json!({ "name": "srv-01" }))
            .await;
        assert_eq!(created.status_code(), 201, "{}", created.text());
        let body = json_of(&created.text());
        let code = body["enrollment"]["code"]
            .as_str()
            .expect("código")
            .to_string();
        assert!(code.starts_with("nma_"));
        assert!(body["enrollment"]["commands"]["dockerCommand"]
            .as_str()
            .unwrap()
            .contains(&code));
        assert!(body["enrollment"]["commands"]["systemdCommand"]
            .as_str()
            .unwrap()
            .contains("install.sh"));
        assert_eq!(body["agent"]["status"], "pending");
        assert_eq!(
            body["agent"]["hostKey"],
            format!("agent-{}", body["agent"]["id"])
        );

        let enrolled = request
            .post("/api/agents/enroll")
            .json(&json!({ "code": code }))
            .await;
        assert_eq!(enrolled.status_code(), 200, "{}", enrolled.text());
        let token = json_of(&enrolled.text())["token"]
            .as_str()
            .expect("token")
            .to_string();
        let id = body["agent"]["id"].as_i64().expect("id");
        let saved = probes::Entity::find_by_id(id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            saved.token_hash,
            sha256_hex(&token),
            "o banco guarda só o hash"
        );

        let again = request
            .post("/api/agents/enroll")
            .json(&json!({ "code": code }))
            .await;
        assert_eq!(again.status_code(), 401, "código de uso único");

        let listed = json_of(&request.get("/api/agents").await.text());
        assert_eq!(listed.as_array().map(Vec::len), Some(1));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn comandos_de_instalacao_usam_os_enderecos_deste_servidor() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Admin).await;
        let saved = request
            .put("/api/server-addresses")
            .json(&json!({
                "overrides": { "public": "monitor.exemplo.com" },
                "custom": [{ "label": "Filial", "value": "172.20.0.5:9000" }],
                "preferredId": "public"
            }))
            .await;
        assert_eq!(saved.status_code(), 200, "{}", saved.text());
        let custom_id = json_of(&saved.text())["data"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["kind"] == "custom")
            .and_then(|entry| entry["id"].as_str())
            .expect("id do personalizado")
            .to_string();

        // Sem dispositivo vinculado, o padrão é o preferido do operador — e
        // não a origem de quem abriu a tela.
        let created = json_of(
            &request
                .post("/api/agents")
                .json(&json!({ "name": "srv-dns" }))
                .await
                .text(),
        );
        let commands = &created["enrollment"]["commands"];
        assert_eq!(commands["addressId"], "public");
        assert!(commands["serverUrl"]
            .as_str()
            .unwrap()
            .starts_with("http://monitor.exemplo.com:"));
        let code = created["enrollment"]["code"].as_str().unwrap().to_string();

        let other = request
            .post("/api/agents/install-commands")
            .json(&json!({ "code": code, "addressId": custom_id }))
            .await;
        assert_eq!(other.status_code(), 200, "{}", other.text());
        let other = json_of(&other.text());
        assert_eq!(
            other["serverUrl"], "http://172.20.0.5:9000",
            "a porta digitada vale"
        );
        assert!(other["systemdCommand"]
            .as_str()
            .unwrap()
            .contains("AGENT_SERVER_URL=http://172.20.0.5:9000"));

        let bad_code = request
            .post("/api/agents/install-commands")
            .json(&json!({ "code": "nma_x; rm -rf /", "addressId": "public" }))
            .await;
        assert_eq!(bad_code.status_code(), 422);
        let unknown = request
            .post("/api/agents/install-commands")
            .json(&json!({ "code": code, "addressId": "inexistente" }))
            .await;
        assert_eq!(unknown.status_code(), 422);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn so_admin_cadastra_ou_revoga_agentes() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Operator).await;
        assert_eq!(request.get("/api/agents").await.status_code(), 200);
        let denied = request
            .post("/api/agents")
            .json(&json!({ "name": "x" }))
            .await;
        assert_eq!(denied.status_code(), 403);
        let (probe, _) = enrolled_agent(&ctx, "srv-02").await;
        let revoke = request
            .post(&format!("/api/agents/{}/revoke", probe.id))
            .await;
        assert_eq!(revoke.status_code(), 403);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn canal_recusa_token_padrao_e_origem_fora_do_tunel() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let service = AgentService::new(&ctx.db);
        // Probe comum com o token compartilhado: vale no HTTP legado, nunca no canal.
        probes::ActiveModel {
            name: Set("probe-lan".into()),
            token_hash: Set(sha256_hex(DEFAULT_VPN_PROBE_TOKEN)),
            status: Set("pending".into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();
        assert!(service
            .authenticate(DEFAULT_VPN_PROBE_TOKEN, None)
            .await
            .is_err());

        let device = devices::ActiveModel {
            name: Set("srv-vpn".into()),
            r#type: Set("server".into()),
            ip_address: Set(Some("10.8.0.9".into())),
            access_mode: Set(Some("vpn".into())),
            status: Set("unknown".into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();
        let (probe, token) = enrolled_agent(&ctx, "srv-vpn").await;
        let mut active = probe.into_active_model();
        active.device_id = Set(Some(device.id));
        active.update(&ctx.db).await.unwrap();

        let tunnel = "10.8.0.9".parse().ok();
        let outside = "200.10.20.30".parse().ok();
        assert!(service.authenticate(&token, tunnel).await.is_ok());
        assert!(service.authenticate(&token, outside).await.is_err());
        assert!(service.authenticate("errado", tunnel).await.is_err());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn docker_do_host_remoto_atravessa_o_canal_com_o_mesmo_mapeamento() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let (probe, _) = enrolled_agent(&ctx, "srv-remoto").await;
        let agent = connect_agent(&ctx, probe.clone()).await;

        let saved = probes::Entity::find_by_id(probe.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(saved.status, "online");
        assert_eq!(saved.version.as_deref(), Some("0.1.0"));

        let host = hosts::resolve(&ctx, HostKey::agent(probe.id)).expect("host remoto");
        let containers = engine::list_containers(host.engine.as_ref())
            .await
            .expect("lista");
        assert_eq!(containers.len(), 2);
        assert_eq!(
            containers[0].names,
            vec!["/api"],
            "ordenados pelo mapeamento central"
        );
        let status = engine::status(host.engine.as_ref()).await;
        assert!(status.available);
        assert_eq!(status.engine_version.as_deref(), Some("27.1.0"));

        session_with_role(&mut request, &ctx, Role::Viewer).await;
        let path = format!("/api/docker/hosts/agent-{}/containers", probe.id);
        let response = request.get(&path).await;
        assert_eq!(response.status_code(), 200, "{}", response.text());
        assert_eq!(
            json_of(&response.text())["data"].as_array().map(Vec::len),
            Some(2)
        );

        let hosts_list = json_of(&request.get("/api/docker/hosts").await.text());
        let remote = hosts_list
            .as_array()
            .unwrap()
            .iter()
            .find(|host| host["key"] == format!("agent-{}", probe.id))
            .expect("host do agente na lista");
        assert_eq!(remote["online"], true);
        assert_eq!(hosts_list[0]["key"], "local");

        // Política anunciada barra antes de enviar: update não foi liberado.
        let forbidden = request
            .post(&format!(
                "/api/docker/hosts/agent-{}/containers/web/stop",
                probe.id
            ))
            .await;
        assert_eq!(forbidden.status_code(), 403, "viewer não muta");

        agent.abort();
        for _ in 0..100 {
            let current = probes::Entity::find_by_id(probe.id)
                .one(&ctx.db)
                .await
                .unwrap()
                .unwrap();
            if current.status == "offline" {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("a queda do canal não marcou o agente offline");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn rotas_por_host_validam_a_chave_e_a_conexao() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Viewer).await;
        assert_eq!(
            request
                .get("/api/docker/hosts/local/status")
                .await
                .status_code(),
            200
        );
        assert_eq!(
            request
                .get("/api/docker/hosts/remoto/status")
                .await
                .status_code(),
            422
        );
        let offline = request.get("/api/docker/hosts/agent-999/containers").await;
        assert_eq!(offline.status_code(), 503, "{}", offline.text());
        // Histórico é do banco: responde mesmo sem o agente conectado.
        let history = request
            .get("/api/docker/hosts/agent-999/history?range=24h")
            .await;
        assert_eq!(history.status_code(), 200, "{}", history.text());
        assert_eq!(json_of(&history.text())["stepMinutes"], 5);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn pump_entrega_o_monitor_ao_agente_e_o_resultado_volta_com_o_probe() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let (probe, _) = enrolled_agent(&ctx, "srv-monitor").await;
        let _agent = connect_agent(&ctx, probe.clone()).await;
        let monitor = monitors::ActiveModel {
            probe_id: Set(Some(probe.id)),
            r#type: Set("ping".into()),
            name: Set("gateway do site".into()),
            configuration: Set(json!({ "host": "127.0.0.1" })),
            interval_seconds: Set(60),
            timeout_seconds: Set(5),
            retry_count: Set(1),
            enabled: Set(true),
            status: Set("unknown".into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();
        dispatcher::dispatch_task(
            &ctx.db,
            probe.id,
            &dispatcher::ProbeTask {
                id: dispatcher::task_id(monitor.id, Utc::now()),
                monitor_id: monitor.id,
                task_type: "ping".into(),
                timeout_ms: 2_000,
                payload: json!({ "host": "127.0.0.1" }),
            },
        )
        .await
        .unwrap();

        let session = AgentHub::from_context(&ctx)
            .unwrap()
            .get(probe.id)
            .expect("sessão");
        background::pump_session(&ctx, &session)
            .await
            .expect("pump");
        for _ in 0..100 {
            let results = monitor_results::Entity::find()
                .filter(monitor_results::Column::MonitorId.eq(monitor.id))
                .all(&ctx.db)
                .await
                .unwrap();
            if let Some(result) = results.first() {
                assert_eq!(result.probe_id, Some(probe.id));
                assert_eq!(result.status, "up");
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("o resultado do agente não chegou");
    })
    .await;
}

fn rollup(bucket_at: chrono::DateTime<Utc>, cpu: f64) -> MetricsRollup {
    MetricsRollup {
        bucket_at,
        samples: 6,
        host: Some(HostRollup {
            cpu_avg: cpu,
            cpu_max: cpu + 10.0,
            memory_used_avg: 1_000.0,
            memory_total: 4_000,
            load1: 0.5,
            net_rx_bps: 10.0,
            net_tx_bps: 20.0,
            disks: Vec::new(),
        }),
        containers: vec![ContainerRollup {
            name: "web".into(),
            project: Some("portal".into()),
            cpu_avg: cpu / 2.0,
            cpu_max: cpu,
            memory_avg: 100.0,
            memory_max: 200,
            net_rx_bytes: 300,
            net_tx_bytes: 400,
            block_read_bytes: 0,
            block_write_bytes: 0,
        }],
    }
}

#[tokio::test]
#[serial]
async fn rollup_do_agente_vira_historico_e_reenvio_nao_duplica() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let (probe, _) = enrolled_agent(&ctx, "srv-historico").await;
        let bucket = backend::services::telemetry::rollup::bucket_of(Utc::now());
        for cpu in [40.0, 60.0] {
            inbound::handle(
                &ctx,
                probe.id,
                AgentEvent::MetricsRollup {
                    rollup: Box::new(rollup(bucket, cpu)),
                },
            )
            .await;
        }
        session_with_role(&mut request, &ctx, Role::Viewer).await;
        let history = json_of(
            &request
                .get(&format!(
                    "/api/docker/hosts/agent-{}/history?range=1h",
                    probe.id
                ))
                .await
                .text(),
        );
        let points = history["host"].as_array().expect("série");
        assert_eq!(
            points.len(),
            1,
            "o mesmo minuto é substituído, não duplicado"
        );
        assert_eq!(points[0]["cpuAvg"], 60.0);
        assert_eq!(history["containers"][0]["name"], "web");
        assert_eq!(history["containers"][0]["netRxBytes"], 300);

        let container = json_of(
            &request
                .get(&format!(
                    "/api/docker/hosts/agent-{}/history/containers/web?range=1h",
                    probe.id
                ))
                .await
                .text(),
        );
        assert_eq!(container["points"].as_array().map(Vec::len), Some(1));

        // Remover o agente leva o histórico junto.
        AgentService::new(&ctx.db)
            .delete(probe.id)
            .await
            .expect("remoção");
        let empty = json_of(
            &request
                .get(&format!(
                    "/api/docker/hosts/agent-{}/history?range=1h",
                    probe.id
                ))
                .await
                .text(),
        );
        assert!(empty["host"].as_array().unwrap().is_empty());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn snapshot_do_agente_fica_guardado_para_quem_assinar_depois() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let snapshot = DockerLiveSnapshot {
            host_key: "local".into(),
            status: DockerStatusResponse {
                available: true,
                reason: None,
                engine_version: None,
                api_version: None,
                name: None,
                operating_system: None,
                architecture: None,
                cpus: None,
                memory_total_bytes: None,
                containers: None,
                containers_running: None,
                containers_stopped: None,
                images: None,
            },
            containers: Vec::new(),
            metrics: serde_json::from_value(json!({
                "dockerAvailable": true, "unavailableReason": null,
                "failedContainerCount": 0, "collectedAt": "", "containers": []
            }))
            .unwrap(),
        };
        inbound::handle(
            &ctx,
            77,
            AgentEvent::DockerLive {
                snapshot: Box::new(snapshot),
            },
        )
        .await;
        let bus = EventBus::from_context(&ctx).unwrap();
        let stored = bus
            .current_snapshots()
            .into_iter()
            .find(|event| event.event_type == "docker:snapshot")
            .expect("snapshot guardado");
        assert_eq!(
            stored.payload["hostKey"], "agent-77",
            "a central carimba a chave do host"
        );
        bus.forget_snapshots("agent-77");
        assert!(bus
            .current_snapshots()
            .iter()
            .all(|event| event.payload["hostKey"] != "agent-77"));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn peer_da_vpn_com_agente_leva_a_instalacao_no_script_linux() {
    let dir = std::env::temp_dir().join(format!("wg-agente-{}", std::process::id()));
    std::env::set_var("WG_CONFIG_DIR", &dir);
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Admin).await;
        request
            .put("/api/vpn/server")
            .json(&json!({ "cidr": "10.8.0.0/24", "publicEndpoint": "vpn.exemplo.com.br" }))
            .await;
        let created = request
            .post("/api/vpn/peers")
            .json(&json!({ "name": "srv-docker", "profile": "linux", "installAgent": true }))
            .await;
        assert_eq!(created.status_code(), 201, "{}", created.text());
        let body = json_of(&created.text());
        let device_id = body["device"]["id"].as_i64().expect("dispositivo");

        let script = serde_json::to_string(&body["artifact"]).expect("artefato");
        assert!(
            script.contains("/api/agents/install.sh"),
            "o script instala o agente"
        );
        assert!(
            script.contains("AGENT_ENROLL_CODE='nma_"),
            "com código de uso único"
        );
        assert!(
            script.contains("http://10.8.0.1:"),
            "pela central dentro do túnel"
        );

        let agents = json_of(&request.get("/api/agents").await.text());
        let agent = &agents.as_array().expect("lista")[0];
        assert_eq!(agent["deviceId"], device_id);
        assert_eq!(agent["enforceTunnelIp"], true);
        assert_eq!(agent["deviceIp"], "10.8.0.2");
    })
    .await;
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
#[serial]
async fn compose_na_propria_central_e_aceito_e_falha_pelo_sse() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Admin).await;
        let response = request
            .post("/api/docker/compose/portal")
            .json(&json!({ "action": "up" }))
            .await;
        assert_eq!(response.status_code(), 202, "{}", response.text());
        let accepted = json_of(&response.text());
        assert_eq!(accepted["kind"], "compose");
        assert_eq!(accepted["hostKey"], "local");
        assert!(accepted["operationId"].is_string());
    })
    .await;
}
