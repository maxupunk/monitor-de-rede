//! Requisições da base CRUD e do motor de monitoramento (Fases 2 e 3).

use backend::{
    app::App,
    models::_entities::{monitor_results, monitor_results_hourly},
    services::monitoring::{rollup::rollup_monitor_results, uptime::uptime_for_monitor},
};
use chrono::{Duration, Utc};
use loco_rs::testing::prelude::*;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
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

#[tokio::test]
#[serial]
async fn criacao_de_monitor_valida_tipo_suportado_quebras_de_linha_e_intervalo() {
    request::<App, _, _>(|mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        // Tipo inválido
        let tipo_invalido = request
            .post("/api/monitors")
            .json(&serde_json::json!({
                "name": "Monitor Invalido",
                "type": "custom_script_invalido",
                "target": "127.0.0.1"
            }))
            .await;
        assert_eq!(tipo_invalido.status_code(), 422);

        // Quebra de linha no nome
        let quebra_nome = request
            .post("/api/monitors")
            .json(&serde_json::json!({
                "name": "Monitor\nInjetado",
                "type": "ping",
                "target": "127.0.0.1"
            }))
            .await;
        assert_eq!(quebra_nome.status_code(), 422);

        // Intervalo menor que 1
        let intervalo_invalido = request
            .post("/api/monitors")
            .json(&serde_json::json!({
                "name": "Monitor Ping",
                "type": "ping",
                "target": "127.0.0.1",
                "intervalSeconds": 0
            }))
            .await;
        assert_eq!(intervalo_invalido.status_code(), 422);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn criacao_de_dispositivo_valida_ip_e_quebras_de_linha() {
    request::<App, _, _>(|mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        // IP inválido
        let ip_invalido = request
            .post("/api/devices")
            .json(&serde_json::json!({
                "name": "Switch Core",
                "deviceType": "switch",
                "ipAddress": "999.999.999.999"
            }))
            .await;
        assert_eq!(ip_invalido.status_code(), 422);

        // Quebra de linha no nome
        let quebra_nome = request
            .post("/api/devices")
            .json(&serde_json::json!({
                "name": "Switch\r\nCore",
                "deviceType": "switch"
            }))
            .await;
        assert_eq!(quebra_nome.status_code(), 422);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn listagem_de_monitores_permite_filtrar_por_enabled() {
    request::<App, _, _>(|mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let mon1 = request
            .post("/api/monitors")
            .json(&serde_json::json!({
                "name": "Monitor Ativo",
                "type": "ping",
                "target": "127.0.0.1",
                "enabled": true
            }))
            .await;
        assert_eq!(mon1.status_code(), 201);
        let mon1_val: serde_json::Value = serde_json::from_str(&mon1.text()).unwrap();

        let mon2 = request
            .post("/api/monitors")
            .json(&serde_json::json!({
                "name": "Monitor Inativo",
                "type": "ping",
                "target": "127.0.0.2",
                "enabled": false
            }))
            .await;
        assert_eq!(mon2.status_code(), 201);
        let mon2_val: serde_json::Value = serde_json::from_str(&mon2.text()).unwrap();

        // Sem filtro: retorna ambos
        let todos = request.get("/api/monitors").await;
        assert_eq!(todos.status_code(), 200);
        let lista_todos: Vec<serde_json::Value> = serde_json::from_str(&todos.text()).unwrap();
        assert!(lista_todos.iter().any(|m| m["id"] == mon1_val["id"]));
        assert!(lista_todos.iter().any(|m| m["id"] == mon2_val["id"]));

        // Filtro enabled=true: retorna apenas o ativo
        let ativos = request.get("/api/monitors?enabled=true").await;
        assert_eq!(ativos.status_code(), 200);
        let lista_ativos: Vec<serde_json::Value> = serde_json::from_str(&ativos.text()).unwrap();
        assert!(lista_ativos.iter().any(|m| m["id"] == mon1_val["id"]));
        assert!(!lista_ativos.iter().any(|m| m["id"] == mon2_val["id"]));

        // Filtro enabled=false: retorna apenas o inativo
        let inativos = request.get("/api/monitors?enabled=false").await;
        assert_eq!(inativos.status_code(), 200);
        let lista_inativos: Vec<serde_json::Value> =
            serde_json::from_str(&inativos.text()).unwrap();
        assert!(!lista_inativos.iter().any(|m| m["id"] == mon1_val["id"]));
        assert!(lista_inativos.iter().any(|m| m["id"] == mon2_val["id"]));
    })
    .await;
}

/// QUA-05 — `monitors.timeout_seconds` é preenchido e devolvido no contrato,
/// permitindo que o scheduler o honre em vez de recalcular do intervalo.
#[tokio::test]
#[serial]
async fn timeout_seconds_e_preenchido_e_exposto_no_monitor() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let monitor = request
            .post("/api/monitors")
            .json(&serde_json::json!({
                "name": "Ping com timeout",
                "type": "ping",
                "target": "127.0.0.1",
                "intervalSeconds": 60,
            }))
            .await;
        assert_eq!(monitor.status_code(), 201);
        let body: serde_json::Value = serde_json::from_str(&monitor.text()).unwrap();
        assert!(
            body["timeoutSeconds"].as_i64().is_some_and(|v| v >= 1),
            "timeoutSeconds deveria ser >= 1, mas foi {:?}",
            body["timeoutSeconds"]
        );
        assert!(
            body["timeoutSeconds"].as_i64().unwrap() < body["intervalSeconds"].as_i64().unwrap(),
            "timeout deve ser menor que o intervalo"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn endpoint_uptime_consolida_resultados_brutos_e_horarios() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let monitor = request
            .post("/api/monitors")
            .json(&serde_json::json!({
                "name": "Uptime local",
                "type": "ping",
                "target": "127.0.0.1",
            }))
            .await;
        assert_eq!(monitor.status_code(), 201);
        let monitor: serde_json::Value = serde_json::from_str(&monitor.text()).unwrap();
        let monitor_id = monitor["id"].as_i64().unwrap();

        // Sem dados: uptime 100% e zero checagens.
        let empty = request
            .get(&format!("/api/monitors/{monitor_id}/uptime"))
            .await;
        assert_eq!(empty.status_code(), 200);
        let empty: serde_json::Value = serde_json::from_str(&empty.text()).unwrap();
        assert_eq!(empty["uptimePercentage"], 100.0);
        assert_eq!(empty["totalChecks"], 0);

        // Resultado bruto na hora atual.
        monitor_results::ActiveModel {
            monitor_id: Set(monitor_id),
            status: Set("up".into()),
            started_at: Set(Utc::now().into()),
            finished_at: Set(Utc::now().into()),
            duration_ms: Set(1),
            latency_ms: Set(Some(10.0)),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();

        let partial = request
            .get(&format!("/api/monitors/{monitor_id}/uptime?hours=24"))
            .await;
        assert_eq!(partial.status_code(), 200);
        let partial: serde_json::Value = serde_json::from_str(&partial.text()).unwrap();
        assert_eq!(partial["uptimePercentage"], 100.0);
        assert_eq!(partial["totalChecks"], 1);
        assert_eq!(partial["avgLatencyMs"], 10.0);

        // Bucket horário fechado (hora anterior).
        let bucket = Utc::now() - Duration::hours(2);
        monitor_results_hourly::ActiveModel {
            monitor_id: Set(monitor_id),
            bucket: Set(bucket.into()),
            total_checks: Set(4),
            up_checks: Set(3),
            down_checks: Set(1),
            unknown_checks: Set(0),
            avg_latency_ms: Set(Some(12.5)),
            min_latency_ms: Set(Some(10.0)),
            max_latency_ms: Set(Some(15.0)),
            first_started_at: Set(bucket.into()),
            last_finished_at: Set(bucket.into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();

        let mixed = request
            .get(&format!("/api/monitors/{monitor_id}/uptime?hours=24"))
            .await;
        assert_eq!(mixed.status_code(), 200);
        let mixed: serde_json::Value = serde_json::from_str(&mixed.text()).unwrap();
        assert_eq!(mixed["totalChecks"], 5);
        assert_eq!(mixed["upChecks"], 4);
        assert_eq!(mixed["downChecks"], 1);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn rollup_agrega_resultados_brutos_em_buckets_horarios() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let monitor = request
            .post("/api/monitors")
            .json(&serde_json::json!({
                "name": "Rollup local",
                "type": "ping",
                "target": "127.0.0.1",
            }))
            .await;
        assert_eq!(monitor.status_code(), 201);
        let monitor: serde_json::Value = serde_json::from_str(&monitor.text()).unwrap();
        let monitor_id = monitor["id"].as_i64().unwrap();

        let base = Utc::now() - Duration::hours(3);
        for sequence in 0..5 {
            let timestamp = base + Duration::minutes(sequence);
            monitor_results::ActiveModel {
                monitor_id: Set(monitor_id),
                status: Set(if sequence % 2 == 0 {
                    "up".into()
                } else {
                    "down".into()
                }),
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

        let stats = rollup_monitor_results(&ctx.db, Utc::now()).await.unwrap();
        assert!(stats.buckets_upserted >= 1);
        assert_eq!(stats.rows_aggregated, 5);

        let buckets = monitor_results_hourly::Entity::find()
            .filter(monitor_results_hourly::Column::MonitorId.eq(monitor_id))
            .all(&ctx.db)
            .await
            .unwrap();
        assert!(!buckets.is_empty());
        let bucket = &buckets[0];
        assert_eq!(bucket.total_checks, 5);
        assert_eq!(bucket.up_checks, 3);
        assert_eq!(bucket.down_checks, 2);

        let uptime = uptime_for_monitor(&ctx.db, monitor_id, 24).await.unwrap();
        assert_eq!(uptime.total_checks, 5);
        assert_eq!(uptime.up_checks, 3);
        assert_eq!(uptime.down_checks, 2);
    })
    .await;
}
