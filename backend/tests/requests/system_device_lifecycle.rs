//! Fase 7 — validação final do dispositivo do sistema ao longo do seu ciclo.
//!
//! São os três casos de backup que a Fase 1 previu e que só podiam ser
//! verificados depois de tudo montado: exportar com o dispositivo presente,
//! restaurar num sistema que já tem o seu, e restaurar um arquivo **anterior**
//! à feature. Em todos, o que se afirma é o mesmo: o dispositivo volta correto,
//! o log interno seguinte vai para ele, e não sobra linha órfã.

use backend::{
    app::App,
    models::{alert_rules, devices, monitors},
    services::{
        devices::system_device::{self, SystemDeviceService, NETMONITOR_KEY},
        monitoring::managed::SYSTEM_HEALTH,
    },
};
use loco_rs::testing::prelude::*;
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use serde_json::Value;
use serial_test::serial;

use super::prepare_data;

async fn autenticado(request: &mut loco_rs::TestServer, ctx: &loco_rs::app::AppContext) {
    let session = prepare_data::init_user_login(request, ctx).await;
    let (header, value) = prepare_data::auth_header(&session.token);
    request.add_header(header, value);
}

async fn quantos_do_sistema(db: &sea_orm::DatabaseConnection) -> u64 {
    devices::Entity::find()
        .filter(devices::Column::SystemKey.eq(NETMONITOR_KEY))
        .count(db)
        .await
        .expect("contagem")
}

#[tokio::test]
#[serial]
async fn o_backup_exporta_o_dispositivo_do_sistema_como_qualquer_outro() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;
        SystemDeviceService::new(&ctx.db).ensure().await.unwrap();

        let exportado = request.get("/api/backup/export").await;
        assert_eq!(exportado.status_code(), 200, "{}", exportado.text());
        let arquivo: Value = serde_json::from_str(&exportado.text()).unwrap();

        let do_sistema: Vec<&Value> = arquivo["tables"]["devices"]
            .as_array()
            .expect("devices no arquivo")
            .iter()
            .filter(|device| device["system_key"] == NETMONITOR_KEY)
            .collect();
        assert_eq!(
            do_sistema.len(),
            1,
            "o servidor precisa viajar no arquivo — esconder o dispositivo do backup \
             o transformaria numa categoria paralela"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn restaurar_num_sistema_que_ja_tem_o_seu_nao_duplica_nem_troca_de_dono() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;
        let antes = SystemDeviceService::new(&ctx.db).ensure().await.unwrap();

        let arquivo: Value =
            serde_json::from_str(&request.get("/api/backup/export").await.text()).unwrap();
        let restaurado = request.post("/api/backup/restore").json(&arquivo).await;
        assert_eq!(restaurado.status_code(), 200, "{}", restaurado.text());

        assert_eq!(quantos_do_sistema(&ctx.db).await, 1);
        let depois = SystemDeviceService::new(&ctx.db)
            .find()
            .await
            .unwrap()
            .expect("o dispositivo continua localizável pela chave");
        assert_eq!(depois.id, antes.id, "o ID do arquivo é o mesmo daqui");
        assert_eq!(
            system_device::resolver::current(),
            Some(depois.id),
            "o cache precisa ter sido reexecutado ao fim do restore"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn restaurar_um_arquivo_anterior_a_feature_recria_o_dispositivo() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;
        SystemDeviceService::new(&ctx.db).ensure().await.unwrap();

        // Um arquivo de antes desta feature: tem dispositivos, e nenhum deles
        // declara `system_key`.
        let arquivo = serde_json::json!({
            "formatVersion": 1,
            "appVersion": "anterior",
            "generatedAt": "2026-01-01T00:00:00Z",
            "tables": {
                "sites": [],
                "probes": [],
                "networks": [],
                "devices": [{
                    "id": 1, "site_id": null, "network_id": null, "parent_id": null,
                    "ip_address": "10.0.0.1", "name": "rt-antigo", "type": "router",
                    "vendor": null, "model": null, "serial_number": null, "description": null,
                    "is_monitored": false, "snmp_enabled": false, "snmp_community": null,
                    "snmp_version": null, "snmp_poll_interval_seconds": 60,
                    "access_mode": null, "operating_system": null, "system_key": null,
                    "status": "unknown", "last_seen_at": null,
                    "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"
                }],
                "device_interfaces": [], "device_links": [], "monitors": [],
                "alert_rules": [], "vpn_servers": [], "vpn_peers": [],
                "dns_servers": [], "system_settings": []
            }
        });

        let restaurado = request.post("/api/backup/restore").json(&arquivo).await;
        assert_eq!(restaurado.status_code(), 200, "{}", restaurado.text());

        // O serviço o recria — e o roteador do arquivo não vira o servidor.
        let depois = SystemDeviceService::new(&ctx.db)
            .find()
            .await
            .unwrap()
            .expect("o dispositivo do sistema é recriado após o restore");
        assert_eq!(quantos_do_sistema(&ctx.db).await, 1);
        assert_ne!(
            depois.id, 1,
            "o roteador restaurado não pode virar o dispositivo do sistema"
        );
        assert_eq!(system_device::resolver::current(), Some(depois.id));

        // E a coleta de saúde volta junto: um servidor restaurado que aparece
        // na lista e nunca mais mede nada seria pior que nenhum.
        assert_eq!(
            monitors::Entity::find()
                .filter(monitors::Column::DeviceId.eq(depois.id))
                .filter(monitors::Column::Type.eq(SYSTEM_HEALTH))
                .count(&ctx.db)
                .await
                .unwrap(),
            1
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn as_regras_do_dispositivo_protegido_seguem_o_ciclo_normal() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;
        let device = SystemDeviceService::new(&ctx.db).ensure().await.unwrap();

        // As regras existem, vinculadas ao dispositivo, e são editáveis e
        // removíveis como as de qualquer outro — a proteção é do **dispositivo**,
        // não das suas regras.
        let regras = alert_rules::Entity::find()
            .filter(alert_rules::Column::DeviceId.eq(device.id))
            .all(&ctx.db)
            .await
            .unwrap();
        assert!(!regras.is_empty(), "as regras de saúde foram aplicadas");

        let alvo = regras[0].id;
        let resposta = request
            .put(&format!("/api/alert-rules/{alvo}"))
            .json(&serde_json::json!({"enabled": false}))
            .await;
        assert_eq!(resposta.status_code(), 200, "{}", resposta.text());

        let resposta = request.delete(&format!("/api/alert-rules/{alvo}")).await;
        assert_eq!(resposta.status_code(), 204, "{}", resposta.text());

        // O dispositivo continua lá.
        assert_eq!(quantos_do_sistema(&ctx.db).await, 1);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn a_politica_de_acesso_nao_mudou_por_causa_do_dispositivo_protegido() {
    // A proteção é regra de negócio, e a resposta é 400 (regra de negócio) —
    // **não** 403 (permissão). Se um dia virar 403, alguém terá transformado
    // uma invariante do produto num perfil de acesso, que é o que a Fase 1
    // proíbe.
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;
        let device = SystemDeviceService::new(&ctx.db).ensure().await.unwrap();

        let resposta = request.delete(&format!("/api/devices/{}", device.id)).await;
        assert_eq!(resposta.status_code(), 400);
        assert_ne!(
            resposta.status_code(),
            403,
            "403 significaria uma terceira categoria na política de acesso"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn a_instalacao_vazia_cria_um_unico_conjunto_completo() {
    // O critério de aceite da instalação nova: **um** dispositivo, **um**
    // monitor gerenciado e **um** conjunto de regras de saúde — tudo
    // provisionado no boot, sem intervenção.
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let device = SystemDeviceService::new(&ctx.db)
            .find()
            .await
            .unwrap()
            .expect("o Initializer provisiona o dispositivo no boot");

        assert_eq!(quantos_do_sistema(&ctx.db).await, 1);
        assert_eq!(
            monitors::Entity::find()
                .filter(monitors::Column::Type.eq(SYSTEM_HEALTH))
                .count(&ctx.db)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            alert_rules::Entity::find()
                .filter(alert_rules::Column::DeviceId.eq(device.id))
                .count(&ctx.db)
                .await
                .unwrap(),
            3,
            "CPU, memória e armazenamento"
        );
    })
    .await;
}
