//! Testes do orquestrador com banco em memória e fakes dos contratos: o
//! plugin é um [`PluginInvoker`] falso, mas os motores nativos são os de
//! verdade — o iperf3 da central contra o servidor falso em loopback e o
//! btest do "MikroTik" discando no servidor armado da central.

use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicI64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use async_trait::async_trait;
use chrono::Utc;
use migration::{Migrator, MigratorTrait};
use sea_orm::{ActiveModelTrait, Database, DatabaseConnection, IntoActiveModel, Set};
use serde_json::{json, Value};
use serial_test::serial;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use super::{
    env::{BtestArmer, CentralAddress, DriverResolver, LinkTestEnv, PluginInvoker, ServerArmer},
    events, guard, janitor,
    model::{
        as_db, LinkTest, LinkTestFix, LinkTestKind, LinkTestOrigin, LinkTestRequest,
        LinkTestScheduleInput, LinkTestSide, LinkTestSpecInput, LinkTestStageKey, LinkTestStatus,
        LinkTestStepStatus,
    },
    plan, repository,
    repository::Requester,
    runner::PendingCleanup,
    schedules, service,
};
use crate::{
    models::{device_links, devices, link_test_legs, plugins},
    services::{
        events::EventBus,
        network_tools::throughput::{
            btest::{self, BtestServer},
            driver::{LocalDriver, ThroughputDriver},
            iperf3::fake_server::{Behavior, FakeServer},
            sample_channel, TestDirection, TestProtocol, ThroughputEngine, ThroughputSpec,
        },
        plugins::{
            credentials::{self, CredentialInput, SecretStorage},
            invoke::{RunOutcome, RunStatus, ServiceRunRequest},
            manifest::TransportKind,
            package::PluginPackage,
            runtime::{ProgressFlow, ProgressSample},
            service::{self as plugin_service, source},
        },
        shared::errors::{AppError, AppResult},
    },
};

// --- Fakes -------------------------------------------------------------------

/// Um plugin de teste que responde sem equipamento. `tp_run` com btest disca
/// de verdade no alvo (o servidor armado da central).
#[derive(Default)]
struct FakePlugins {
    calls: Mutex<Vec<(i64, String, Value)>>,
    next_id: AtomicI64,
    /// Porta que o `tp_serve` devolve (o servidor iperf3 falso).
    serve_port: Option<u16>,
    /// `tp_run` só volta quando o teste é cancelado.
    block_run: bool,
}

impl FakePlugins {
    fn calls(&self, action: &str) -> Vec<(i64, Value)> {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, called, _)| called == action)
            .map(|(device, _, params)| (*device, params.clone()))
            .collect()
    }

    async fn btest_run(params: &Value, cancel: &CancellationToken) -> AppResult<Value> {
        let target: SocketAddr = format!(
            "{}:{}",
            params["target"].as_str().unwrap(),
            params["port"].as_u64().unwrap()
        )
        .parse()
        .unwrap();
        let direction: TestDirection = serde_json::from_value(params["direction"].clone()).unwrap();
        let spec = ThroughputSpec {
            engine: ThroughputEngine::Btest,
            direction,
            protocol: TestProtocol::Tcp,
            duration_s: u8::try_from(params["durationS"].as_u64().unwrap()).unwrap(),
            streams: 1,
            limit_mbps: None,
        };
        let (sink, _rx) = sample_channel();
        let report = btest::run_client(target, &spec, None, &sink, cancel).await?;
        Ok(json!(report))
    }
}

#[async_trait]
impl PluginInvoker for FakePlugins {
    async fn execute(
        &self,
        request: ServiceRunRequest,
        started: Option<oneshot::Sender<i64>>,
    ) -> AppResult<RunOutcome> {
        let run_id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        if let Some(started) = started {
            let _ = started.send(run_id);
        }
        self.calls.lock().unwrap().push((
            request.device_id,
            request.action.clone(),
            request.params.clone(),
        ));
        let params = &request.params;
        let done = |status, output: Option<Value>| {
            Ok(RunOutcome {
                run_id,
                status,
                output,
                error: (status == RunStatus::Failed).then(|| "falhou".to_owned()),
            })
        };
        let cancel = request.cancel.clone().unwrap_or_default();
        match request.action.as_str() {
            "tp_prepare" => done(
                RunStatus::Succeeded,
                Some(json!({ "ready": true, "installed": ["iperf3"] })),
            ),
            "tp_serve" => done(
                RunStatus::Succeeded,
                Some(json!({
                    "port": self.serve_port.map_or_else(|| params["port"].clone(), |port| json!(port)),
                    "handle": "pid:4242"
                })),
            ),
            "tp_stop" => done(RunStatus::Succeeded, Some(json!({ "stopped": true }))),
            "tp_run" if self.block_run => {
                cancel.cancelled().await;
                done(RunStatus::Cancelled, None)
            }
            "tp_run" if params["engine"] == "btest" => match Self::btest_run(params, &cancel).await
            {
                Ok(report) => done(RunStatus::Succeeded, Some(report)),
                Err(_) => done(RunStatus::Failed, None),
            },
            "tp_run" => {
                if let Some(progress) = &request.progress {
                    for flow in [ProgressFlow::Tx, ProgressFlow::Rx] {
                        let _ = progress.send(ProgressSample {
                            t_ms: 1_000,
                            flow,
                            bps: 5e8,
                        });
                    }
                }
                // Quem dirige mede: envia 900 Mbit/s, recebe 400 Mbit/s.
                let report = match params["direction"].as_str() {
                    Some("send") => json!({ "engine": "iperf3", "txBps": 9e8, "retransmits": 3 }),
                    Some("receive") => json!({ "engine": "iperf3", "rxBps": 4e8 }),
                    _ => json!({ "engine": "iperf3", "txBps": 9e8, "rxBps": 4e8 }),
                };
                done(RunStatus::Succeeded, Some(report))
            }
            other => panic!("ação inesperada {other}"),
        }
    }
}

struct Drivers;

impl DriverResolver for Drivers {
    fn central(&self) -> Arc<dyn ThroughputDriver> {
        Arc::new(LocalDriver)
    }

    fn agent(&self, _agent_id: i64) -> AppResult<Arc<dyn ThroughputDriver>> {
        Err(AppError::business_rule("sem agente nos testes"))
    }
}

struct Loopback;

#[async_trait]
impl CentralAddress for Loopback {
    async fn toward(&self, _device: &devices::Model) -> AppResult<Option<String>> {
        Ok(Some("127.0.0.1".to_owned()))
    }
}

async fn db() -> DatabaseConnection {
    let mut options = sea_orm::ConnectOptions::new("sqlite::memory:".to_owned());
    options.max_connections(1).min_connections(1);
    let db = Database::connect(options).await.expect("banco");
    Migrator::up(&db, None).await.expect("migrations");
    db
}

fn env_with(
    db: DatabaseConnection,
    plugins: Arc<FakePlugins>,
    btest: Option<BtestServer>,
) -> Arc<LinkTestEnv> {
    Arc::new(LinkTestEnv {
        db,
        bus: Some(EventBus::create()),
        plugins,
        drivers: Arc::new(Drivers),
        btest: Arc::new(ServerArmer(btest)) as Arc<dyn BtestArmer>,
        central: Arc::new(Loopback),
    })
}

// --- Dados -------------------------------------------------------------------

async fn device(db: &DatabaseConnection, name: &str, ip: &str, system: bool) -> devices::Model {
    devices::ActiveModel {
        name: Set(name.into()),
        r#type: Set(if system { "server" } else { "router" }.into()),
        status: Set("online".into()),
        ip_address: Set(Some(ip.into())),
        system_key: Set(system.then(|| "netmonitor".to_owned())),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap()
}

fn text() -> Value {
    json!({ "type": "string" })
}

fn number() -> Value {
    json!({ "type": "integer" })
}

fn secret() -> Value {
    json!({ "type": "string", "secret": true })
}

/// Manifesto com o contrato fixo do §3: `serve`/`drive` dizem os motores.
fn manifest(slug: &str, serve: &[&str], drive: Value) -> Value {
    let mut actions = vec![
        json!({ "id": "detect", "title": "Detectar", "effect": "read", "output": "kv" }),
        json!({ "id": "tp_prepare", "title": "Preparar", "effect": "write",
        "params": { "type": "object", "properties": { "engine": text(), "role": text() } } }),
    ];
    let mut roles = json!({ "prepare": "tp_prepare" });
    if !serve.is_empty() {
        actions.push(
            json!({ "id": "tp_serve", "title": "Servir", "effect": "write",
            "params": { "type": "object", "properties": {
                "engine": text(), "port": number(), "peer": text(), "user": text(),
                "password": secret(), "requireAuth": { "type": "boolean" } } } }),
        );
        actions.push(
            json!({ "id": "tp_stop", "title": "Parar", "effect": "write",
            "params": { "type": "object", "properties": { "engine": text(), "handle": text() } } }),
        );
        roles["serve"] = json!("tp_serve");
        roles["stop"] = json!("tp_stop");
    }
    if drive.as_array().is_some_and(|list| !list.is_empty()) {
        actions.push(json!({ "id": "tp_run", "title": "Medir", "effect": "read",
            "params": { "type": "object", "properties": {
                "engine": text(), "target": text(), "port": number(), "direction": text(),
                "durationS": number(), "streams": number(), "limitMbps": number(),
                "user": text(), "password": secret() } } }));
        roles["run"] = json!("tp_run");
    }
    json!({
        "slug": slug, "name": slug, "version": "1.0.0", "transports": ["http"],
        "actions": actions,
        "throughput": {
            "serve": serve.iter().map(|engine| json!({ "engine": engine })).collect::<Vec<_>>(),
            "drive": drive,
            "actions": roles
        }
    })
}

async fn plugin(db: &DatabaseConnection, manifest: Value) -> plugins::Model {
    let package: PluginPackage = serde_json::from_value(json!({
        "manifest": manifest, "script": "", "usage": "## Teste de enlace"
    }))
    .unwrap();
    let created = plugin_service::create(db, &package, None, source::USER)
        .await
        .expect("manifesto válido");
    let mut active = created.into_active_model();
    active.status = Set(plugin_service::status::ACTIVE.to_owned());
    active.update(db).await.unwrap()
}

/// OpenWrt: iperf3 nos dois papéis (com `prepare`).
async fn openwrt_plugin(db: &DatabaseConnection) -> plugins::Model {
    plugin(
        db,
        manifest(
            "lt-openwrt",
            &["iperf3"],
            json!([{ "engine": "iperf3", "auth": false }]),
        ),
    )
    .await
}

/// RouterOS: btest nos dois papéis.
async fn mikrotik_plugin(db: &DatabaseConnection) -> plugins::Model {
    plugin(
        db,
        manifest(
            "lt-mikrotik",
            &["btest"],
            json!([{ "engine": "btest", "auth": true }]),
        ),
    )
    .await
}

async fn install(db: &DatabaseConnection, device: &devices::Model, plugin: &plugins::Model) {
    plugin_service::install(db, device, plugin.id, None)
        .await
        .unwrap();
}

async fn link(db: &DatabaseConnection, a: i64, b: i64) -> i64 {
    let now = Utc::now().into();
    device_links::ActiveModel {
        source_device_id: Set(a),
        target_device_id: Set(b),
        link_type: Set("ethernet".into()),
        discovery_method: Set("manual".into()),
        confidence: Set(100),
        confirmed: Set(true),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap()
    .id
}

fn request(kind: LinkTestKind) -> LinkTestRequest {
    LinkTestRequest {
        kind,
        spec: Some(LinkTestSpecInput {
            duration_s: Some(1),
            ..Default::default()
        }),
        confirm: Some(true),
        ..Default::default()
    }
}

fn user() -> Requester {
    Requester {
        origin: LinkTestOrigin::User,
        schedule_id: None,
        user_id: None,
    }
}

/// Espera o teste sair de `queued`/`running` (no máximo 15 s).
async fn finished(env: &LinkTestEnv, id: i64) -> LinkTest {
    for _ in 0..300 {
        let test = repository::detail(&env.db, id).await.unwrap();
        if !test.status.is_active() && !guard::is_running(id) {
            return test;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("o teste {id} não terminou em 15 s");
}

/// Dois OpenWrt ligados por um enlace.
async fn dois_openwrt(db: &DatabaseConnection) -> (devices::Model, devices::Model, i64) {
    let p = openwrt_plugin(db).await;
    let a = device(db, "OpenWrt A", "10.0.0.1", false).await;
    let b = device(db, "OpenWrt B", "10.0.0.2", false).await;
    install(db, &a, &p).await;
    install(db, &b, &p).await;
    let link_id = link(db, a.id, b.id).await;
    (a, b, link_id)
}

// --- Plano -------------------------------------------------------------------

#[tokio::test]
#[serial(link_tests)]
async fn plano_de_enlace_entre_dois_openwrt() {
    let env = env_with(db().await, Arc::default(), None);
    let (a, b, link_id) = dois_openwrt(&env.db).await;
    let built = plan::build(
        &env,
        &LinkTestRequest {
            link_id: Some(link_id),
            ..request(LinkTestKind::Link)
        },
    )
    .await
    .unwrap();
    let view = &built.view;
    assert!(view.blocking.is_empty(), "{:?}", view.blocking);
    assert_eq!(view.legs.len(), 1);
    let leg = &view.legs[0];
    assert_eq!(leg.engine, Some(ThroughputEngine::Iperf3));
    assert_eq!(leg.server_side, Some(LinkTestSide::B));
    assert_eq!((leg.a.device_id, leg.b.device_id), (Some(a.id), Some(b.id)));
    assert_eq!(leg.edge_id, Some(link_id));
    assert!(leg.runnable);
    assert_eq!(view.will_install.len(), 2, "o prepare pode instalar iperf3");
    assert!(view.estimated_seconds > 0);
    assert_eq!(built.device_ids().len(), 2);

    // Orientado pela origem escolhida.
    let invertido = plan::build(
        &env,
        &LinkTestRequest {
            link_id: Some(link_id),
            source_device_id: Some(b.id),
            ..request(LinkTestKind::Link)
        },
    )
    .await
    .unwrap();
    assert_eq!(invertido.view.legs[0].a.device_id, Some(b.id));
}

#[tokio::test]
#[serial(link_tests)]
async fn plano_sem_plugin_bloqueia_com_motivo() {
    let env = env_with(db().await, Arc::default(), None);
    let p = openwrt_plugin(&env.db).await;
    let a = device(&env.db, "OpenWrt", "10.0.0.1", false).await;
    let b = device(&env.db, "Switch", "10.0.0.2", false).await;
    install(&env.db, &a, &p).await;
    let link_id = link(&env.db, a.id, b.id).await;
    let built = plan::build(
        &env,
        &LinkTestRequest {
            link_id: Some(link_id),
            ..request(LinkTestKind::Link)
        },
    )
    .await
    .unwrap();
    assert!(!built.view.legs[0].runnable);
    assert!(
        built.view.blocking[0].contains("Switch não tem plugin de teste de enlace"),
        "{:?}",
        built.view.blocking
    );
    let erro = service::start(Arc::clone(&env), &built.request, user())
        .await
        .unwrap_err();
    assert!(matches!(erro, AppError::BusinessRule(_)), "{erro:?}");

    // Enlace inexistente é 404; pedido incompleto, 422.
    let erro = plan::build(
        &env,
        &LinkTestRequest {
            link_id: Some(9_999),
            ..request(LinkTestKind::Link)
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(erro, AppError::NotFound(_)));
    let erro = plan::build(&env, &request(LinkTestKind::Path))
        .await
        .unwrap_err();
    assert!(matches!(erro, AppError::Validation(_)));
}

#[tokio::test]
#[serial(link_tests)]
async fn plano_de_caminho_tem_uma_perna_por_enlace() {
    let env = env_with(db().await, Arc::default(), None);
    let p = openwrt_plugin(&env.db).await;
    let mut ids = Vec::new();
    for (index, name) in ["R1", "R2", "R3"].into_iter().enumerate() {
        let d = device(&env.db, name, &format!("10.0.1.{}", index + 1), false).await;
        install(&env.db, &d, &p).await;
        ids.push(d.id);
    }
    let l1 = link(&env.db, ids[0], ids[1]).await;
    let l2 = link(&env.db, ids[1], ids[2]).await;
    let built = plan::build(
        &env,
        &LinkTestRequest {
            source_device_id: Some(ids[0]),
            target_device_id: Some(ids[2]),
            ..request(LinkTestKind::Path)
        },
    )
    .await
    .unwrap();
    let edges: Vec<_> = built.view.legs.iter().map(|leg| leg.edge_id).collect();
    assert_eq!(edges, [Some(l1), Some(l2)]);
    assert_eq!(built.view.legs[1].seq, 2);
}

/// Um OpenWrt identificado (sistema escolhido no cadastro), sem plugin.
async fn openwrt_sem_plugin(db: &DatabaseConnection, name: &str) -> devices::Model {
    devices::ActiveModel {
        name: Set(name.into()),
        r#type: Set("router".into()),
        status: Set("online".into()),
        ip_address: Set(Some("10.0.9.1".into())),
        operating_system: Set(Some("openwrt".into())),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap()
}

fn ssh(secret: &str) -> CredentialInput {
    CredentialInput {
        kind: TransportKind::Ssh,
        username: "root".into(),
        secret: Some(secret.into()),
        storage: SecretStorage::Vault,
        port: None,
        https: None,
        via_probe_id: None,
    }
}

#[tokio::test]
#[serial(link_tests)]
async fn plano_oferece_instalar_o_plugin_e_cadastrar_o_acesso() {
    let env = env_with(db().await, Arc::default(), None);
    let ap = openwrt_sem_plugin(&env.db, "AP-terraço").await;
    let pedido = LinkTestRequest {
        target_device_id: Some(ap.id),
        ..request(LinkTestKind::Server)
    };
    let built = plan::build(&env, &pedido).await.unwrap();
    let view = &built.view;
    let leg = &view.legs[0];
    assert!(!leg.runnable);
    assert!(
        view.blocking.is_empty(),
        "só faltam passos: {:?}",
        view.blocking
    );
    assert_eq!(view.fixes.len(), 2, "{:?}", view.fixes);
    let LinkTestFix::InstallPlugin {
        plugin_id,
        plugin_slug,
        device_id,
        ..
    } = &view.fixes[0]
    else {
        panic!("primeiro, instalar: {:?}", view.fixes)
    };
    assert_eq!(plugin_slug, "openwrt-link-test");
    assert_eq!(*device_id, ap.id);
    assert!(matches!(
        &view.fixes[1],
        LinkTestFix::AddCredential { transport: TransportKind::Ssh, device_id, .. } if *device_id == ap.id
    ));
    assert_eq!(leg.fixes, view.fixes, "a perna diz os mesmos passos");
    assert_eq!(
        leg.engine,
        Some(ThroughputEngine::Iperf3),
        "já mostra como vai medir"
    );
    let erro = service::start(Arc::clone(&env), &built.request, user())
        .await
        .unwrap_err();
    assert!(
        matches!(&erro, AppError::BusinessRule(text) if text.starts_with("Antes de testar, falta")),
        "{erro:?}"
    );

    // Instalado (o embutido desligado liga junto), falta só o acesso.
    plugin_service::install(&env.db, &ap, *plugin_id, None)
        .await
        .unwrap();
    let built = plan::build(&env, &pedido).await.unwrap();
    assert!(matches!(
        built.view.fixes.as_slice(),
        [LinkTestFix::AddCredential { message, .. }] if message == "AP-terraço está sem acesso SSH"
    ));
    assert!(built.view.blocking.is_empty(), "{:?}", built.view.blocking);

    // Com o acesso, a perna roda.
    credentials::upsert(&env.db, ap.id, ssh("segredo"))
        .await
        .unwrap();
    let built = plan::build(&env, &pedido).await.unwrap();
    assert!(built.view.fixes.is_empty(), "{:?}", built.view.fixes);
    assert!(
        built.view.legs[0].runnable,
        "{:?}",
        built.view.legs[0].issues
    );
}

#[tokio::test]
#[serial(link_tests)]
async fn sistema_desconhecido_nao_ganha_sugestao() {
    let env = env_with(db().await, Arc::default(), None);
    let switch = device(&env.db, "Switch", "10.0.0.2", false).await;
    let built = plan::build(
        &env,
        &LinkTestRequest {
            target_device_id: Some(switch.id),
            ..request(LinkTestKind::Server)
        },
    )
    .await
    .unwrap();
    assert!(built.view.fixes.is_empty(), "{:?}", built.view.fixes);
    assert!(
        built.view.blocking[0].contains("Switch não tem plugin de teste de enlace"),
        "{:?}",
        built.view.blocking
    );
}

// --- Execução ----------------------------------------------------------------

#[tokio::test]
#[serial(link_tests)]
async fn executa_enlace_entre_equipamentos_e_traduz_os_sentidos() {
    let plugins = Arc::new(FakePlugins::default());
    let env = env_with(db().await, Arc::clone(&plugins), None);
    let (a, b, link_id) = dois_openwrt(&env.db).await;
    let mut events_rx = env.bus.as_ref().unwrap().subscribe();

    let started = service::start(
        Arc::clone(&env),
        &LinkTestRequest {
            link_id: Some(link_id),
            ..request(LinkTestKind::Link)
        },
        user(),
    )
    .await
    .unwrap();
    assert_eq!(started.status, LinkTestStatus::Queued);
    let test = finished(&env, started.id).await;

    assert_eq!(test.status, LinkTestStatus::Succeeded, "{test:#?}");
    let leg = &test.legs[0];
    assert_eq!(leg.status, LinkTestStepStatus::Succeeded);
    // A dirige (envia 900 → A→B; recebe 400 → B→A).
    assert_eq!(leg.forward_bps, Some(9e8));
    assert_eq!(leg.reverse_bps, Some(4e8));
    assert_eq!(leg.retransmits, Some(3));
    let summary = test.summary.unwrap();
    assert_eq!(summary.bottleneck_leg_seq, Some(1));
    assert_eq!(summary.min_reverse_bps, Some(4e8));
    for stage in &leg.stages {
        assert_eq!(stage.status, LinkTestStepStatus::Succeeded, "{stage:?}");
        assert!(stage.run_id.is_some(), "transcript a um clique: {stage:?}");
    }

    // Papéis e parâmetros do contrato fixo.
    let serve = plugins.calls("tp_serve");
    assert_eq!(serve.len(), 1);
    assert_eq!(serve[0].0, b.id, "B serve");
    assert!(
        guard::IPERF3_PORTS.contains(&u16::try_from(serve[0].1["port"].as_u64().unwrap()).unwrap())
    );
    assert_eq!(serve[0].1["peer"], "10.0.0.1");
    assert!(
        serve[0].1.get("password").is_none(),
        "iperf3 sem autenticação"
    );
    let runs = plugins.calls("tp_run");
    assert_eq!(runs.len(), 2, "iperf3: um sentido por vez");
    assert!(runs.iter().all(|(device, _)| *device == a.id), "A dirige");
    assert_eq!(runs[0].1["direction"], "send");
    assert_eq!(runs[1].1["direction"], "receive");
    assert_eq!(runs[0].1["target"], "10.0.0.2");
    assert_eq!(runs[0].1["durationS"], 1);
    assert!(runs[0].1.get("limitMbps").is_none(), "sem limite, omitido");
    let stop = plugins.calls("tp_stop");
    assert_eq!(stop[0].1["handle"], "pid:4242");

    // Limpeza feita: nada pendente; amostras guardadas no sentido do enlace.
    assert!(repository::pending_cleanups(&env.db)
        .await
        .unwrap()
        .is_empty());
    let samples = leg.samples.as_ref().unwrap();
    assert!(!samples.is_empty());

    // SSE: snapshot ao vivo, amostras e o estado final.
    let mut tipos = Vec::new();
    while let Ok(event) = events_rx.try_recv() {
        tipos.push((event.event_type.clone(), event.payload["status"].clone()));
    }
    assert!(tipos.iter().any(|(t, _)| t == events::SAMPLE_EVENT));
    assert!(tipos
        .iter()
        .any(|(t, s)| t == events::UPDATED_EVENT && s == "running"));
    assert_eq!(
        tipos
            .iter()
            .rev()
            .find(|(t, _)| t == events::UPDATED_EVENT)
            .unwrap()
            .1,
        "succeeded"
    );
    // O snapshot sai do barramento quando o teste termina.
    assert!(env
        .bus
        .as_ref()
        .unwrap()
        .current_snapshots()
        .iter()
        .all(|event| event.event_type != events::UPDATED_EVENT));
}

#[tokio::test]
#[serial(link_tests)]
async fn central_mede_openwrt_por_iperf3_em_loopback() {
    let server = FakeServer::start(Behavior::Normal).await;
    let plugins = Arc::new(FakePlugins {
        serve_port: Some(server.addr.port()),
        ..FakePlugins::default()
    });
    let env = env_with(db().await, Arc::clone(&plugins), None);
    device(&env.db, "Servidor NetMonitor", "127.0.0.2", true).await;
    let p = openwrt_plugin(&env.db).await;
    let alvo = device(&env.db, "OpenWrt", "127.0.0.1", false).await;
    install(&env.db, &alvo, &p).await;

    let started = service::start(
        Arc::clone(&env),
        &LinkTestRequest {
            target_device_id: Some(alvo.id),
            ..request(LinkTestKind::Server)
        },
        user(),
    )
    .await
    .unwrap();
    let test = finished(&env, started.id).await;
    assert_eq!(test.status, LinkTestStatus::Succeeded, "{test:#?}");
    let leg = &test.legs[0];
    assert_eq!(leg.plan.kind, super::model::LinkTestLegKind::EndToEnd);
    assert_eq!(leg.plan.a.kind, super::model::LinkTestEndpointKind::Central);
    assert_eq!(leg.plan.server_side, Some(LinkTestSide::B));
    assert!(leg.forward_bps.unwrap() > 0.0, "a central enviou");
    assert!(leg.reverse_bps.unwrap() > 0.0, "a central recebeu");
    assert_eq!(server.tests().await, 2, "um teste iperf3 por sentido");
    assert!(
        plugins.calls("tp_run").is_empty(),
        "quem dirige é a central"
    );
    assert_eq!(plugins.calls("tp_stop").len(), 1);
}

#[tokio::test]
#[serial(link_tests)]
async fn enlace_com_o_servidor_usa_a_central_e_nao_um_plugin() {
    let server = FakeServer::start(Behavior::Normal).await;
    let plugins = Arc::new(FakePlugins {
        serve_port: Some(server.addr.port()),
        ..FakePlugins::default()
    });
    let env = env_with(db().await, Arc::clone(&plugins), None);
    let sistema = device(&env.db, "Servidor NetMonitor", "127.0.0.2", true).await;
    let p = openwrt_plugin(&env.db).await;
    let alvo = device(&env.db, "OpenWrt", "127.0.0.1", false).await;
    install(&env.db, &alvo, &p).await;
    let enlace = link(&env.db, sistema.id, alvo.id).await;

    let started = service::start(
        Arc::clone(&env),
        &LinkTestRequest {
            link_id: Some(enlace),
            ..request(LinkTestKind::Link)
        },
        user(),
    )
    .await
    .unwrap();
    let test = finished(&env, started.id).await;
    assert_eq!(test.status, LinkTestStatus::Succeeded, "{test:#?}");
    let leg = &test.legs[0];
    assert_eq!(leg.plan.a.kind, super::model::LinkTestEndpointKind::Central);
    assert_eq!(leg.plan.a.device_id, Some(sistema.id));
    assert!(
        plugins.calls("tp_run").is_empty(),
        "quem dirige é a central"
    );
}

#[tokio::test]
#[serial(link_tests)]
async fn mikrotik_disca_no_btest_armado_da_central() {
    let server = BtestServer::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let plugins = Arc::new(FakePlugins::default());
    let env = env_with(db().await, Arc::clone(&plugins), Some(server.clone()));
    device(&env.db, "Servidor NetMonitor", "127.0.0.2", true).await;
    let p = mikrotik_plugin(&env.db).await;
    let mikrotik = device(&env.db, "MikroTik", "127.0.0.1", false).await;
    install(&env.db, &mikrotik, &p).await;

    let started = service::start(
        Arc::clone(&env),
        &LinkTestRequest {
            target_device_id: Some(mikrotik.id),
            ..request(LinkTestKind::Server)
        },
        user(),
    )
    .await
    .unwrap();
    let test = finished(&env, started.id).await;
    server.shutdown();
    assert_eq!(test.status, LinkTestStatus::Succeeded, "{test:#?}");
    let leg = &test.legs[0];
    assert_eq!(leg.plan.engine, Some(ThroughputEngine::Btest));
    assert_eq!(
        leg.plan.server_side,
        Some(LinkTestSide::A),
        "a central serve"
    );
    // O MikroTik (B) dirige: o que ele envia é B→A.
    assert!(leg.forward_bps.unwrap() > 0.0);
    assert!(leg.reverse_bps.unwrap() > 0.0);
    // Um sentido por vez (o "both" em TCP não devolve o envio num RouterOS
    // real), cada um numa sessão armada nova da central.
    let runs = plugins.calls("tp_run");
    assert_eq!(runs.len(), 2, "btest: um sentido por chamada");
    let directions: Vec<&str> = runs
        .iter()
        .map(|run| run.1["direction"].as_str().unwrap_or_default())
        .collect();
    assert!(directions.contains(&"send") && directions.contains(&"receive"));
    for run in &runs {
        assert_eq!(run.1["target"], "127.0.0.1");
        assert_eq!(run.1["port"], server.local_addr().port());
    }
    assert!(
        plugins.calls("tp_serve").is_empty(),
        "a central serve, não o plugin"
    );
    let stage = |key| leg.stages.iter().find(|stage| stage.key == key).unwrap();
    assert_eq!(
        stage(LinkTestStageKey::Stop).status,
        LinkTestStepStatus::Succeeded
    );
    // Cada sentido tem a sua execução (e o seu transcript).
    assert_ne!(
        stage(LinkTestStageKey::Forward).run_id,
        stage(LinkTestStageKey::Reverse).run_id
    );
}

#[tokio::test]
#[serial(link_tests)]
async fn cancelar_interrompe_e_sempre_encerra_o_servidor() {
    let plugins = Arc::new(FakePlugins {
        block_run: true,
        ..FakePlugins::default()
    });
    let env = env_with(db().await, Arc::clone(&plugins), None);
    let (a, _, link_id) = dois_openwrt(&env.db).await;
    let pedido = LinkTestRequest {
        link_id: Some(link_id),
        ..request(LinkTestKind::Link)
    };
    let started = service::start(Arc::clone(&env), &pedido, user())
        .await
        .unwrap();
    // Espera a medida começar.
    for _ in 0..100 {
        if !plugins.calls("tp_run").is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    // Um teste por dispositivo: o segundo pedido recebe 409.
    let erro = service::start(Arc::clone(&env), &pedido, user())
        .await
        .unwrap_err();
    assert!(
        matches!(&erro, AppError::Conflict(m) if m.contains("já está em outro teste")),
        "{erro:?}"
    );
    let plano = plan::build(&env, &pedido).await.unwrap();
    assert!(
        plano.view.blocking[0].contains(&a.name),
        "{:?}",
        plano.view.blocking
    );

    service::cancel(&env, started.id).await.unwrap();
    let test = finished(&env, started.id).await;
    assert_eq!(test.status, LinkTestStatus::Cancelled);
    assert_eq!(test.legs[0].status, LinkTestStepStatus::Cancelled);
    let stop = test.legs[0]
        .stages
        .iter()
        .find(|stage| stage.key == LinkTestStageKey::Stop)
        .unwrap();
    assert_eq!(
        stop.status,
        LinkTestStepStatus::Succeeded,
        "stop roda sempre"
    );
    assert_eq!(plugins.calls("tp_stop").len(), 1);
    assert!(repository::pending_cleanups(&env.db)
        .await
        .unwrap()
        .is_empty());

    // Terminado não cancela de novo.
    assert!(matches!(
        service::cancel(&env, started.id).await,
        Err(AppError::Conflict(_))
    ));
}

#[tokio::test]
#[serial(link_tests)]
async fn janitor_encerra_pendencias_e_marca_interrompidos() {
    let plugins = Arc::new(FakePlugins::default());
    let env = env_with(db().await, Arc::clone(&plugins), None);
    let (a, b, link_id) = dois_openwrt(&env.db).await;
    let built = plan::build(
        &env,
        &LinkTestRequest {
            link_id: Some(link_id),
            ..request(LinkTestKind::Link)
        },
    )
    .await
    .unwrap();
    let (mut test, legs) = repository::insert(&env.db, &built, user()).await.unwrap();
    test.status = as_db(&LinkTestStatus::Running);
    repository::save_test(&env.db, &test).await.unwrap();
    let mut leg: link_test_legs::Model = legs[0].clone();
    leg.status = as_db(&LinkTestStepStatus::Running);
    leg.cleanup = Some(json!(PendingCleanup {
        device_id: b.id,
        plugin_slug: "lt-openwrt".into(),
        action: "tp_stop".into(),
        engine: ThroughputEngine::Iperf3,
        handle: "pid:777".into(),
    }));
    repository::save_leg(&env.db, &leg).await.unwrap();

    let stats = janitor::recover(&env).await.unwrap();
    assert_eq!(stats.stopped, 1);
    assert_eq!(stats.interrupted, 1);
    let stop = plugins.calls("tp_stop");
    assert_eq!(
        stop,
        [(b.id, json!({ "engine": "iperf3", "handle": "pid:777" }))]
    );
    let detail = repository::detail(&env.db, test.id).await.unwrap();
    assert_eq!(detail.status, LinkTestStatus::Failed);
    assert_eq!(detail.error.as_deref(), Some(janitor::INTERRUPTED));
    assert_eq!(detail.legs[0].status, LinkTestStepStatus::Failed);
    assert!(repository::pending_cleanups(&env.db)
        .await
        .unwrap()
        .is_empty());
    assert_ne!(a.id, b.id);

    // Idempotente.
    assert_eq!(
        janitor::recover(&env).await.unwrap(),
        janitor::RecoverStats::default()
    );
}

#[tokio::test]
#[serial(link_tests)]
async fn agendamento_vencido_dispara_e_guarda_o_desfecho() {
    let plugins = Arc::new(FakePlugins::default());
    let env = env_with(db().await, Arc::clone(&plugins), None);
    let (_, _, link_id) = dois_openwrt(&env.db).await;
    let criado = schedules::create(
        &env,
        &LinkTestScheduleInput {
            name: "Enlace da torre".into(),
            kind: LinkTestKind::Link,
            link_id: Some(link_id),
            source_device_id: None,
            target_device_id: None,
            interval_minutes: 60,
            window_start: None,
            window_end: None,
            enabled: true,
            spec: Some(LinkTestSpecInput {
                duration_s: Some(1),
                ..Default::default()
            }),
        },
        None,
    )
    .await
    .unwrap();
    assert!(criado.next_run_at.is_some());
    // Ainda não venceu.
    assert_eq!(
        schedules::run_due(Arc::clone(&env), Utc::now())
            .await
            .unwrap(),
        0
    );
    let depois = Utc::now() + chrono::Duration::minutes(61);
    assert_eq!(
        schedules::run_due(Arc::clone(&env), depois).await.unwrap(),
        1
    );
    // O executor grava o último teste assim que começa (em segundo plano).
    let mut test_id = None;
    for _ in 0..100 {
        test_id = schedules::find(&env, criado.id).await.unwrap().last_test_id;
        if test_id.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let test_id = test_id.expect("o executor grava o último teste");
    let test = finished(&env, test_id).await;
    assert_eq!(test.origin, LinkTestOrigin::Schedule);
    assert_eq!(test.schedule_id, Some(criado.id));
    let row = schedules::find(&env, criado.id).await.unwrap();
    assert_eq!(row.last_status.as_deref(), Some("succeeded"));
    assert!(row.next_run_at.unwrap() > depois);

    // Intervalo abaixo de 15 min e janela pela metade são recusados.
    let mut ruim = LinkTestScheduleInput {
        name: "x".into(),
        kind: LinkTestKind::Link,
        link_id: Some(link_id),
        source_device_id: None,
        target_device_id: None,
        interval_minutes: 5,
        window_start: None,
        window_end: None,
        enabled: true,
        spec: None,
    };
    assert!(matches!(
        schedules::create(&env, &ruim, None).await,
        Err(AppError::Validation(_))
    ));
    ruim.interval_minutes = 30;
    ruim.window_start = Some("22:00".into());
    assert!(matches!(
        schedules::create(&env, &ruim, None).await,
        Err(AppError::Validation(_))
    ));
}

#[tokio::test]
#[serial(link_tests)]
async fn lista_filtra_por_dispositivo_enlace_e_estado() {
    let plugins = Arc::new(FakePlugins::default());
    let env = env_with(db().await, Arc::clone(&plugins), None);
    let (a, _, link_id) = dois_openwrt(&env.db).await;
    let outro = device(&env.db, "Outro", "10.0.9.9", false).await;
    let started = service::start(
        Arc::clone(&env),
        &LinkTestRequest {
            link_id: Some(link_id),
            ..request(LinkTestKind::Link)
        },
        user(),
    )
    .await
    .unwrap();
    finished(&env, started.id).await;
    let page = |query| {
        let env = Arc::clone(&env);
        async move { repository::page(&env.db, &query).await.unwrap() }
    };
    let todos = page(super::model::LinkTestQuery::default()).await;
    assert_eq!((todos.total, todos.page, todos.per_page), (1, 1, 20));
    assert!(
        todos.items[0].legs[0].samples.is_none(),
        "amostras só no detalhe"
    );
    let por_dispositivo = page(super::model::LinkTestQuery {
        device_id: Some(a.id),
        ..Default::default()
    })
    .await;
    assert_eq!(por_dispositivo.total, 1);
    let nenhum = page(super::model::LinkTestQuery {
        device_id: Some(outro.id),
        ..Default::default()
    })
    .await;
    assert_eq!(nenhum.total, 0);
    let por_enlace = page(super::model::LinkTestQuery {
        link_id: Some(link_id),
        status: Some(LinkTestStatus::Succeeded),
        ..Default::default()
    })
    .await;
    assert_eq!(por_enlace.total, 1);
    let falhos = page(super::model::LinkTestQuery {
        status: Some(LinkTestStatus::Failed),
        ..Default::default()
    })
    .await;
    assert_eq!(falhos.total, 0);
}
