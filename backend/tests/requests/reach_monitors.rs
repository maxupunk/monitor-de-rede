//! Fase 1 do roadmap de ajustes: **o dispositivo do sistema não recebe monitor
//! de alcance**, e nenhum dispositivo sem endereço ganha um alvo inventado.
//!
//! Os quatro caminhos que criam monitor são cobertos aqui:
//! `POST /api/monitors`, `PUT /api/monitors/{id}`, o `sync_device_monitor` do
//! cadastro de dispositivos e a limpeza de boot. O quinto —
//! `vpn::monitor_provisioner::provision` — é chamado direto, sem HTTP, e por
//! isso é testado pela função e não pela rota.

use backend::{
    app::App,
    models::{devices, monitors},
    services::{
        devices::system_device::SystemDeviceService,
        monitoring::{managed::SYSTEM_HEALTH, reachability},
        vpn::monitor_provisioner::{provision, MonitorProvisioningOptions},
    },
};
use loco_rs::testing::prelude::*;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::{json, Value};
use serial_test::serial;

use super::prepare_data;

async fn autenticado(request: &mut loco_rs::TestServer, ctx: &loco_rs::app::AppContext) {
    let session = prepare_data::init_user_login(request, ctx).await;
    let (header, value) = prepare_data::auth_header(&session.token);
    request.add_header(header, value);
}

async fn monitores_de(db: &sea_orm::DatabaseConnection, device_id: i64) -> Vec<monitors::Model> {
    monitors::Entity::find()
        .filter(monitors::Column::DeviceId.eq(device_id))
        .all(db)
        .await
        .expect("monitores do dispositivo")
}

#[tokio::test]
#[serial]
async fn criar_monitor_de_alcance_para_o_servidor_e_recusado_por_regra_de_negocio() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;
        let servidor = SystemDeviceService::new(&ctx.db).ensure().await.unwrap();

        for kind in ["ping", "tcp", "http", "https", "dns"] {
            let resposta = request
                .post("/api/monitors")
                .json(&json!({
                    "type": kind,
                    "name": format!("Checagem {kind}"),
                    "deviceId": servidor.id,
                    "target": "127.0.0.1",
                }))
                .await;
            assert_eq!(
                resposta.status_code(),
                400,
                "{kind} deveria ser recusado por regra de negócio, não por validação: {}",
                resposta.text()
            );
            assert!(
                resposta.text().contains("não é alcançado pela rede"),
                "a recusa precisa dizer por quê: {}",
                resposta.text()
            );
        }

        assert!(
            monitores_de(&ctx.db, servidor.id)
                .await
                .iter()
                .all(|monitor| !reachability::is_reach_check(&monitor.r#type)),
            "nenhum monitor de alcance pode ter sido gravado"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn mover_um_ping_existente_para_o_servidor_e_recusado() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;
        let servidor = SystemDeviceService::new(&ctx.db).ensure().await.unwrap();

        let criado = request
            .post("/api/monitors")
            .json(&json!({ "type": "ping", "name": "Ping gateway", "target": "127.0.0.1" }))
            .await;
        assert_eq!(criado.status_code(), 201, "{}", criado.text());
        let monitor: Value = serde_json::from_str(&criado.text()).unwrap();
        let monitor_id = monitor["id"].as_i64().expect("id do monitor");

        // Sem esta guarda no `PUT`, o caminho de edição devolveria ao servidor
        // justamente o ping que o boot removeu.
        let movido = request
            .put(&format!("/api/monitors/{monitor_id}"))
            .json(&json!({ "deviceId": servidor.id }))
            .await;
        assert_eq!(movido.status_code(), 400, "{}", movido.text());
        assert!(movido.text().contains("não é alcançado pela rede"));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn a_coleta_de_saude_continua_valendo_para_o_servidor() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let servidor = SystemDeviceService::new(&ctx.db).ensure().await.unwrap();
        backend::services::monitoring::managed::ensure_system_health_monitor(&ctx.db, servidor.id)
            .await
            .expect("a coleta de saúde é o que mede o servidor");

        let monitores = monitores_de(&ctx.db, servidor.id).await;
        assert_eq!(monitores.len(), 1);
        assert_eq!(monitores[0].r#type, SYSTEM_HEALTH);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn um_ping_preexistente_no_servidor_e_removido_no_boot() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let servidor = SystemDeviceService::new(&ctx.db).ensure().await.unwrap();

        // Simula a instalação que atualizou no meio do caminho: a linha entrou
        // antes de a guarda existir, e por isso é gravada direto.
        let herdado = monitors::ActiveModel {
            device_id: Set(Some(servidor.id)),
            r#type: Set("ping".into()),
            name: Set("Ping Servidor NetMonitor".into()),
            configuration: Set(json!({ "host": "Servidor NetMonitor" })),
            interval_seconds: Set(60),
            timeout_seconds: Set(10),
            retry_count: Set(3),
            enabled: Set(true),
            status: Set("unknown".into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();

        let removidos = reachability::purge_system_device(&ctx.db, &servidor)
            .await
            .expect("a limpeza de boot não pode falhar");
        assert_eq!(removidos.len(), 1);
        assert_eq!(removidos[0].id, herdado.id);

        assert!(
            monitores_de(&ctx.db, servidor.id)
                .await
                .iter()
                .all(|monitor| !reachability::is_reach_check(&monitor.r#type)),
            "o ping herdado precisa sair do banco"
        );

        // Idempotente: o segundo boot não tem nada a remover nem falha.
        assert!(reachability::purge_system_device(&ctx.db, &servidor)
            .await
            .unwrap()
            .is_empty());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn a_limpeza_de_boot_preserva_a_coleta_de_saude() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let servidor = SystemDeviceService::new(&ctx.db).ensure().await.unwrap();
        backend::services::monitoring::managed::ensure_system_health_monitor(&ctx.db, servidor.id)
            .await
            .unwrap();

        reachability::purge_system_device(&ctx.db, &servidor)
            .await
            .unwrap();

        let restantes = monitores_de(&ctx.db, servidor.id).await;
        assert_eq!(
            restantes.len(),
            1,
            "a limpeza remove alcance, não tudo que existe no servidor"
        );
        assert_eq!(restantes[0].r#type, SYSTEM_HEALTH);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn dispositivo_comum_sem_ip_nao_ganha_monitor_de_alcance() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        let criado = request
            .post("/api/devices")
            .json(&json!({
                "name": "Switch sem endereço",
                "type": "switch",
                "isMonitored": true,
            }))
            .await;
        assert_eq!(criado.status_code(), 201, "{}", criado.text());
        let device: Value = serde_json::from_str(&criado.text()).unwrap();
        let device_id = device["id"].as_i64().expect("id do dispositivo");

        assert!(
            monitores_de(&ctx.db, device_id).await.is_empty(),
            "um ping contra o **nome** do equipamento só poderia falhar"
        );

        // E o motivo fica disponível para a tela, em vez de a interface deduzir.
        let capacidades = request
            .get(&format!("/api/devices/{device_id}/capabilities"))
            .await;
        assert_eq!(capacidades.status_code(), 200, "{}", capacidades.text());
        let corpo: Value = serde_json::from_str(&capacidades.text()).unwrap();
        assert!(corpo["reachMonitorBlockedReason"]
            .as_str()
            .is_some_and(|motivo| motivo.contains("endereço IP")));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn dispositivo_comum_com_ip_continua_ganhando_o_ping_de_sempre() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        let criado = request
            .post("/api/devices")
            .json(&json!({
                "name": "Roteador da borda",
                "type": "router",
                "ipAddress": "192.168.99.1",
                "isMonitored": true,
            }))
            .await;
        assert_eq!(criado.status_code(), 201, "{}", criado.text());
        let device: Value = serde_json::from_str(&criado.text()).unwrap();
        let device_id = device["id"].as_i64().expect("id do dispositivo");

        let monitores = monitores_de(&ctx.db, device_id).await;
        assert_eq!(monitores.len(), 1, "o ping automático continua nascendo");
        assert_eq!(monitores[0].r#type, "ping");
        assert_eq!(monitores[0].configuration["host"], "192.168.99.1");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn informar_o_ip_depois_provisiona_o_ping_que_faltava() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        let criado = request
            .post("/api/devices")
            .json(&json!({
                "name": "AP do galpão",
                "type": "access_point",
                "isMonitored": true,
            }))
            .await;
        let device: Value = serde_json::from_str(&criado.text()).unwrap();
        let device_id = device["id"].as_i64().expect("id do dispositivo");
        assert!(monitores_de(&ctx.db, device_id).await.is_empty());

        let atualizado = request
            .put(&format!("/api/devices/{device_id}"))
            .json(&json!({ "ipAddress": "192.168.99.50" }))
            .await;
        assert_eq!(atualizado.status_code(), 200, "{}", atualizado.text());

        let monitores = monitores_de(&ctx.db, device_id).await;
        assert_eq!(
            monitores.len(),
            1,
            "o motivo do bloqueio deixou de valer; o ping precisa nascer agora"
        );
        assert_eq!(monitores[0].configuration["host"], "192.168.99.50");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn o_provisionador_da_vpn_respeita_as_mesmas_duas_regras() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let servidor = SystemDeviceService::new(&ctx.db).ensure().await.unwrap();
        let erro = provision(&ctx.db, &servidor, &MonitorProvisioningOptions::default())
            .await
            .expect_err("o servidor não é peer alcançável");
        assert!(erro.to_string().contains("não é alcançado pela rede"));

        let sem_ip = devices::ActiveModel {
            name: Set("Peer sem endereço".into()),
            r#type: Set("server".into()),
            is_monitored: Set(true),
            snmp_enabled: Set(false),
            status: Set("unknown".into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();
        let criados = provision(
            &ctx.db,
            &sem_ip,
            &MonitorProvisioningOptions {
                snmp_enabled: true,
                ..Default::default()
            },
        )
        .await
        .expect("sem endereço não é erro: é só não haver o que provisionar");
        assert!(
            criados.is_empty(),
            "nem o ping nem o SNMP têm alvo quando o peer não tem endereço"
        );
    })
    .await;
}
