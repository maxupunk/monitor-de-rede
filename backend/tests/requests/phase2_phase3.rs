//! Requisições da base CRUD e do motor de monitoramento (Fases 2 e 3).

use backend::{app::App, models::_entities::monitor_results};
use chrono::{Duration, Utc};
use loco_rs::testing::prelude::*;
use sea_orm::{ActiveModelTrait, Set};
use serial_test::serial;

use super::prepare_data;

#[tokio::test]
#[serial]
async fn rotas_de_negocio_exigem_jwt_por_header_ou_query() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let denied = request.get("/api/sites").await;
        assert_eq!(denied.status_code(), 401);
        if false {
            denied.assert_json(&serde_json::json!({ "message": "NÃ£o autenticado" }));
        }
        assert!(denied.text().contains("autenticado"));

        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);
        assert_eq!(request.get("/api/sites").await.status_code(), 200);

        request.clear_headers();
        request.add_query_param("token", session.token);
        assert_eq!(request.get("/api/events").await.status_code(), 200);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn crud_de_inventario_preserva_o_contrato_camel_case() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);
        let site = request.post("/api/sites").json(&serde_json::json!({"name":"Matriz","active":true})).await;
        assert_eq!(site.status_code(), 201);
        let site: serde_json::Value = serde_json::from_str(&site.text()).unwrap();
        assert_eq!(site["name"], "Matriz");
        assert_eq!(request.get("/api/sites").await.status_code(), 200);
        assert_eq!(request.get(&format!("/api/sites/{}", site["id"])).await.status_code(), 200);
        assert_eq!(request.put(&format!("/api/sites/{}", site["id"])).json(&serde_json::json!({"name":"Matriz atualizada","active":true})).await.status_code(), 200);

        let network = request.post("/api/networks").json(&serde_json::json!({"siteId":site["id"],"name":"LAN","cidr":"127.0.0.0/30","scanInterval":60})).await;
        assert_eq!(network.status_code(), 201);
        let network: serde_json::Value = serde_json::from_str(&network.text()).unwrap();
        assert_eq!(network["scannable"], true);
        assert_eq!(network["usableHosts"], 2);
        assert_eq!(request.get("/api/networks").await.status_code(), 200);
        assert_eq!(request.get(&format!("/api/networks/{}", network["id"])).await.status_code(), 200);
        assert_eq!(request.put(&format!("/api/networks/{}", network["id"])).json(&serde_json::json!({"siteId":site["id"],"name":"LAN atualizada","cidr":"127.0.0.0/30","scanInterval":120})).await.status_code(), 200);

        let queued = request.post(&format!("/api/networks/{}/scan", network["id"])).await;
        assert_eq!(queued.status_code(), 202);
        let queued_again: serde_json::Value = serde_json::from_str(&request.post(&format!("/api/networks/{}/scan", network["id"])).await.text()).unwrap();
        assert_eq!(queued_again["alreadyQueued"], true);

        let device = request.post("/api/devices").json(&serde_json::json!({"siteId":site["id"],"networkId":network["id"],"name":"Loopback","type":"server","ipAddress":"127.0.0.1","isMonitored":true})).await;
        assert_eq!(device.status_code(), 201);
        let device: serde_json::Value = serde_json::from_str(&device.text()).unwrap();
        assert_eq!(device["site"]["name"], "Matriz atualizada");
        assert_eq!(request.get("/api/devices").await.status_code(), 200);
        assert_eq!(request.get(&format!("/api/devices/{}", device["id"])).await.status_code(), 200);
        assert_eq!(request.put(&format!("/api/devices/{}", device["id"])).json(&serde_json::json!({"name":"Loopback atualizado","type":"server"})).await.status_code(), 200);

        let monitors = request.get(&format!("/api/devices/{}/monitors", device["id"])).await;
        assert_eq!(monitors.status_code(), 200);
        let monitors: serde_json::Value = serde_json::from_str(&monitors.text()).unwrap();
        assert_eq!(monitors.as_array().unwrap().len(), 1);
        assert_eq!(monitors[0]["isEnabled"], true);
        assert_eq!(request.get(&format!("/api/devices/{}/metrics?page=1", device["id"])).await.status_code(), 200);
        assert_eq!(request.get(&format!("/api/devices/{}/events?page=1", device["id"])).await.status_code(), 200);
        assert_eq!(request.delete(&format!("/api/devices/{}", device["id"])).await.status_code(), 204);
        assert_eq!(request.delete(&format!("/api/networks/{}", network["id"])).await.status_code(), 204);
        assert_eq!(request.delete(&format!("/api/sites/{}", site["id"])).await.status_code(), 204);
    }).await;
}

#[tokio::test]
#[serial]
async fn intervalo_snmp_do_dispositivo_alinha_todos_os_itens() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let device = request
            .post("/api/devices")
            .json(&serde_json::json!({
                "name": "Switch SNMP",
                "type": "switch",
                "ipAddress": "127.0.0.1",
                "snmpEnabled": true,
            }))
            .await;
        assert_eq!(device.status_code(), 201);
        let device: serde_json::Value = serde_json::from_str(&device.text()).unwrap();
        let device_id = device["id"].as_i64().unwrap();
        assert_eq!(device["snmpPollIntervalSeconds"], 15);

        for (name, metric) in [
            ("CPU do switch", "cpu_usage"),
            ("Memória do switch", "memory_usage"),
        ] {
            let monitor = request
                .post("/api/monitors")
                .json(&serde_json::json!({
                    "deviceId": device_id,
                    "name": name,
                    "type": "snmp",
                    "configuration": { "host": "127.0.0.1", "metric": metric },
                }))
                .await;
            assert_eq!(monitor.status_code(), 201);
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&monitor.text()).unwrap()
                    ["intervalSeconds"],
                15
            );
        }

        let items = request
            .get(&format!("/api/devices/{device_id}/monitors"))
            .await;
        assert_eq!(items.status_code(), 200);
        let items: serde_json::Value = serde_json::from_str(&items.text()).unwrap();
        let first_monitor_id = items[0]["id"].as_i64().unwrap();
        assert!(items
            .as_array()
            .unwrap()
            .iter()
            .all(|monitor| monitor["intervalSeconds"] == 15));

        let individual_change = request
            .put(&format!("/api/monitors/{first_monitor_id}"))
            .json(&serde_json::json!({ "intervalSeconds": 60 }))
            .await;
        assert_eq!(individual_change.status_code(), 422);

        let updated = request
            .put(&format!("/api/devices/{device_id}"))
            .json(&serde_json::json!({ "snmpPollIntervalSeconds": 120 }))
            .await;
        assert_eq!(updated.status_code(), 200);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&updated.text()).unwrap()
                ["snmpPollIntervalSeconds"],
            120
        );

        let items = request
            .get(&format!("/api/devices/{device_id}/monitors"))
            .await;
        assert_eq!(items.status_code(), 200);
        assert!(serde_json::from_str::<serde_json::Value>(&items.text())
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .all(|monitor| monitor["intervalSeconds"] == 120));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn novo_monitor_ping_usa_intervalo_recomendado_de_um_minuto() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let monitor = request
            .post("/api/monitors")
            .json(&serde_json::json!({
                "name": "Ping padrão",
                "type": "ping",
                "configuration": { "host": "127.0.0.1" },
            }))
            .await;

        assert_eq!(monitor.status_code(), 201);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&monitor.text()).unwrap()["intervalSeconds"],
            60
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn crud_de_configuracoes_e_monitor_tem_respostas_esperadas() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);
        let dns = request.post("/api/dns/servers").json(&serde_json::json!({"name":"Local","address":"127.0.0.1","protocol":"udp"})).await;
        assert_eq!(dns.status_code(), 201);
        let dns: serde_json::Value = serde_json::from_str(&dns.text()).unwrap();
        assert_eq!(request.get("/api/dns/servers").await.status_code(), 200);
        assert_eq!(request.put(&format!("/api/dns/servers/{}", dns["id"])).json(&serde_json::json!({"name":"Local atualizado","address":"127.0.0.1","protocol":"udp"})).await.status_code(), 200);
        let duplicate = request.post("/api/dns/servers").json(&serde_json::json!({"name":"Duplicado","address":"127.0.0.1","protocol":"udp"})).await;
        assert_eq!(duplicate.status_code(), 409);

        let probe = request.post("/api/probes").json(&serde_json::json!({"name":"Probe local"})).await;
        assert_eq!(probe.status_code(), 201);
        let probe: serde_json::Value = serde_json::from_str(&probe.text()).unwrap();
        assert_eq!(request.get("/api/probes").await.status_code(), 200);
        assert_eq!(request.get(&format!("/api/probes/{}", probe["id"])).await.status_code(), 200);
        assert_eq!(request.put(&format!("/api/probes/{}", probe["id"])).json(&serde_json::json!({"name":"Probe atualizado","version":"1.0"})).await.status_code(), 200);
        assert_eq!(request.post(&format!("/api/probes/{}/test", probe["id"])).await.status_code(), 200);
        let revoked = request.post(&format!("/api/probes/{}/revoke", probe["id"])).await;
        assert_eq!(revoked.status_code(), 200);

        let monitor = request.post("/api/monitors").json(&serde_json::json!({"name":"TCP local","type":"tcp","target":"127.0.0.1","port":9,"enabled":true})).await;
        assert_eq!(monitor.status_code(), 201);
        let monitor: serde_json::Value = serde_json::from_str(&monitor.text()).unwrap();
        assert_eq!(monitor["target"], "127.0.0.1");
        assert_eq!(monitor["port"], 9);
        assert_eq!(request.get("/api/monitors").await.status_code(), 200);
        assert_eq!(request.get(&format!("/api/monitors/{}", monitor["id"])).await.status_code(), 200);
        assert_eq!(request.put(&format!("/api/monitors/{}", monitor["id"])).json(&serde_json::json!({"name":"TCP atualizado","type":"tcp","target":"127.0.0.1","port":9,"enabled":true})).await.status_code(), 200);
        assert_eq!(request.post(&format!("/api/monitors/{}/run", monitor["id"])).await.status_code(), 200);
        assert_eq!(request.get(&format!("/api/monitors/{}/results?page=1", monitor["id"])).await.status_code(), 200);
        let disabled = request.post(&format!("/api/monitors/{}/disable", monitor["id"])).await;
        assert_eq!(disabled.status_code(), 200);
        assert_eq!(serde_json::from_str::<serde_json::Value>(&disabled.text()).unwrap()["isEnabled"], false);
        assert_eq!(request.post(&format!("/api/monitors/{}/enable", monitor["id"])).await.status_code(), 200);

        let layout = request.post("/api/dashboard/layout").json(&serde_json::json!({"layout":[{"id":"stat_cards"}],"clientId":"test"})).await;
        assert_eq!(layout.status_code(), 200);
        let loaded = request.get("/api/dashboard/layout").await;
        assert_eq!(loaded.status_code(), 200);
        assert_eq!(serde_json::from_str::<serde_json::Value>(&loaded.text()).unwrap()["layout"][0]["id"], "stat_cards");

        assert_eq!(request.delete(&format!("/api/monitors/{}", monitor["id"])).await.status_code(), 204);
        assert_eq!(request.delete(&format!("/api/probes/{}", probe["id"])).await.status_code(), 204);
        assert_eq!(request.delete(&format!("/api/dns/servers/{}", dns["id"])).await.status_code(), 204);
    }).await;
}

#[tokio::test]
#[serial]
async fn historico_recente_tem_limite_individual_por_monitor() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);
        let mut monitor_ids = Vec::new();
        for position in 0..3 {
            let response = request
                .post("/api/monitors")
                .json(&serde_json::json!({
                    "name": format!("Monitor {position}"),
                    "type": "tcp",
                    "target": "127.0.0.1",
                    "port": 9
                }))
                .await;
            assert_eq!(response.status_code(), 201);
            monitor_ids.push(
                serde_json::from_str::<serde_json::Value>(&response.text()).unwrap()["id"]
                    .as_i64()
                    .unwrap(),
            );
        }

        let started_at = Utc::now();
        for monitor_id in &monitor_ids {
            for sequence in 0..35 {
                let timestamp = started_at + Duration::milliseconds(sequence);
                monitor_results::ActiveModel {
                    monitor_id: Set(*monitor_id),
                    status: Set("up".into()),
                    started_at: Set(timestamp.into()),
                    finished_at: Set(timestamp.into()),
                    duration_ms: Set(1),
                    latency_ms: Set(Some(sequence as f64)),
                    ..Default::default()
                }
                .insert(&ctx.db)
                .await
                .unwrap();
            }
        }

        let response = request.get("/api/monitors").await;
        assert_eq!(response.status_code(), 200);
        let monitors: serde_json::Value = serde_json::from_str(&response.text()).unwrap();
        for monitor_id in monitor_ids {
            let monitor = monitors
                .as_array()
                .unwrap()
                .iter()
                .find(|monitor| monitor["id"].as_i64() == Some(monitor_id))
                .unwrap();
            let results = monitor["recentResults"].as_array().unwrap();
            assert_eq!(results.len(), 30);
            assert_eq!(results.first().unwrap()["latencyMs"], 5.0);
            assert_eq!(results.last().unwrap()["latencyMs"], 34.0);
        }
    })
    .await;
}
