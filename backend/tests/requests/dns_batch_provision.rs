//! Testes de provisionamento em lote de monitores DNS.

use backend::app::App;
use loco_rs::testing::prelude::*;
use serial_test::serial;

use super::prepare_data;

#[tokio::test]
#[serial]
async fn provisionamento_em_lote_de_monitores_dns() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        // 1. Validação de lista vazia
        let invalid = request
            .post("/api/dns/provision")
            .json(&serde_json::json!({
                "servers": []
            }))
            .await;
        assert_eq!(invalid.status_code(), 422);

        // 2. Provisionamento com múltiplos servidores
        let response = request
            .post("/api/dns/provision")
            .json(&serde_json::json!({
                "servers": [
                    { "server": "1.1.1.1", "name": "Cloudflare DNS", "protocol": "udp" },
                    { "server": "8.8.8.8", "name": "Google DNS", "protocol": "udp" },
                    { "server": "9.9.9.9", "name": "Quad9 DNS", "protocol": "udp" }
                ],
                "domain": "google.com",
                "domains": ["google.com", "cloudflare.com"],
                "intervalSeconds": 30,
                "executeNow": false
            }))
            .await;
        assert_eq!(response.status_code(), 200);

        let data: serde_json::Value = serde_json::from_str(&response.text()).unwrap();
        assert_eq!(data["createdCount"], 3);
        assert_eq!(data["alreadyMonitoredCount"], 0);
        assert_eq!(data["totalRequested"], 3);

        // 3. Tentar provisionar novamente os mesmos servidores (deve ignorar duplicados)
        let second_response = request
            .post("/api/dns/provision")
            .json(&serde_json::json!({
                "servers": [
                    { "server": "1.1.1.1", "name": "Cloudflare DNS", "protocol": "udp" },
                    { "server": "208.67.222.222", "name": "OpenDNS", "protocol": "udp" }
                ],
                "domain": "google.com",
                "executeNow": false
            }))
            .await;
        assert_eq!(second_response.status_code(), 200);

        let second_data: serde_json::Value = serde_json::from_str(&second_response.text()).unwrap();
        assert_eq!(second_data["createdCount"], 1);
        assert_eq!(second_data["alreadyMonitoredCount"], 1);

        // 4. Verificar que os monitores aparecem em GET /api/monitors?type=dns
        let monitors = request.get("/api/monitors?type=dns").await;
        assert_eq!(monitors.status_code(), 200);
        let monitors: Vec<serde_json::Value> = serde_json::from_str(&monitors.text()).unwrap();
        assert_eq!(monitors.len(), 4);

        // 5. Testar GET /api/dns/performance com series e ranking
        let perf_res = request.get("/api/dns/performance?hours=24").await;
        assert_eq!(perf_res.status_code(), 200);
        let perf_data: serde_json::Value = serde_json::from_str(&perf_res.text()).unwrap();
        assert_eq!(perf_data["monitorCount"], 4);
        assert!(perf_data["ranking"].is_array());
        assert!(perf_data["series"].is_array());
    })
    .await;
}
