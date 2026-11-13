//! Sinais e clientes Wi-Fi pela API: a capacidade do equipamento, a leitura
//! em segundo plano publicada pelo SSE (`wifi:snapshot`), o ping até uma
//! estação e o vigia dos aparelhos que deveriam estar sempre conectados — com
//! um plugin de teste que fala com um "AP" em 127.0.0.1.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::{routing::get, Router};
use backend::app::App;
use chrono::Utc;
use loco_rs::{app::AppContext, testing::prelude::*};
use netmonitor_core::{
    events::Interests,
    models::{
        _entities::alert_events, alert_rules, devices, wifi_roaming_events, wifi_watched_clients,
    },
};
use netmonitor_network::plugins::wifi_source::Requester;
use netmonitor_network_wifi::{
    self as wifi, live::LiveStep, roaming::ROAMED_EVENT, watchlist::WATCHED_EVENT, SNAPSHOT_EVENT,
};
use netmonitor_nucleo::{services::events::EventBus, services::users::Role};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::{json, Value};
use serial_test::serial;

use super::prepare_data::session_with_role as session;

fn json_of(text: &str) -> Value {
    serde_json::from_str(text).expect("resposta JSON")
}

async fn device(ctx: &AppContext, name: &str) -> devices::Model {
    devices::ActiveModel {
        name: Set(name.into()),
        r#type: Set("access_point".into()),
        status: Set("online".into()),
        ip_address: Set(Some("127.0.0.1".into())),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap()
}

/// As estações que o "AP" entrega; o teste tira e devolve aparelhos.
type Stations = Arc<Mutex<Vec<Value>>>;

fn notebook() -> Value {
    json!({ "mac": "AA:BB:CC:DD:EE:01", "hostname": "notebook", "ip": "192.168.1.150",
            "ssid": "Loja", "band": "5g", "channel": 36, "signal_dbm": -58, "noise_dbm": -92,
            "tx_packets": 900, "tx_retries": 100, "inactive_ms": 40 })
}

/// Um "AP" que entrega as estações no contrato dos plugins Wi-Fi.
async fn fake_ap() -> (u16, Stations) {
    serve_ap(vec![
        notebook(),
        json!({ "mac": "3a:11:22:33:44:55", "band": "2g", "signal_dbm": -83, "inactive_ms": 20 }),
        json!({ "mac": "sem-mac", "signal_dbm": -40 }),
    ])
    .await
}

/// Um "AP" com estas estações.
async fn serve_ap(initial: Vec<Value>) -> (u16, Stations) {
    let stations: Stations = Arc::new(Mutex::new(initial));
    let served = stations.clone();
    let app = Router::new().route(
        "/clients",
        get(move || {
            let clients = served.lock().unwrap().clone();
            async move { json!({ "summary": "clientes", "clients": clients }).to_string() }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (port, stations)
}

/// Plugin mínimo com o bloco `wifi`: lista pela página do AP e "pinga".
fn package() -> Value {
    json!({
        "format": 1,
        "manifest": {
            "slug": "ap-teste", "name": "AP de teste", "version": "1.0.0",
            "transports": ["http"],
            "actions": [
                { "id": "detect", "title": "Detectar", "effect": "read", "output": "kv" },
                { "id": "clients", "title": "Clientes", "effect": "read", "output": "report" },
                { "id": "ping", "title": "Latência", "effect": "read", "output": "kv",
                  "params": { "type": "object", "properties": {
                      "target": { "type": "string", "pattern": "[0-9a-fA-F.:]{2,45}" } },
                      "required": ["target"] } }
            ],
            "wifi": { "clients": "clients", "ping": "ping" }
        },
        "script": "fn detect(device, params) { #{ firmware: \"1.0\" } }
fn clients(device, params) { json_parse(device.get(\"/clients\").body) }
fn ping(device, params) { #{ summary: params.target, sent: 3, received: 2, avg_ms: 4.5 } }",
        "usage": "## Detectar\nVersão.\n## Clientes\nEstações.\n## Latência\nPing.",
        "tests": {
            "unit": [
                { "action": "detect", "expect": { "firmware": "1.0" } },
                { "action": "clients",
                  "fixtures": [ { "http": "GET /clients", "body": "{\"clients\": []}" } ],
                  "expect": { "clients": [] } },
                { "action": "ping", "params": { "target": "10.0.0.2" },
                  "expect": { "sent": 3, "received": 2 } }
            ],
            "functional": [
                { "action": "detect", "expectKeys": ["firmware"] },
                { "action": "clients", "expectKeys": ["clients"] }
            ]
        }
    })
}

/// Um AP com o plugin de teste ativo, instalado e com acesso HTTP.
async fn ap_with_wifi(
    request: &loco_rs::TestServer,
    ctx: &AppContext,
) -> (devices::Model, Stations) {
    let id = wifi_plugin(request).await;
    let (port, stations) = fake_ap().await;
    let ap = attach_ap(request, ctx, id, "AP Sala", port).await;
    (ap, stations)
}

/// O plugin de teste, validado e promovido; devolve o id. Cada teste começa
/// sem memória de onde os aparelhos estavam (o roaming é do processo).
async fn wifi_plugin(request: &loco_rs::TestServer) -> i64 {
    wifi::roaming::forget();
    let created = json_of(
        &request
            .post("/api/plugins")
            .json(&json!({ "package": package() }))
            .await
            .text(),
    );
    let id = created["summary"]["id"].as_i64().unwrap();
    assert_eq!(created["summary"]["wifi"], true, "a frota leva ao painel");
    let test = json_of(
        &request
            .post(&format!("/api/plugins/{id}/test"))
            .await
            .text(),
    );
    assert_eq!(test["passed"], true, "{test}");
    request
        .post(&format!("/api/plugins/{id}/promote"))
        .await
        .assert_status_success();
    id
}

/// Um equipamento com o plugin instalado, falando com o "AP" na porta.
async fn attach_ap(
    request: &loco_rs::TestServer,
    ctx: &AppContext,
    plugin_id: i64,
    name: &str,
    port: u16,
) -> devices::Model {
    let ap = device(ctx, name).await;
    let id = plugin_id;
    request
        .put(&format!("/api/devices/{}/credentials", ap.id))
        .json(
            &json!({ "kind": "http", "username": "admin", "storage": "ask",
                       "secret": "s3nh4", "port": port }),
        )
        .await
        .assert_status_success();
    request
        .post(&format!("/api/devices/{}/plugins/{id}/install", ap.id))
        .await
        .assert_status_success();
    ap
}

/// O snapshot do equipamento no barramento, quando a leitura terminar.
async fn finished_snapshot(ctx: &AppContext, device_id: i64) -> Value {
    let bus = EventBus::from_context(ctx).unwrap();
    for _ in 0..200 {
        let found = bus
            .current_snapshots()
            .into_iter()
            .filter(|event| event.event_type == SNAPSHOT_EVENT)
            .map(|event| event.payload)
            .find(|payload| payload["deviceId"] == device_id && payload["status"] != "reading");
        if let Some(snapshot) = found {
            return snapshot;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("a leitura do equipamento {device_id} não terminou");
}

#[tokio::test]
#[serial]
async fn le_classifica_e_publica_as_estacoes_pelo_sse() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session(&mut request, &ctx, Role::Admin).await;
        let (ap, _) = ap_with_wifi(&request, &ctx).await;

        let caps = json_of(
            &request
                .get(&format!("/api/devices/{}/capabilities", ap.id))
                .await
                .text(),
        );
        assert_eq!(caps["wifi"], true, "plugin com bloco `wifi` dá a aba");

        let started = request
            .post("/api/wifi/refresh")
            .json(&json!({ "deviceId": ap.id }))
            .await;
        assert_eq!(started.status_code(), 202);
        assert_eq!(json_of(&started.text())["deviceIds"], json!([ap.id]));

        let snapshot = finished_snapshot(&ctx, ap.id).await;
        assert_eq!(snapshot["status"], "ok", "{snapshot}");
        assert_eq!(snapshot["deviceName"], "AP Sala");
        assert_eq!(snapshot["canPing"], true);
        assert!(snapshot["readAt"].is_string());
        let stations = snapshot["stations"].as_array().unwrap();
        assert_eq!(stations.len(), 2, "MAC inválido não vira estação");
        // Da pior para a melhor.
        let fraca = &stations[0];
        assert_eq!(fraca["mac"], "3a:11:22:33:44:55");
        assert_eq!(fraca["health"], "critical");
        assert_eq!(fraca["signalGrade"], "poor");
        assert_eq!(fraca["randomizedMac"], true, "MAC privado de celular");
        assert_eq!(fraca["snrDb"], Value::Null, "sem ruído não há SNR");
        let boa = &stations[1];
        assert_eq!(boa["mac"], "aa:bb:cc:dd:ee:01");
        assert_eq!(boa["band"], "5g");
        assert_eq!(boa["snrDb"], 34);
        assert_eq!(boa["txRetriesPct"], 10.0);
        assert_eq!(boa["health"], "healthy");

        let ping = request
            .post("/api/wifi/ping")
            .json(&json!({ "deviceId": ap.id, "target": "192.168.1.150" }))
            .await;
        assert_eq!(ping.status_code(), 200, "{}", ping.text());
        let ping = json_of(&ping.text());
        assert_eq!(ping["avgMs"], 4.5);
        assert_eq!(ping["lossPct"], 33.3);
        assert_eq!(ping["grade"], "critical", "perda é crítica");

        // Ler todos inclui o AP.
        let all = request.post("/api/wifi/refresh").await;
        assert_eq!(all.status_code(), 202);
        assert_eq!(json_of(&all.text())["deviceIds"], json!([ap.id]));
        finished_snapshot(&ctx, ap.id).await;
    })
    .await;
}

#[tokio::test]
#[serial]
async fn equipamento_sem_plugin_wifi_explica_o_que_falta() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session(&mut request, &ctx, Role::Operator).await;
        let ap = device(&ctx, "AP sem plugin").await;

        let caps = json_of(
            &request
                .get(&format!("/api/devices/{}/capabilities", ap.id))
                .await
                .text(),
        );
        assert_eq!(caps["wifi"], false, "tipo `access_point` não basta");

        let all = request.post("/api/wifi/refresh").await;
        assert_eq!(all.status_code(), 202);
        assert_eq!(json_of(&all.text())["deviceIds"], json!([]));

        let one = request
            .post("/api/wifi/refresh")
            .json(&json!({ "deviceId": ap.id }))
            .await;
        assert_eq!(one.status_code(), 400);
        assert!(one.text().contains("não tem um plugin de Wi-Fi ativo"));

        let missing = request
            .post("/api/wifi/refresh")
            .json(&json!({ "deviceId": 999_999 }))
            .await;
        assert_eq!(missing.status_code(), 404);

        let ping = request
            .post("/api/wifi/ping")
            .json(&json!({ "deviceId": ap.id, "target": "192.168.1.150" }))
            .await;
        assert_eq!(ping.status_code(), 400);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn ping_valida_o_ip_antes_de_tocar_o_equipamento() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session(&mut request, &ctx, Role::Operator).await;
        let ap = device(&ctx, "AP").await;
        for target in ["nao-eh-ip", "192.168.1.1; reboot", ""] {
            let response = request
                .post("/api/wifi/ping")
                .json(&json!({ "deviceId": ap.id, "target": target }))
                .await;
            assert_eq!(response.status_code(), 422, "{target}");
        }
    })
    .await;
}

#[tokio::test]
#[serial]
async fn visualizador_nao_dispara_leitura_nem_ping() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session(&mut request, &ctx, Role::Viewer).await;
        let ap = device(&ctx, "AP").await;
        let refresh = request.post("/api/wifi/refresh").await;
        assert_eq!(refresh.status_code(), 403);
        let ping = request
            .post("/api/wifi/ping")
            .json(&json!({ "deviceId": ap.id, "target": "192.168.1.150" }))
            .await;
        assert_eq!(ping.status_code(), 403);
    })
    .await;
}

/// A lista dos vigiados como a tela a recebe pelo SSE.
fn watched_snapshot(ctx: &AppContext) -> Value {
    EventBus::from_context(ctx)
        .unwrap()
        .current_snapshots()
        .into_iter()
        .find(|event| event.event_type == WATCHED_EVENT)
        .map(|event| event.payload)
        .expect("a lista dos vigiados está no barramento")
}

async fn events_of(ctx: &AppContext, watched_id: i64) -> Vec<alert_events::Model> {
    alert_events::Entity::find()
        .filter(alert_events::Column::ScopeKey.eq(format!("wifi_client:{watched_id}")))
        .all(&ctx.db)
        .await
        .unwrap()
}

#[tokio::test]
#[serial]
async fn aparelho_vigiado_que_some_vira_alerta_e_volta_resolvendo() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session(&mut request, &ctx, Role::Admin).await;
        let (ap, stations) = ap_with_wifi(&request, &ctx).await;

        // Cadastro: MAC normalizado, nasce aguardando a primeira leitura.
        let created = request
            .post("/api/wifi/watched")
            .json(&json!({ "mac": "AA-BB-CC-DD-EE-01", "name": "  Câmera da portaria " }))
            .await;
        assert_eq!(created.status_code(), 201, "{}", created.text());
        let created = json_of(&created.text());
        let id = created["id"].as_i64().unwrap();
        assert_eq!(created["mac"], "aa:bb:cc:dd:ee:01");
        assert_eq!(created["name"], "Câmera da portaria");
        assert_eq!(created["state"], "pending");
        assert_eq!(watched_snapshot(&ctx)["clients"][0]["id"], id);

        let repetido = request
            .post("/api/wifi/watched")
            .json(&json!({ "mac": "aa:bb:cc:dd:ee:01", "name": "Outra" }))
            .await;
        assert_eq!(repetido.status_code(), 409);
        let invalido = request
            .post("/api/wifi/watched")
            .json(&json!({ "mac": "nao-eh-mac", "name": "X" }))
            .await;
        assert_eq!(invalido.status_code(), 422);

        // O primeiro vigiado instala as regras de Wi-Fi.
        let regra = alert_rules::Entity::find()
            .filter(alert_rules::Column::TemplateKey.eq("wifi_watched_offline"))
            .one(&ctx.db)
            .await
            .unwrap();
        assert!(regra.is_some(), "regra de queda instalada");

        // Conectado: sem alerta.
        assert_eq!(wifi::watch::run(&ctx).await.unwrap(), 1);
        let cliente = watched_snapshot(&ctx)["clients"][0].clone();
        assert_eq!(cliente["state"], "connected", "{cliente}");
        assert_eq!(cliente["deviceId"], ap.id);
        assert_eq!(cliente["deviceName"], "AP Sala");
        assert!(events_of(&ctx, id).await.is_empty());

        // Some do AP (o celular de MAC privado também some — e não é vigiado).
        stations.lock().unwrap().clear();
        let mut visto: wifi_watched_clients::ActiveModel =
            wifi_watched_clients::Entity::find_by_id(id)
                .one(&ctx.db)
                .await
                .unwrap()
                .unwrap()
                .into();
        visto.last_seen_at = Set(Some((Utc::now() - chrono::Duration::minutes(15)).into()));
        visto.update(&ctx.db).await.unwrap();

        assert_eq!(wifi::watch::run(&ctx).await.unwrap(), 1);
        assert_eq!(watched_snapshot(&ctx)["clients"][0]["state"], "offline");
        let eventos = events_of(&ctx, id).await;
        assert_eq!(eventos.len(), 1, "um alerta, só do vigiado");
        assert_eq!(eventos[0].status, "active");
        assert_eq!(eventos[0].severity, "critical");
        let mensagem = eventos[0].message.clone().unwrap_or_default();
        assert!(mensagem.contains("fora do Wi-Fi"), "{mensagem}");
        let total = alert_events::Entity::find().all(&ctx.db).await.unwrap();
        assert_eq!(total.len(), 1, "aparelho não vigiado nunca alerta");

        // Volta: o alerta entra em recuperação.
        stations.lock().unwrap().push(notebook());
        wifi::watch::run(&ctx).await.unwrap();
        assert_eq!(watched_snapshot(&ctx)["clients"][0]["state"], "connected");
        let status = events_of(&ctx, id).await[0].status.clone();
        assert!(
            status == "recovering" || status == "resolved",
            "status após a volta: {status}"
        );

        // Renomear e deixar de vigiar fecham o ciclo.
        let renomeado = request
            .patch(&format!("/api/wifi/watched/{id}"))
            .json(&json!({ "name": "Câmera do portão" }))
            .await;
        assert_eq!(renomeado.status_code(), 200);
        assert_eq!(json_of(&renomeado.text())["name"], "Câmera do portão");
        let removido = request.delete(&format!("/api/wifi/watched/{id}")).await;
        assert_eq!(removido.status_code(), 204);
        assert_eq!(events_of(&ctx, id).await[0].status, "resolved");
        assert_eq!(watched_snapshot(&ctx)["clients"], json!([]));
        assert_eq!(
            wifi::watch::run(&ctx).await.unwrap(),
            0,
            "sem vigiado, sem leitura"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn visualizador_nao_vigia_aparelho() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session(&mut request, &ctx, Role::Viewer).await;
        let response = request
            .post("/api/wifi/watched")
            .json(&json!({ "mac": "aa:bb:cc:dd:ee:01", "name": "Câmera" }))
            .await;
        assert_eq!(response.status_code(), 403);
        let response = request.delete("/api/wifi/watched/1").await;
        assert_eq!(response.status_code(), 403);
    })
    .await;
}

/// O celular que anda pela casa: o mesmo MAC em um AP ou no outro.
fn celular(signal_dbm: i32) -> Value {
    json!({ "mac": "AA:BB:CC:DD:EE:77", "hostname": "iPhone-de-Joao", "ip": "192.168.1.77",
            "ssid": "Casa", "band": "5g", "channel": 36, "frequency_mhz": 5180,
            "tx_power_dbm": 23, "signal_dbm": signal_dbm, "noise_dbm": -92,
            "inactive_ms": 100 })
}

const CELULAR: &str = "aa:bb:cc:dd:ee:77";

/// Põe o celular só no AP `aqui` (e tira do outro).
fn move_to(aqui: &Stations, la: &Stations, signal_dbm: i32) {
    aqui.lock().unwrap().clear();
    aqui.lock().unwrap().push(celular(signal_dbm));
    la.lock().unwrap().clear();
}

#[tokio::test]
#[serial]
async fn roaming_entre_dois_aps_vira_historico_evento_ao_vivo_e_ping_pong() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session(&mut request, &ctx, Role::Admin).await;
        let plugin = wifi_plugin(&request).await;
        let (porta_sala, sala) = serve_ap(vec![celular(-60)]).await;
        let (porta_cozinha, cozinha) = serve_ap(Vec::new()).await;
        let ap_sala = attach_ap(&request, &ctx, plugin, "AP Sala", porta_sala).await;
        let ap_cozinha = attach_ap(&request, &ctx, plugin, "AP Cozinha", porta_cozinha).await;

        // Ping-pong com 3 trocas em 10 min.
        let mut ajustes = json_of(&request.get("/api/wifi/settings").await.text());
        assert_eq!(ajustes["pingPongCount"], 4, "o padrão");
        ajustes["pingPongCount"] = json!(3);
        let gravado = request.put("/api/wifi/settings").json(&ajustes).await;
        assert_eq!(gravado.status_code(), 200, "{}", gravado.text());

        let bus = EventBus::from_context(&ctx).unwrap();
        let mut ao_vivo = bus.subscribe();

        // Primeira rodada: aprende onde está, sem troca.
        let rodada = wifi::rounds::run(&ctx, Requester::watch()).await.unwrap();
        assert!(rodada.is_some());
        let estacao = finished_snapshot(&ctx, ap_sala.id).await["stations"][0].clone();
        assert!(estacao["linkScore"].as_f64().unwrap() > 0.9, "{estacao}");
        assert_eq!(
            estacao["distance"]["calibrated"], true,
            "potência e frequência do rádio"
        );
        assert_eq!(estacao["distance"]["environment"], "office");
        assert!(estacao["distance"]["minM"].as_f64() < estacao["distance"]["maxM"].as_f64());

        // Sala → Cozinha → Sala → Cozinha: três trocas.
        move_to(&cozinha, &sala, -55);
        wifi::rounds::run(&ctx, Requester::watch()).await.unwrap();
        move_to(&sala, &cozinha, -62);
        wifi::rounds::run(&ctx, Requester::watch()).await.unwrap();
        move_to(&cozinha, &sala, -57);
        wifi::rounds::run(&ctx, Requester::watch()).await.unwrap();

        // O histórico do painel, da mais recente para a mais antiga.
        let historico = request.get("/api/wifi/roaming?mac=AA-BB-CC-DD-EE-77").await;
        assert_eq!(historico.status_code(), 200, "{}", historico.text());
        let historico = json_of(&historico.text());
        let trocas = historico.as_array().unwrap();
        assert_eq!(trocas.len(), 3, "{historico}");
        assert_eq!(trocas[0]["fromDeviceName"], "AP Sala");
        assert_eq!(trocas[0]["toDeviceName"], "AP Cozinha");
        assert_eq!(trocas[0]["toDeviceId"], ap_cozinha.id);
        assert_eq!(trocas[0]["signalBeforeDbm"], -62);
        assert_eq!(trocas[0]["signalAfterDbm"], -57);
        assert_eq!(
            trocas[0]["pingPong"], true,
            "a terceira troca fecha o ping-pong"
        );
        assert_eq!(trocas[2]["pingPong"], false);
        assert_eq!(trocas[2]["fromDeviceName"], "AP Sala");

        // O mapa recebeu as trocas ao vivo.
        let mut vistas = 0;
        while let Ok(evento) = ao_vivo.try_recv() {
            if evento.event_type == ROAMED_EVENT {
                assert_eq!(evento.payload["mac"], CELULAR);
                vistas += 1;
            }
        }
        assert_eq!(vistas, 3);

        // A regra entrou com a primeira troca e o ping-pong virou alerta.
        let regra = alert_rules::Entity::find()
            .filter(alert_rules::Column::TemplateKey.eq("wifi_roaming_ping_pong"))
            .one(&ctx.db)
            .await
            .unwrap();
        assert!(regra.is_some(), "regra do ping-pong instalada");
        let alertas = alert_events::Entity::find()
            .filter(alert_events::Column::ScopeKey.eq(format!("wifi_roaming:{CELULAR}")))
            .all(&ctx.db)
            .await
            .unwrap();
        assert_eq!(alertas.len(), 1, "um alerta do aparelho");
        assert_eq!(alertas[0].status, "active");
        let mensagem = alertas[0].message.clone().unwrap_or_default();
        assert!(
            mensagem.contains("iPhone-de-Joao trocou de AP 3 vezes"),
            "{mensagem}"
        );

        // MAC inválido não consulta.
        let invalido = request.get("/api/wifi/roaming?mac=nao-eh-mac").await;
        assert_eq!(invalido.status_code(), 422);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn configuracoes_de_wifi_validam_os_limites_e_o_ambiente_por_ap() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session(&mut request, &ctx, Role::Operator).await;
        let padrao = json_of(&request.get("/api/wifi/settings").await.text());
        assert_eq!(padrao["liveIntervalS"], 15);
        assert_eq!(padrao["backgroundIntervalS"], 120);
        assert_eq!(padrao["roamingRetentionDays"], 30);
        assert_eq!(padrao["pingPongWindowMin"], 10);
        assert_eq!(padrao["environment"], "office");

        let mut rapido = padrao.clone();
        rapido["liveIntervalS"] = json!(1);
        let recusado = request.put("/api/wifi/settings").json(&rapido).await;
        assert_eq!(recusado.status_code(), 422);
        assert!(
            recusado.text().contains("atualização do mapa"),
            "{}",
            recusado.text()
        );

        let mut ajustes = padrao.clone();
        ajustes["environment"] = json!("walls");
        ajustes["apEnvironments"] = json!([
            { "deviceId": 7, "environment": "open" },
            { "deviceId": 8, "environment": "walls" }
        ]);
        let gravado = json_of(
            &request
                .put("/api/wifi/settings")
                .json(&ajustes)
                .await
                .text(),
        );
        assert_eq!(
            gravado["apEnvironments"],
            json!([{ "deviceId": 7, "environment": "open" }]),
            "igual ao padrão não fica guardado"
        );
        assert_eq!(
            json_of(&request.get("/api/wifi/settings").await.text())["environment"],
            "walls"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn visualizador_le_historico_e_configuracoes_mas_nao_grava() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session(&mut request, &ctx, Role::Viewer).await;
        assert_eq!(request.get("/api/wifi/settings").await.status_code(), 200);
        assert_eq!(
            request
                .get("/api/wifi/roaming?mac=aa:bb:cc:dd:ee:01")
                .await
                .status_code(),
            200
        );
        let padrao = json_of(&request.get("/api/wifi/settings").await.text());
        let gravar = request.put("/api/wifi/settings").json(&padrao).await;
        assert_eq!(gravar.status_code(), 403);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn coletor_do_mapa_so_le_com_alguem_olhando() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session(&mut request, &ctx, Role::Admin).await;
        let (ap, _) = ap_with_wifi(&request, &ctx).await;
        let lido = |ctx: &AppContext| {
            EventBus::from_context(ctx)
                .unwrap()
                .current_snapshots()
                .into_iter()
                .any(|event| event.event_type == SNAPSHOT_EVENT)
        };

        // Ninguém com o mapa aberto: nada é lido.
        assert_eq!(
            wifi::live::step(&ctx).await.unwrap(),
            LiveStep::NoSubscriber
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!lido(&ctx), "sem assinante, o AP não é tocado");

        // Uma aba abre o mapa (interesse `wifi` no stream SSE).
        let aba = Interests::from_context(&ctx).connect("wifi");
        let passo = wifi::live::step(&ctx).await.unwrap();
        assert!(
            matches!(passo, LiveStep::Read | LiveStep::Busy),
            "com assinante, lê (ou outra rodada já está lendo): {passo:?}"
        );
        assert_eq!(finished_snapshot(&ctx, ap.id).await["status"], "ok");

        // A aba fecha: o coletor para.
        drop(aba);
        assert_eq!(
            wifi::live::step(&ctx).await.unwrap(),
            LiveStep::NoSubscriber
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn historico_de_roaming_respeita_a_retencao_configurada() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let agora = Utc::now();
        for dias in [40, 2] {
            wifi_roaming_events::ActiveModel {
                mac: Set(CELULAR.into()),
                from_device_name: Set("AP Sala".into()),
                to_device_name: Set("AP Cozinha".into()),
                ping_pong: Set(false),
                created_at: Set((agora - chrono::Duration::days(dias)).into()),
                ..Default::default()
            }
            .insert(&ctx.db)
            .await
            .unwrap();
        }
        assert_eq!(
            wifi::roaming::prune(&ctx.db, agora).await.unwrap(),
            1,
            "30 dias"
        );
        let restantes = wifi::roaming::history(&ctx.db, CELULAR, None)
            .await
            .unwrap();
        assert_eq!(restantes.len(), 1);
        assert_eq!(restantes[0].from_device_id, None);
    })
    .await;
}
