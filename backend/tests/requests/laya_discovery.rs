//! O Laya sugerindo tipo e sistema do que a descoberta encontrou.
//!
//! O que se afirma aqui, contra o Ollaya falso: um host em dúvida ganha o
//! palpite em `details.laya`, sem mudar o tipo que a heurística gravou; com o
//! Laya desligado não há consulta; fora do ar, o enriquecimento só não
//! acontece; e o "identificar" do cadastro devolve o palpite quando a
//! evidência dele é fraca.

use backend::{
    app::App,
    models::{discovery_results, discovery_runs, networks},
    services::{
        ai::{laya::config::AiLayaSettings, settings},
        discovery::{laya_identity, merger::DiscoveredHost},
    },
};
use chrono::Utc;
use loco_rs::{app::AppContext, testing::prelude::*};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde_json::{json, Value};
use serial_test::serial;

use super::{
    fake_ollaya::{self, by_keyword, with_key},
    prepare_data,
};

const IP: &str = "10.0.0.9";

/// Uma câmera que a heurística só soube chamar de `web_device`.
fn camera_host() -> DiscoveredHost {
    DiscoveredHost {
        ip_address: IP.into(),
        vendor: Some("Hikvision".into()),
        device_type: Some("web_device".into()),
        open_ports: vec![80, 554],
        confidence: 70,
        data: json!({ "server": "Linux/3.x UPnP/1.0 Hikvision-Webs" }),
        ..DiscoveredHost::default()
    }
}

/// Grava a execução como `persist_results` grava e devolve o id dela.
async fn persisted_run(ctx: &AppContext, host: &DiscoveredHost) -> (i64, i64) {
    let now = Utc::now();
    let network = networks::ActiveModel {
        name: Set("Rede de teste".into()),
        cidr: Set("10.0.0.0/24".into()),
        scan_enabled: Set(false),
        scan_interval: Set(3_600),
        active: Set(true),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap();
    let run = discovery_runs::ActiveModel {
        network_id: Set(network.id),
        status: Set("completed".into()),
        started_at: Set(now.into()),
        finished_at: Set(Some(now.into())),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap();
    let row = discovery_results::ActiveModel {
        discovery_run_id: Set(run.id),
        ip_address: Set(host.ip_address.clone()),
        vendor: Set(host.vendor.clone()),
        device_type: Set(host.device_type.clone()),
        confidence: Set(host.confidence),
        data: Set(Some(
            json!({ "openPorts": host.open_ports, "details": host.data }),
        )),
        first_seen_at: Set(now.into()),
        last_seen_at: Set(now.into()),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap();
    (run.id, row.id)
}

async fn row_data(ctx: &AppContext, id: i64) -> discovery_results::Model {
    discovery_results::Entity::find_by_id(id)
        .one(&ctx.db)
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
#[serial]
async fn host_em_duvida_ganha_o_palpite_sem_mudar_o_tipo_gravado() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        with_key();
        let fake = fake_ollaya::start(by_keyword(&[("hikvision", "camera")])).await;
        settings::save_laya(&ctx.db, fake.settings()).await.unwrap();
        let host = camera_host();
        let (run_id, row_id) = persisted_run(&ctx, &host).await;

        let enriched = laya_identity::enrich(&ctx, run_id, &[host]).await.unwrap();

        assert_eq!(enriched, 1);
        assert_eq!(fake.calls(), 1, "uma consulta por host em dúvida");
        let row = row_data(&ctx, row_id).await;
        assert_eq!(
            row.device_type.as_deref(),
            Some("web_device"),
            "a heurística fica"
        );
        let laya = &row.data.unwrap()["details"]["laya"];
        assert_eq!(laya["deviceType"]["value"], "camera");
        assert_eq!(laya["deviceType"]["model"], "laya:multilingual");
        assert!(laya["deviceType"]["confidence"].as_f64().unwrap() >= 60.0);
        let state = fake.states.lock().unwrap()[0].clone();
        assert!(
            state.contains("Hikvision-Webs"),
            "o SSDP vai no estado: {state}"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn desligado_nao_consulta_e_nao_mexe_no_resultado() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        with_key();
        let fake = fake_ollaya::start(by_keyword(&[("hikvision", "camera")])).await;
        let mut laya = fake.settings();
        laya.features.device_identity = false;
        settings::save_laya(&ctx.db, laya).await.unwrap();
        let host = camera_host();
        let (run_id, row_id) = persisted_run(&ctx, &host).await;

        assert_eq!(
            laya_identity::enrich(&ctx, run_id, &[host]).await.unwrap(),
            0
        );
        assert_eq!(fake.calls(), 0);
        assert!(row_data(&ctx, row_id).await.data.unwrap()["details"]
            .get("laya")
            .is_none());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn fora_do_ar_so_nao_enriquece() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let down = AiLayaSettings {
            enabled: true,
            base_url: "http://127.0.0.1:9".into(),
            timeout_ms: 300,
            ..AiLayaSettings::default()
        };
        settings::save_laya(&ctx.db, down).await.unwrap();
        let host = camera_host();
        let (run_id, _) = persisted_run(&ctx, &host).await;

        assert_eq!(
            laya_identity::enrich(&ctx, run_id, &[host]).await.unwrap(),
            0
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn identificar_devolve_o_palpite_quando_a_evidencia_e_fraca() {
    request_with_config::<App, _, _>(RequestConfig::default(), |request, ctx| async move {
        with_key();
        let fake = fake_ollaya::start(by_keyword(&[("hikvision", "camera")])).await;
        settings::save_laya(&ctx.db, fake.settings()).await.unwrap();
        // Sem SNMP nem SSH no loopback: a detecção fica no `padrão`.
        let host = DiscoveredHost {
            ip_address: "127.0.0.1".into(),
            ..camera_host()
        };
        persisted_run(&ctx, &host).await;
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (h, v) = prepare_data::auth_header(&session.token);

        let response = request
            .post("/api/devices/identify")
            .add_header(h, v)
            .json(&json!({ "ipAddress": "127.0.0.1", "snmpCommunity": "nao-existe" }))
            .await;
        assert_eq!(response.status_code(), 200, "{}", response.text());
        let body: Value = serde_json::from_str(&response.text()).unwrap();

        assert_eq!(body["source"], "padrão", "{body}");
        assert_eq!(body["laya"]["deviceType"]["value"], "camera", "{body}");
    })
    .await;
}
