//! Testes de limpeza e manutenção de histórico em dispositivos e descoberta SNMP.

use backend::{
    app::App,
    models::{
        _entities::{alert_events, metrics, monitor_results},
        devices, monitors,
    },
};
use chrono::Utc;
use loco_rs::testing::prelude::*;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::json;
use serial_test::serial;

use super::prepare_data;

async fn autenticado(request: &mut loco_rs::TestServer, ctx: &loco_rs::app::AppContext) {
    let session = prepare_data::init_user_login(request, ctx).await;
    let (header, value) = prepare_data::auth_header(&session.token);
    request.add_header(header, value);
}

#[tokio::test]
#[serial]
async fn atualizar_dispositivo_com_clear_history_true_apaga_historico_acumulado() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        let dev = devices::ActiveModel {
            name: Set("Roteador Teste".into()),
            r#type: Set("router".into()),
            ip_address: Set(Some("192.168.1.1".into())),
            is_monitored: Set(true),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();

        let mon = monitors::ActiveModel {
            device_id: Set(Some(dev.id)),
            r#type: Set("ping".into()),
            name: Set("Ping Roteador Teste".into()),
            configuration: Set(json!({"host": "192.168.1.1"})),
            interval_seconds: Set(60),
            timeout_seconds: Set(5),
            retry_count: Set(3),
            enabled: Set(true),
            status: Set("up".into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();

        // Inserir histórico
        metrics::ActiveModel {
            device_id: Set(dev.id),
            monitor_id: Set(Some(mon.id)),
            name: Set("latency".into()),
            value: Set(15.2),
            unit: Set("ms".into()),
            recorded_at: Set(Utc::now().into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();

        monitor_results::ActiveModel {
            monitor_id: Set(mon.id),
            status: Set("up".into()),
            started_at: Set(Utc::now().into()),
            finished_at: Set(Utc::now().into()),
            duration_ms: Set(15),
            latency_ms: Set(Some(15.0)),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();

        alert_events::ActiveModel {
            device_id: Set(Some(dev.id)),
            monitor_id: Set(Some(mon.id)),
            status: Set("triggered".into()),
            severity: Set("warning".into()),
            started_at: Set(Utc::now().into()),
            message: Set(Some("Latência alta".into())),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();

        // Verifica que o histórico existe antes da atualização
        assert_eq!(
            metrics::Entity::find()
                .filter(metrics::Column::DeviceId.eq(dev.id))
                .all(&ctx.db)
                .await
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            monitor_results::Entity::find()
                .filter(monitor_results::Column::MonitorId.eq(mon.id))
                .all(&ctx.db)
                .await
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            alert_events::Entity::find()
                .filter(alert_events::Column::DeviceId.eq(dev.id))
                .all(&ctx.db)
                .await
                .unwrap()
                .len(),
            1
        );

        // Atualiza o IP solicitando clearHistory: true
        let resp = request
            .put(&format!("/api/devices/{}", dev.id))
            .json(&json!({
                "ipAddress": "192.168.1.50",
                "clearHistory": true,
            }))
            .await;
        assert_eq!(resp.status_code(), 200);

        let dev_atualizado = devices::Entity::find_by_id(dev.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(dev_atualizado.ip_address.as_deref(), Some("192.168.1.50"));

        // Histórico foi apagado
        assert_eq!(
            metrics::Entity::find()
                .filter(metrics::Column::DeviceId.eq(dev.id))
                .all(&ctx.db)
                .await
                .unwrap()
                .len(),
            0
        );
        assert_eq!(
            monitor_results::Entity::find()
                .filter(monitor_results::Column::MonitorId.eq(mon.id))
                .all(&ctx.db)
                .await
                .unwrap()
                .len(),
            0
        );
        assert_eq!(
            alert_events::Entity::find()
                .filter(alert_events::Column::DeviceId.eq(dev.id))
                .all(&ctx.db)
                .await
                .unwrap()
                .len(),
            0
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn atualizar_dispositivo_com_clear_history_false_preserva_historico() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        let dev = devices::ActiveModel {
            name: Set("Switch Core".into()),
            r#type: Set("switch".into()),
            ip_address: Set(Some("10.0.0.1".into())),
            is_monitored: Set(true),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();

        let mon = monitors::ActiveModel {
            device_id: Set(Some(dev.id)),
            r#type: Set("ping".into()),
            name: Set("Ping Switch Core".into()),
            configuration: Set(json!({"host": "10.0.0.1"})),
            interval_seconds: Set(60),
            timeout_seconds: Set(5),
            retry_count: Set(3),
            enabled: Set(true),
            status: Set("up".into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();

        metrics::ActiveModel {
            device_id: Set(dev.id),
            monitor_id: Set(Some(mon.id)),
            name: Set("latency".into()),
            value: Set(5.0),
            unit: Set("ms".into()),
            recorded_at: Set(Utc::now().into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();

        // Atualiza com clearHistory: false
        let resp = request
            .put(&format!("/api/devices/{}", dev.id))
            .json(&json!({
                "ipAddress": "10.0.0.2",
                "clearHistory": false,
            }))
            .await;
        assert_eq!(resp.status_code(), 200);

        let dev_atualizado = devices::Entity::find_by_id(dev.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(dev_atualizado.ip_address.as_deref(), Some("10.0.0.2"));

        // Histórico foi preservado
        assert_eq!(
            metrics::Entity::find()
                .filter(metrics::Column::DeviceId.eq(dev.id))
                .all(&ctx.db)
                .await
                .unwrap()
                .len(),
            1
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn atualizar_dispositivo_com_interfaces_e_clear_history_true_apaga_metricas_de_interfaces() {
    use backend::models::device_interfaces;

    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        let dev = devices::ActiveModel {
            name: Set("Roteador Borda".into()),
            r#type: Set("router".into()),
            ip_address: Set(Some("172.16.0.1".into())),
            is_monitored: Set(true),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();

        let iface = device_interfaces::ActiveModel {
            device_id: Set(dev.id),
            name: Set("ether1".into()),
            snmp_index: Set(Some(1)),
            admin_status: Set(Some("up".into())),
            oper_status: Set(Some("up".into())),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();

        metrics::ActiveModel {
            device_id: Set(dev.id),
            interface_id: Set(Some(iface.id)),
            name: Set("ifHCInOctets".into()),
            value: Set(1000000.0),
            unit: Set("bytes".into()),
            recorded_at: Set(Utc::now().into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();

        assert_eq!(
            metrics::Entity::find()
                .filter(metrics::Column::InterfaceId.eq(Some(iface.id)))
                .all(&ctx.db)
                .await
                .unwrap()
                .len(),
            1
        );

        let resp = request
            .put(&format!("/api/devices/{}", dev.id))
            .json(&json!({
                "ipAddress": "172.16.0.2",
                "clearHistory": true,
            }))
            .await;
        assert_eq!(resp.status_code(), 200);

        assert_eq!(
            metrics::Entity::find()
                .filter(metrics::Column::InterfaceId.eq(Some(iface.id)))
                .all(&ctx.db)
                .await
                .unwrap()
                .len(),
            0
        );

        // A interface continua cadastrada
        assert!(device_interfaces::Entity::find_by_id(iface.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .is_some());
    })
    .await;
}
