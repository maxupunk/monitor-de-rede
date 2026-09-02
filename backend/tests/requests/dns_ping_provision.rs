//! Testes de provisionamento de ping para servidores DNS e correlação de latência.

use backend::app::App;
use loco_rs::testing::prelude::*;
use serial_test::serial;

use super::prepare_data;

#[tokio::test]
#[serial]
async fn provisionamento_de_ping_para_servidores_dns() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        // 1. Provisionamento explícito de Ping para DNS
        let response = request
            .post("/api/dns/provision-ping")
            .json(&serde_json::json!({
                "servers": [
                    { "server": "1.1.1.1", "name": "Cloudflare DNS" },
                    { "server": "8.8.8.8", "name": "Google DNS" }
                ],
                "intervalSeconds": 30,
                "executeNow": false
            }))
            .await;
        assert_eq!(response.status_code(), 200);

        let data: serde_json::Value = serde_json::from_str(&response.text()).unwrap();
        assert_eq!(data["createdCount"], 2);
        assert_eq!(data["alreadyMonitoredCount"], 0);

        // 2. Chamada subsequente com os mesmos alvos detecta que já estão monitorados
        let second_response = request
            .post("/api/dns/provision-ping")
            .json(&serde_json::json!({
                "servers": [
                    { "server": "1.1.1.1", "name": "Cloudflare DNS" }
                ],
                "executeNow": false
            }))
            .await;
        assert_eq!(second_response.status_code(), 200);
        let second_data: serde_json::Value = serde_json::from_str(&second_response.text()).unwrap();
        assert_eq!(second_data["createdCount"], 0);
        assert_eq!(second_data["alreadyMonitoredCount"], 1);

        // 3. Monitores aparecem em GET /api/monitors?type=ping
        let ping_monitors = request.get("/api/monitors?type=ping").await;
        assert_eq!(ping_monitors.status_code(), 200);
        let list: Vec<serde_json::Value> = serde_json::from_str(&ping_monitors.text()).unwrap();
        assert!(list
            .iter()
            .any(|m| m["name"] == "Ping Cloudflare DNS (1.1.1.1)" || m["target"] == "1.1.1.1"));

        // 4. Testar correlação em /api/devices/bandwidth-latency-series com pingTarget IP
        let series_res = request
            .get("/api/devices/bandwidth-latency-series?pingTarget=1.1.1.1&timeframe=15m")
            .await;
        assert_eq!(series_res.status_code(), 200);
        let series_data: serde_json::Value = serde_json::from_str(&series_res.text()).unwrap();
        assert!(series_data["samples"].is_array());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn provisionamento_dns_com_flag_include_ping() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        // Provisiona DNS e solicita inclusão de Ping
        let response = request
            .post("/api/dns/provision")
            .json(&serde_json::json!({
                "servers": [
                    { "server": "9.9.9.9", "name": "Quad9 DNS", "protocol": "udp" }
                ],
                "domain": "google.com",
                "intervalSeconds": 30,
                "includePing": true,
                "executeNow": false
            }))
            .await;
        assert_eq!(response.status_code(), 200);

        // Verifica que o monitor de ping correspondente foi criado
        let ping_monitors = request.get("/api/monitors?type=ping").await;
        assert_eq!(ping_monitors.status_code(), 200);
        let list: Vec<serde_json::Value> = serde_json::from_str(&ping_monitors.text()).unwrap();
        assert!(list.iter().any(|m| m["target"] == "9.9.9.9"));
    })
    .await;
}
