use backend::{app::App, models::alert_events};
use chrono::Utc;
use loco_rs::testing::prelude::*;
use sea_orm::{ActiveModelTrait, Set};
use serial_test::serial;

use super::prepare_data;

async fn authenticated_request(request: &mut loco_rs::TestServer, ctx: &loco_rs::app::AppContext) {
    let session = prepare_data::init_operator(ctx).await;
    let (header, value) = prepare_data::auth_header(&session.token);
    request.add_header(header, value);
}

#[tokio::test]
#[serial]
async fn correlaciona_queda_de_pai_com_filhos_na_mesma_janela() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        authenticated_request(&mut request, &ctx).await;

        let pai = backend::models::devices::ActiveModel {
            name: Set("Roteador Principal".into()),
            r#type: Set("router".into()),
            ip_address: Set(Some("192.168.1.1".into())),
            status: Set("offline".into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("pai");

        let filho = backend::models::devices::ActiveModel {
            parent_id: Set(Some(pai.id)),
            name: Set("Servidor atrás do roteador".into()),
            r#type: Set("server".into()),
            ip_address: Set(Some("192.168.1.10".into())),
            status: Set("offline".into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("filho");

        let agora: chrono::DateTime<chrono::FixedOffset> = Utc::now().into();

        let alerta_pai = alert_events::ActiveModel {
            device_id: Set(Some(pai.id)),
            scope_key: Set(Some(format!("device:{}", pai.id))),
            status: Set("active".into()),
            severity: Set("critical".into()),
            started_at: Set(agora - chrono::Duration::seconds(30)),
            message: Set(Some("Pai inacessível".into())),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("alerta do pai");

        let alerta_filho = alert_events::ActiveModel {
            device_id: Set(Some(filho.id)),
            scope_key: Set(Some(format!("device:{}", filho.id))),
            status: Set("active".into()),
            severity: Set("critical".into()),
            started_at: Set(agora),
            message: Set(Some("Filho inacessível".into())),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("alerta do filho");

        let response = request
            .get(&format!("/api/alerts/{}/correlation", alerta_filho.id))
            .await;
        assert_eq!(response.status_code(), 200, "{}", response.text());

        let body: serde_json::Value = serde_json::from_str(&response.text()).unwrap();
        assert_eq!(body["correlationCount"], 1);
        assert_eq!(
            body["primaryCause"]["deviceId"],
            serde_json::json!(pai.id),
            "a causa raiz deve ser o pai"
        );
        assert_eq!(body["primaryCause"]["id"], alerta_pai.id);
        assert_eq!(body["causalCategory"], "router");
        assert_eq!(body["causalCategoryLabel"], "Roteador Principal");
        assert!(body["confidence"].as_i64().unwrap_or(0) >= 80);
        assert!(body["explanation"]
            .as_str()
            .unwrap()
            .contains("Roteador Principal"));
        assert_eq!(body["impactedDevicesCount"], 1);
        assert_eq!(body["dependencyChain"].as_array().unwrap().len(), 2);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn sem_eventos_relacionados_retorna_vazio() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        authenticated_request(&mut request, &ctx).await;

        let device = backend::models::devices::ActiveModel {
            name: Set("Dispositivo isolado".into()),
            r#type: Set("server".into()),
            status: Set("offline".into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("device");

        let evento = alert_events::ActiveModel {
            device_id: Set(Some(device.id)),
            scope_key: Set(Some(format!("device:{}", device.id))),
            status: Set("active".into()),
            severity: Set("critical".into()),
            started_at: Set(Utc::now().into()),
            message: Set(Some("Isolado".into())),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("alerta");

        let response = request
            .get(&format!("/api/alerts/{}/correlation", evento.id))
            .await;
        assert_eq!(response.status_code(), 200, "{}", response.text());

        let body: serde_json::Value = serde_json::from_str(&response.text()).unwrap();
        assert_eq!(
            body["correlationCount"].as_i64(),
            Some(0),
            "expected 0 correlationCount: {:?}",
            body
        );
        assert!(
            body["primaryCause"].is_null(),
            "expected primaryCause null: {:?}",
            body["primaryCause"]
        );
        assert_eq!(
            body["confidence"].as_i64(),
            Some(100),
            "expected 100 confidence: {:?}",
            body["confidence"]
        );
        assert_eq!(
            body["causalCategory"].as_str(),
            Some("isolated_device"),
            "expected isolated_device: {:?}",
            body["causalCategory"]
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn eventos_fechados_nao_entram_na_correlacao() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        authenticated_request(&mut request, &ctx).await;

        let agora: chrono::DateTime<chrono::FixedOffset> = Utc::now().into();

        let device = backend::models::devices::ActiveModel {
            name: Set("Dispositivo".into()),
            r#type: Set("server".into()),
            status: Set("offline".into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("device");

        let _evento_fechado = alert_events::ActiveModel {
            device_id: Set(Some(device.id)),
            scope_key: Set(Some(format!("device:{}", device.id))),
            status: Set("resolved".into()),
            severity: Set("critical".into()),
            started_at: Set(agora),
            resolved_at: Set(Some(agora)),
            message: Set(Some("Resolvido".into())),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("alerta fechado");

        let evento_alvo = alert_events::ActiveModel {
            device_id: Set(Some(device.id)),
            scope_key: Set(Some(format!("device:{}:alvo", device.id))),
            status: Set("active".into()),
            severity: Set("critical".into()),
            started_at: Set(agora),
            message: Set(Some("Alvo".into())),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("alerta alvo");

        let response = request
            .get(&format!("/api/alerts/{}/correlation", evento_alvo.id))
            .await;
        assert_eq!(response.status_code(), 200, "{}", response.text());

        let body: serde_json::Value = serde_json::from_str(&response.text()).unwrap();
        assert_eq!(body["correlationCount"], 0);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn diagnostico_global_de_causa_raiz_agrupa_incidentes_ativos() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        authenticated_request(&mut request, &ctx).await;

        let switch = backend::models::devices::ActiveModel {
            name: Set("Switch Distribuição".into()),
            r#type: Set("switch".into()),
            status: Set("offline".into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("switch");

        let host1 = backend::models::devices::ActiveModel {
            parent_id: Set(Some(switch.id)),
            name: Set("Câmera 01".into()),
            r#type: Set("camera".into()),
            status: Set("offline".into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("host1");

        let host2 = backend::models::devices::ActiveModel {
            parent_id: Set(Some(switch.id)),
            name: Set("Câmera 02".into()),
            r#type: Set("camera".into()),
            status: Set("offline".into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("host2");

        let agora: chrono::DateTime<chrono::FixedOffset> = Utc::now().into();

        let _alerta_sw = alert_events::ActiveModel {
            device_id: Set(Some(switch.id)),
            scope_key: Set(Some(format!("device:{}", switch.id))),
            status: Set("active".into()),
            severity: Set("critical".into()),
            started_at: Set(agora - chrono::Duration::seconds(20)),
            message: Set(Some("Switch offline".into())),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("alerta sw");

        let _alerta_h1 = alert_events::ActiveModel {
            device_id: Set(Some(host1.id)),
            scope_key: Set(Some(format!("device:{}", host1.id))),
            status: Set("active".into()),
            severity: Set("critical".into()),
            started_at: Set(agora),
            message: Set(Some("Camera 01 offline".into())),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("alerta h1");

        let _alerta_h2 = alert_events::ActiveModel {
            device_id: Set(Some(host2.id)),
            scope_key: Set(Some(format!("device:{}", host2.id))),
            status: Set("active".into()),
            severity: Set("critical".into()),
            started_at: Set(agora),
            message: Set(Some("Camera 02 offline".into())),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("alerta h2");

        let response = request.get("/api/alerts/root-cause-analysis").await;
        assert_eq!(response.status_code(), 200, "{}", response.text());

        let body: serde_json::Value = serde_json::from_str(&response.text()).unwrap();
        assert!(body["totalActiveIncidents"].as_u64().unwrap_or(0) >= 1);
        let clusters = body["activeClusters"].as_array().expect("clusters array");
        assert!(!clusters.is_empty());

        let cluster = &clusters[0];
        assert_eq!(cluster["causalCategory"], "switch");
        assert_eq!(cluster["causalCategoryLabel"], "Switch de Rede");
        assert_eq!(cluster["rootCauseDeviceId"], switch.id);
        assert_eq!(cluster["totalAlertsCount"], 3);
        assert_eq!(cluster["impactedDevicesCount"], 2);
    })
    .await;
}
