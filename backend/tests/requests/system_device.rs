//! Fase 1 — identidade do dispositivo do sistema.
//!
//! O que estes testes protegem não é a existência de uma linha, e sim a
//! **estabilidade da referência**: a instalação sempre encontra o mesmo
//! dispositivo pela chave `netmonitor`, sem assumir um ID, sem duplicá-lo no
//! segundo boot e — o caso que quebrava tudo em silêncio — sem passar a
//! apontar para outro equipamento depois de uma restauração de backup.

use backend::{
    app::App,
    models::devices,
    services::devices::system_device::{
        self, ProposedIdentity, SystemDeviceService, NETMONITOR_KEY,
    },
};
use loco_rs::testing::prelude::*;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, Set};
use serde_json::Value;
use serial_test::serial;

use super::prepare_data;

async fn autenticado(request: &mut loco_rs::TestServer, ctx: &loco_rs::app::AppContext) {
    let session = prepare_data::init_user_login(request, ctx).await;
    let (header, value) = prepare_data::auth_header(&session.token);
    request.add_header(header, value);
}

async fn quantos(db: &sea_orm::DatabaseConnection) -> u64 {
    devices::Entity::find()
        .filter(devices::Column::SystemKey.eq(NETMONITOR_KEY))
        .count(db)
        .await
        .expect("contagem")
}

#[tokio::test]
#[serial]
async fn o_primeiro_boot_cria_exatamente_um_e_o_segundo_nao_duplica() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let servico = SystemDeviceService::new(&ctx.db);

        let primeiro = servico.ensure().await.expect("primeiro boot");
        let segundo = servico.ensure().await.expect("segundo boot");

        assert_eq!(primeiro.id, segundo.id, "o segundo boot duplicou a linha");
        assert_eq!(quantos(&ctx.db).await, 1);
        assert_eq!(primeiro.system_key.as_deref(), Some(NETMONITOR_KEY));
        // Nenhum vínculo fictício: site, rede e pai não representam nada real.
        assert!(primeiro.site_id.is_none());
        assert!(primeiro.network_id.is_none());
        assert!(primeiro.ip_address.is_none());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn boots_concorrentes_convergem_para_a_mesma_linha() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let a = ctx.db.clone();
        let b = ctx.db.clone();
        let (um, dois) = tokio::join!(
            async move { SystemDeviceService::new(&a).ensure().await },
            async move { SystemDeviceService::new(&b).ensure().await },
        );

        let um = um.expect("boot concorrente A");
        let dois = dois.expect("boot concorrente B");
        assert_eq!(um.id, dois.id, "a corrida produziu dois dispositivos");
        assert_eq!(quantos(&ctx.db).await, 1);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn o_resolvedor_publica_o_id_e_esquece_ao_invalidar() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        system_device::resolver::invalidate();
        assert_eq!(system_device::resolver::current(), None);

        let device = SystemDeviceService::new(&ctx.db).ensure().await.unwrap();
        assert_eq!(system_device::resolver::current(), Some(device.id));

        system_device::resolver::invalidate();
        assert_eq!(system_device::resolver::current(), None);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn ninguem_exclui_o_dispositivo_do_sistema_nem_o_admin() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;
        let device = SystemDeviceService::new(&ctx.db).ensure().await.unwrap();

        let resposta = request.delete(&format!("/api/devices/{}", device.id)).await;
        assert_eq!(
            resposta.status_code(),
            400,
            "regra de negócio, não permissão: {}",
            resposta.text()
        );
        let corpo: Value = serde_json::from_str(&resposta.text()).expect("json do erro");
        let mensagem = corpo["message"].as_str().unwrap_or_default();
        assert!(
            mensagem.contains("não pode ser excluído"),
            "mensagem em português esperada, veio {mensagem:?}"
        );

        assert_eq!(quantos(&ctx.db).await, 1, "o dispositivo sumiu mesmo assim");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn a_identidade_tecnica_e_protegida_mas_o_nome_continua_editavel() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;
        let device = SystemDeviceService::new(&ctx.db).ensure().await.unwrap();
        let url = format!("/api/devices/{}", device.id);

        for corpo in [
            serde_json::json!({"ipAddress": "10.0.0.9"}),
            serde_json::json!({"type": "router"}),
            serde_json::json!({"snmpEnabled": true}),
        ] {
            let resposta = request.put(&url).json(&corpo).await;
            assert_eq!(
                resposta.status_code(),
                400,
                "alteração deveria ser recusada: {corpo}"
            );
        }

        // Renomear é legítimo: a chave não depende do nome exibido.
        let resposta = request
            .put(&url)
            .json(&serde_json::json!({"name": "Meu servidor"}))
            .await;
        assert_eq!(resposta.status_code(), 200, "{}", resposta.text());

        let relido = SystemDeviceService::new(&ctx.db)
            .find()
            .await
            .unwrap()
            .expect("o dispositivo continua localizável pela chave");
        assert_eq!(relido.name, "Meu servidor");
        assert_eq!(relido.id, device.id);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn depois_do_restore_o_id_cacheado_nao_aponta_para_outro_equipamento() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        // Estado "antes do restore": o dispositivo existe com um ID qualquer e
        // o resolvedor o publicou.
        let antes = SystemDeviceService::new(&ctx.db).ensure().await.unwrap();
        assert_eq!(system_device::resolver::current(), Some(antes.id));

        // O `wipe` + recarga do restore recoloca as linhas com os IDs do
        // arquivo. Simulamos o pior caso: o ID antigo passa a pertencer a
        // outro equipamento.
        devices::Entity::delete_by_id(antes.id)
            .exec(&ctx.db)
            .await
            .unwrap();
        let intruso = devices::ActiveModel {
            id: Set(antes.id),
            name: Set("Roteador da filial".to_string()),
            r#type: Set("router".to_string()),
            status: Set("unknown".to_string()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();

        // O que o restore faz ao terminar.
        system_device::resolver::invalidate();
        let depois = SystemDeviceService::new(&ctx.db).ensure().await.unwrap();

        assert_ne!(
            depois.id, intruso.id,
            "o dispositivo do sistema virou o roteador da filial"
        );
        assert_eq!(system_device::resolver::current(), Some(depois.id));
        assert_eq!(quantos(&ctx.db).await, 1);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn dispositivo_comum_nao_e_protegido() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;
        let resposta = request
            .post("/api/devices")
            .json(&serde_json::json!({"name": "comum", "type": "router"}))
            .await;
        assert_eq!(resposta.status_code(), 201, "{}", resposta.text());
        let corpo: Value = serde_json::from_str(&resposta.text()).unwrap();
        let id = corpo["id"].as_i64().expect("id");

        let comum = devices::Entity::find_by_id(id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert!(!system_device::is_protected(&comum));
        assert!(system_device::ensure_deletable(&comum).is_ok());
        assert!(system_device::ensure_identity_preserved(
            &comum,
            &ProposedIdentity {
                ip_address: Some("10.0.0.1"),
                ..Default::default()
            }
        )
        .is_ok());

        let resposta = request.delete(&format!("/api/devices/{id}")).await;
        assert_eq!(resposta.status_code(), 204, "{}", resposta.text());
    })
    .await;
}
