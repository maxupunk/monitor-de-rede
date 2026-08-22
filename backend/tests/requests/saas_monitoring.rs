//! Testes de integração para os serviços de Monitoramento SaaS e Heatmap Horário (§2.2.2).

use backend::{
    app::App,
    dtos::saas::{HourlyHeatmapResponse, SaasPresetsResponse, SaasProvisionResponse},
    models::{_entities::monitor_results_hourly, monitors},
};
use chrono::{Duration, Utc};
use loco_rs::testing::prelude::*;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serial_test::serial;

use super::prepare_data;

#[tokio::test]
#[serial]
async fn catalogo_de_saas_retorna_presets_curados_com_thresholds_e_categorias() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let res = request.get("/api/monitors/saas/presets").await;
        assert_eq!(res.status_code(), 200);

        let catalog: SaasPresetsResponse = serde_json::from_str(&res.text()).unwrap();
        assert!(catalog.total_presets >= 8);
        assert_eq!(catalog.provisioned_count, 0);

        // Verifica provedores essenciais e bancos
        let providers: Vec<_> = catalog
            .presets
            .iter()
            .map(|p| p.provider.as_str())
            .collect();
        assert!(providers.contains(&"Nubank"));
        assert!(providers.contains(&"Itaú"));
        assert!(providers.contains(&"Google"));
        assert!(providers.contains(&"Cloudflare"));
        assert!(providers.contains(&"Microsoft"));
        assert!(providers.contains(&"GitHub"));
        assert!(providers.contains(&"Netflix"));
        assert!(providers.contains(&"Amazon"));

        // Valida campos e thresholds
        let nubank = catalog
            .presets
            .iter()
            .find(|p| p.id == "nubank-http")
            .expect("preset nubank-http deve existir");
        assert_eq!(nubank.check_type, "http");
        assert_eq!(nubank.category, "finance");
        assert_eq!(nubank.target, "https://www.nubank.com.br");
        assert!(nubank.suggested_thresholds.warning_latency_ms > 0.0);

        let cloudflare_http = catalog
            .presets
            .iter()
            .find(|p| p.id == "cloudflare-http")
            .expect("preset cloudflare-http deve existir");
        assert_eq!(cloudflare_http.check_type, "http");
        assert_eq!(cloudflare_http.http_method, Some("HEAD".into()));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn provisionamento_de_saas_cria_monitores_com_is_saas_e_reutiliza_existentes() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        // 1. Provisiona Nubank HTTP e Cloudflare HTTP
        let provision_res = request
            .post("/api/monitors/saas/provision")
            .json(&serde_json::json!({
                "presetIds": ["nubank-http", "cloudflare-http"]
            }))
            .await;
        assert_eq!(provision_res.status_code(), 200);

        let prov: SaasProvisionResponse = serde_json::from_str(&provision_res.text()).unwrap();
        assert_eq!(prov.provisioned_count, 2);
        assert_eq!(prov.created_monitor_ids.len(), 2);
        assert_eq!(prov.existing_monitor_ids.len(), 0);

        // 2. Consulta os monitores criados e valida configuração
        let m1_id = prov.created_monitor_ids[0];
        let m1 = monitors::Entity::find_by_id(m1_id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(m1.r#type, "http");
        assert_eq!(m1.configuration["isSaas"], true);
        assert_eq!(m1.configuration["saasPresetId"], "nubank-http");

        let m2_id = prov.created_monitor_ids[1];
        let m2 = monitors::Entity::find_by_id(m2_id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(m2.r#type, "http");
        assert_eq!(m2.configuration["method"], "HEAD");
        assert_eq!(m2.configuration["isSaas"], true);

        // 3. Consulta catálogo novamente e confirma `is_provisioned: true`
        let catalog_res = request.get("/api/monitors/saas/presets").await;
        let catalog: SaasPresetsResponse = serde_json::from_str(&catalog_res.text()).unwrap();
        assert_eq!(catalog.provisioned_count, 2);

        let nubank_p = catalog
            .presets
            .iter()
            .find(|p| p.id == "nubank-http")
            .unwrap();
        assert!(nubank_p.is_provisioned);
        assert_eq!(nubank_p.monitor_id, Some(m1_id));

        // 4. Provisiona novamente e garante idempotência (não duplica monitor)
        let reprovision_res = request
            .post("/api/monitors/saas/provision")
            .json(&serde_json::json!({
                "presetIds": ["nubank-http"]
            }))
            .await;
        assert_eq!(reprovision_res.status_code(), 200);
        let reprov: SaasProvisionResponse = serde_json::from_str(&reprovision_res.text()).unwrap();
        assert_eq!(reprov.created_monitor_ids.len(), 0);
        assert_eq!(reprov.existing_monitor_ids.len(), 1);
        assert_eq!(reprov.existing_monitor_ids[0], m1_id);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn heatmap_horario_calcula_matriz_e_detecta_picos() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        // 1. Cria um monitor
        let now = Utc::now();
        let mon = monitors::ActiveModel {
            name: Set("Cloudflare SaaS".into()),
            r#type: Set("ping".into()),
            configuration: Set(serde_json::json!({ "host": "1.1.1.1", "isSaas": true })),
            interval_seconds: Set(60),
            timeout_seconds: Set(5),
            retry_count: Set(2),
            enabled: Set(true),
            status: Set("up".into()),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();

        // 2. Insere dados horários:
        // - Hora 10: 20ms
        // - Hora 20 (Pico): 150ms
        let yesterday = (now - Duration::days(1)).date_naive();
        let t_h10 = yesterday.and_hms_opt(10, 0, 0).unwrap().and_utc();
        let t_h20 = yesterday.and_hms_opt(20, 0, 0).unwrap().and_utc();

        monitor_results_hourly::ActiveModel {
            monitor_id: Set(mon.id),
            probe_id: Set(None),
            bucket: Set(t_h10.into()),
            total_checks: Set(60),
            up_checks: Set(60),
            down_checks: Set(0),
            unknown_checks: Set(0),
            avg_latency_ms: Set(Some(20.0)),
            min_latency_ms: Set(Some(18.0)),
            max_latency_ms: Set(Some(25.0)),
            first_started_at: Set(t_h10.into()),
            last_finished_at: Set(t_h10.into()),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();

        monitor_results_hourly::ActiveModel {
            monitor_id: Set(mon.id),
            probe_id: Set(None),
            bucket: Set(t_h20.into()),
            total_checks: Set(60),
            up_checks: Set(55),
            down_checks: Set(5),
            unknown_checks: Set(0),
            avg_latency_ms: Set(Some(150.0)),
            min_latency_ms: Set(Some(100.0)),
            max_latency_ms: Set(Some(220.0)),
            first_started_at: Set(t_h20.into()),
            last_finished_at: Set(t_h20.into()),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();

        // 3. Consulta o endpoint de heatmap
        let heatmap_res = request
            .get(&format!(
                "/api/monitors/hourly-heatmap?monitorId={}&days=7",
                mon.id
            ))
            .await;
        assert_eq!(heatmap_res.status_code(), 200);

        let heatmap: HourlyHeatmapResponse = serde_json::from_str(&heatmap_res.text()).unwrap();
        assert!(!heatmap.matrix.is_empty());
        assert_eq!(heatmap.by_hour_of_day.len(), 24);
        assert_eq!(heatmap.monitors.len(), 1);
        assert_eq!(heatmap.monitors[0].name, "Cloudflare SaaS");

        // Valida que o peakHour detectou a hora com maior latência média (hora 20)
        assert_eq!(heatmap.peak_hour, Some(20));
        assert_eq!(heatmap.best_hour, Some(10));
        assert_eq!(heatmap.total_checks, 120);
    })
    .await;
}
