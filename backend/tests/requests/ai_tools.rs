//! Ferramentas da IA contra o banco: o que a IA lê para diagnosticar e o que
//! a tela recebe para desenhar.
//!
//! O que se afirma aqui: as consultas de histórico e de interfaces valem no
//! dialeto do banco de teste, os gráficos chegam com pontos para a tela e só
//! o resumo para a IA, e ferramenta ativa desabilitada não roda.

use backend::{
    app::App,
    models::_entities::{
        alert_events, device_interfaces, devices, metrics, monitor_results, monitors,
    },
    services::ai::harness::tools::ToolRegistry,
};
use chrono::{Duration, Utc};
use loco_rs::{prelude::AppContext, testing::prelude::*};
use sea_orm::{ActiveModelTrait, Set};
use serde_json::json;
use serial_test::serial;

async fn aparelho(ctx: &AppContext) -> devices::Model {
    devices::ActiveModel {
        name: Set("Borda Principal".into()),
        r#type: Set("router".into()),
        ip_address: Set(Some("10.0.0.1".into())),
        is_monitored: Set(true),
        snmp_enabled: Set(true),
        status: Set("warning".into()),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap()
}

async fn porta(
    ctx: &AppContext,
    device_id: i64,
    index: i32,
    name: &str,
    oper: &str,
) -> device_interfaces::Model {
    device_interfaces::ActiveModel {
        device_id: Set(device_id),
        snmp_index: Set(Some(index)),
        name: Set(name.into()),
        admin_status: Set(Some("up".into())),
        oper_status: Set(Some(oper.into())),
        speed: Set(Some(100_000_000)),
        created_at: Set(Utc::now().into()),
        updated_at: Set(Utc::now().into()),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap()
}

async fn trafego(ctx: &AppContext, device_id: i64, interface_id: i64, nome: &str, valores: &[f64]) {
    let total = valores.len() as i64;
    for (i, valor) in valores.iter().enumerate() {
        let quando = Utc::now() - Duration::minutes(total - i as i64);
        metrics::ActiveModel {
            device_id: Set(device_id),
            interface_id: Set(Some(interface_id)),
            name: Set(nome.into()),
            value: Set(*valor),
            unit: Set("bps".into()),
            recorded_at: Set(quando.into()),
            created_at: Set(quando.into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();
    }
}

async fn ping(ctx: &AppContext, device_id: i64) -> monitors::Model {
    monitors::ActiveModel {
        device_id: Set(Some(device_id)),
        r#type: Set("ping".into()),
        name: Set("Ping Borda".into()),
        configuration: Set(json!({ "host": "10.0.0.1" })),
        interval_seconds: Set(60),
        timeout_seconds: Set(5),
        retry_count: Set(3),
        enabled: Set(true),
        status: Set("up".into()),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap()
}

async fn checagem(
    ctx: &AppContext,
    monitor_id: i64,
    minutos_atras: i64,
    status: &str,
    latencia: f64,
) {
    let quando = Utc::now() - Duration::minutes(minutos_atras);
    monitor_results::ActiveModel {
        monitor_id: Set(monitor_id),
        status: Set(status.into()),
        started_at: Set(quando.into()),
        finished_at: Set(quando.into()),
        duration_ms: Set(10),
        latency_ms: Set((status == "up").then_some(latencia)),
        message: Set((status != "up").then(|| "Timeout".to_string())),
        created_at: Set(quando.into()),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap();
}

async fn alerta(ctx: &AppContext, device_id: i64, status: &str, mensagem: &str) {
    let inicio = Utc::now() - Duration::hours(2);
    alert_events::ActiveModel {
        device_id: Set(Some(device_id)),
        status: Set(status.into()),
        severity: Set("critical".into()),
        started_at: Set(inicio.into()),
        resolved_at: Set((status == "resolved").then(|| (inicio + Duration::minutes(30)).into())),
        message: Set(Some(mensagem.into())),
        created_at: Set(inicio.into()),
        updated_at: Set(inicio.into()),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap();
}

#[tokio::test]
#[serial]
async fn interfaces_com_problema_mostram_porta_caida_e_saturada() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let dev = aparelho(&ctx).await;
        let wan = porta(&ctx, dev.id, 1, "ether1", "up").await;
        porta(&ctx, dev.id, 2, "ether2", "down").await;
        porta(&ctx, dev.id, 3, "ether3", "up").await;
        trafego(&ctx, dev.id, wan.id, "inBps", &[10_000_000.0, 95_000_000.0]).await;

        let saida = ToolRegistry::new(false)
            .execute(
                &ctx,
                "get_device_interfaces",
                r#"{"device": "borda", "problems_only": true}"#,
            )
            .await
            .unwrap();

        let nomes: Vec<&str> = saida.data["interfaces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|iface| iface["name"].as_str().unwrap())
            .collect();
        assert_eq!(nomes, vec!["ether1", "ether2"], "ether3 está saudável");
        assert_eq!(saida.data["interfaces"][0]["utilization_pct"], 95.0);
        assert_eq!(saida.data["total"], 3);
        assert!(saida.chart.is_none());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn grafico_de_trafego_manda_pontos_para_a_tela_e_resumo_para_a_ia() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let dev = aparelho(&ctx).await;
        let wan = porta(&ctx, dev.id, 1, "ether1", "up").await;
        trafego(&ctx, dev.id, wan.id, "inBps", &[10.0, 20.0, 30.0]).await;
        trafego(&ctx, dev.id, wan.id, "outBps", &[1.0, 2.0, 3.0]).await;

        let saida = ToolRegistry::new(false)
            .execute(
                &ctx,
                "chart_interface_traffic",
                r#"{"device": "10.0.0.1", "interface": "ETHER1", "hours": "2"}"#,
            )
            .await
            .unwrap();

        let grafico = saida.chart.expect("gráfico para a tela");
        assert_eq!(grafico.series.len(), 2);
        assert_eq!(grafico.series[0].points.len(), 3);
        assert_eq!(saida.data["in_bps"]["max"], 30.0);
        assert_eq!(saida.data["out_bps"]["avg"], 2.0);
        assert!(
            saida.data.get("points").is_none() && !saida.data.to_string().contains("\"time\""),
            "os pontos não voltam para a IA"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn historico_do_monitor_traz_uptime_e_falhas_recentes() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let dev = aparelho(&ctx).await;
        let mon = ping(&ctx, dev.id).await;
        checagem(&ctx, mon.id, 3, "up", 12.0).await;
        checagem(&ctx, mon.id, 2, "down", 0.0).await;
        checagem(&ctx, mon.id, 1, "up", 18.0).await;

        let registro = ToolRegistry::new(false);
        let saida = registro
            .execute(
                &ctx,
                "get_monitor_history",
                r#"{"device": "Borda Principal"}"#,
            )
            .await
            .unwrap();
        assert_eq!(saida.data["monitor"]["id"], mon.id);
        assert_eq!(saida.data["checks"]["down"], 1);
        assert_eq!(saida.data["recent_failures"][0]["message"], "Timeout");

        let grafico = registro
            .execute(
                &ctx,
                "chart_monitor_latency",
                &format!(r#"{{"monitor_id": {}, "timeframe": "15m"}}"#, mon.id),
            )
            .await
            .unwrap();
        assert!(grafico.chart.is_some());
        assert_eq!(grafico.data["chart_shown"], true);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn alertas_separam_abertos_do_historico() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let dev = aparelho(&ctx).await;
        alerta(&ctx, dev.id, "active", "Link caído").await;
        alerta(&ctx, dev.id, "resolved", "Perda alta").await;

        let registro = ToolRegistry::new(false);
        let abertos = registro.execute(&ctx, "get_alerts", "{}").await.unwrap();
        assert_eq!(abertos.data["total"], 1);
        assert_eq!(abertos.data["alerts"][0]["message"], "Link caído");
        assert_eq!(abertos.data["alerts"][0]["device"], "Borda Principal");

        let resolvidos = registro
            .execute(&ctx, "get_alerts", r#"{"status": "resolved", "hours": 6}"#)
            .await
            .unwrap();
        assert_eq!(resolvidos.data["total"], 1);
        assert_eq!(resolvidos.data["alerts"][0]["duration_min"], 30);

        let resumo = registro
            .execute(&ctx, "get_system_summary", "{}")
            .await
            .unwrap();
        assert_eq!(resumo.data["open_alerts"]["total"], 1);
        assert_eq!(resumo.data["devices"]["by_status"]["warning"], 1);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn alvo_inexistente_volta_como_dado_e_ferramenta_ativa_desligada_nao_roda() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let registro = ToolRegistry::new(false);
        let saida = registro
            .execute(
                &ctx,
                "get_device_detail",
                r#"{"identifier": "inexistente"}"#,
            )
            .await
            .unwrap();
        assert!(saida.data["error"]
            .as_str()
            .unwrap()
            .contains("list_devices"));

        assert!(
            registro
                .execute(&ctx, "ping_host", r#"{"target": "127.0.0.1"}"#)
                .await
                .is_err(),
            "ping fora do registro quando as ferramentas ativas estão desligadas"
        );
    })
    .await;
}
