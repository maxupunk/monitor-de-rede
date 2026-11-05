//! Áreas do produto (ADR 017) pela API: estado, impacto, mudança, papel,
//! bloqueio da VPN, auditoria e evidência.

use backend::{
    app::App,
    models::{
        _entities::{audit_logs, networks, vpn_servers},
        users,
    },
    services::users::Role,
};
use loco_rs::testing::prelude::*;
use sea_orm::{ActiveModelTrait, EntityTrait, IntoActiveModel, Set};
use serde_json::{json, Value};
use serial_test::serial;

use super::prepare_data;

async fn session_with_role(
    request: &mut loco_rs::TestServer,
    ctx: &loco_rs::app::AppContext,
    role: Role,
) -> users::Model {
    let session = prepare_data::init_operator(ctx).await;
    let mut active = session.user.into_active_model();
    active.role = Set(role.as_str().to_string());
    let user = active.update(&ctx.db).await.unwrap();
    let (header, value) = prepare_data::auth_header(&session.token);
    request.add_header(header, value);
    user
}

fn area<'a>(body: &'a Value, id: &str) -> &'a Value {
    body["areas"]
        .as_array()
        .expect("areas")
        .iter()
        .find(|item| item["area"] == id)
        .expect("área presente")
}

async fn vpn_server(ctx: &loco_rs::app::AppContext, active: bool) {
    let rede = networks::ActiveModel {
        name: Set("VPN".to_owned()),
        cidr: Set("10.8.0.0/24".to_owned()),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .expect("rede");
    vpn_servers::ActiveModel {
        network_id: Set(rede.id),
        interface_name: Set("wg0".to_owned()),
        listen_port: Set(51820),
        public_key: Set("chave".to_owned()),
        private_key_encrypted: Set("cifrada".to_owned()),
        active: Set(active),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .expect("servidor vpn");
}

#[tokio::test]
#[serial]
async fn instalacao_nova_tem_todas_as_areas_ligadas() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Viewer).await;
        let response = request.get("/api/areas").await;
        assert_eq!(response.status_code(), 200, "{}", response.text());
        let body: Value = serde_json::from_str(&response.text()).unwrap();
        for id in ["core", "network", "servers", "vpn"] {
            assert_eq!(area(&body, id)["enabled"], true, "{id}");
        }
        assert_eq!(area(&body, "core")["optional"], false);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn admin_desliga_e_religa_com_auditoria() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Admin).await;

        let response = request
            .put("/api/areas/servers")
            .json(&json!({ "enabled": false }))
            .await;
        assert_eq!(response.status_code(), 200, "{}", response.text());
        let body: Value = serde_json::from_str(&response.text()).unwrap();
        assert_eq!(area(&body, "servers")["enabled"], false);
        assert_eq!(area(&body, "network")["enabled"], true);

        let estado: Value = serde_json::from_str(&request.get("/api/areas").await.text()).unwrap();
        assert_eq!(area(&estado, "servers")["enabled"], false, "gravado");

        let logs = audit_logs::Entity::find().all(&ctx.db).await.unwrap();
        assert!(
            logs.iter().any(|log| log.description.as_deref()
                == Some("Área Servidores e Docker desligada")),
            "mudança auditada"
        );

        let religa = request
            .put("/api/areas/servers")
            .json(&json!({ "enabled": true }))
            .await;
        assert_eq!(religa.status_code(), 200, "{}", religa.text());
        let body: Value = serde_json::from_str(&religa.text()).unwrap();
        assert_eq!(area(&body, "servers")["enabled"], true);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn operador_e_visualizador_nao_mudam_area() {
    for role in [Role::Operator, Role::Viewer] {
        request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
            session_with_role(&mut request, &ctx, role).await;
            let response = request
                .put("/api/areas/network")
                .json(&json!({ "enabled": false }))
                .await;
            assert_eq!(response.status_code(), 403, "{}", response.text());
            let impacto = request.get("/api/areas/network/impact").await;
            assert_eq!(impacto.status_code(), 403, "{}", impacto.text());
        })
        .await;
    }
}

#[tokio::test]
#[serial]
async fn nucleo_nao_desliga_e_area_desconhecida_e_404() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Admin).await;
        let nucleo = request
            .put("/api/areas/core")
            .json(&json!({ "enabled": false }))
            .await;
        assert_eq!(nucleo.status_code(), 422, "{}", nucleo.text());
        let desconhecida = request
            .put("/api/areas/docker")
            .json(&json!({ "enabled": false }))
            .await;
        assert_eq!(desconhecida.status_code(), 404, "{}", desconhecida.text());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn vpn_com_servidor_ativo_nao_desliga() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Admin).await;
        vpn_server(&ctx, true).await;

        let impacto: Value =
            serde_json::from_str(&request.get("/api/areas/vpn/impact").await.text()).unwrap();
        assert!(impacto["blocker"]["message"].is_string(), "{impacto}");

        let response = request
            .put("/api/areas/vpn")
            .json(&json!({ "enabled": false }))
            .await;
        assert_eq!(response.status_code(), 409, "{}", response.text());
        let estado: Value = serde_json::from_str(&request.get("/api/areas").await.text()).unwrap();
        assert_eq!(area(&estado, "vpn")["enabled"], true, "nada mudou");
        assert_eq!(area(&estado, "vpn")["evidence"]["present"], true);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn vpn_com_servidor_inativo_desliga() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Admin).await;
        vpn_server(&ctx, false).await;
        let response = request
            .put("/api/areas/vpn")
            .json(&json!({ "enabled": false }))
            .await;
        assert_eq!(response.status_code(), 200, "{}", response.text());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn evidencia_de_rede_nao_conta_o_que_o_sistema_cria_sozinho() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Viewer).await;
        // A central se cadastra como dispositivo do sistema e registra o
        // agente local no boot: nenhum dos dois é evidência.
        let estado: Value = serde_json::from_str(&request.get("/api/areas").await.text()).unwrap();
        assert_eq!(area(&estado, "network")["evidence"]["present"], false);
        assert_eq!(area(&estado, "servers")["evidence"]["present"], false);

        networks::ActiveModel {
            name: Set("LAN".to_owned()),
            cidr: Set("192.168.0.0/24".to_owned()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("rede");
        let estado: Value = serde_json::from_str(&request.get("/api/areas").await.text()).unwrap();
        assert_eq!(area(&estado, "network")["evidence"]["present"], true);
        assert_eq!(
            area(&estado, "network")["evidence"]["reason"],
            "1 rede IP cadastrada"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn impacto_de_desligar_a_rede_lista_consequencias() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Admin).await;
        let response = request.get("/api/areas/network/impact").await;
        assert_eq!(response.status_code(), 200, "{}", response.text());
        let body: Value = serde_json::from_str(&response.text()).unwrap();
        assert!(!body["consequences"].as_array().unwrap().is_empty());
        assert!(body["blocker"].is_null());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn onboarding_so_servidores_nao_cobra_rede() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Admin).await;

        let antes: Value =
            serde_json::from_str(&request.get("/api/settings/onboarding").await.text()).unwrap();
        assert_eq!(antes["needsOnboarding"], true);
        assert_eq!(antes["intentChosen"], false);

        let response = request
            .post("/api/settings/onboarding/intent")
            .json(&json!({ "network": false, "servers": true, "vpn": false }))
            .await;
        assert_eq!(response.status_code(), 200, "{}", response.text());
        let body: Value = serde_json::from_str(&response.text()).unwrap();
        assert_eq!(area(&body, "network")["enabled"], false);
        assert_eq!(area(&body, "servers")["enabled"], true);
        assert_eq!(area(&body, "vpn")["enabled"], false);

        let depois: Value =
            serde_json::from_str(&request.get("/api/settings/onboarding").await.text()).unwrap();
        assert_eq!(depois["intentChosen"], true);
        assert_eq!(
            depois["needsOnboarding"], false,
            "sem a área Rede, site e rede não são cobrados"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn onboarding_com_rede_continua_cobrando_site_e_rede() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Admin).await;
        let response = request
            .post("/api/settings/onboarding/intent")
            .json(&json!({ "network": true, "servers": false, "vpn": true }))
            .await;
        assert_eq!(response.status_code(), 200, "{}", response.text());
        let status: Value =
            serde_json::from_str(&request.get("/api/settings/onboarding").await.text()).unwrap();
        assert_eq!(status["needsOnboarding"], true);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn onboarding_exige_rede_ou_servidores_e_admin() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Admin).await;
        let nada = request
            .post("/api/settings/onboarding/intent")
            .json(&json!({ "network": false, "servers": false, "vpn": true }))
            .await;
        assert_eq!(nada.status_code(), 422, "{}", nada.text());
    })
    .await;
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Operator).await;
        let operador = request
            .post("/api/settings/onboarding/intent")
            .json(&json!({ "network": true, "servers": true, "vpn": true }))
            .await;
        assert_eq!(operador.status_code(), 403, "{}", operador.text());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn monitor_de_container_continua_rodando_com_servidores_desligado() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Admin).await;
        let monitor = backend::models::_entities::monitors::ActiveModel {
            name: Set("web".to_owned()),
            r#type: Set("container".to_owned()),
            configuration: Set(json!({ "target": "web" })),
            enabled: Set(true),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("monitor");
        let response = request
            .put("/api/areas/servers")
            .json(&json!({ "enabled": false }))
            .await;
        assert_eq!(response.status_code(), 200, "{}", response.text());

        backend::tasks::scheduler_run::run_cycle(&ctx)
            .await
            .expect("ciclo");
        let depois = backend::models::_entities::monitors::Entity::find_by_id(monitor.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .expect("monitor");
        assert!(
            depois.next_run_at.is_some(),
            "o ciclo executou o monitor: monitor nunca para pela área"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn varredura_periodica_pausa_com_a_rede_desligada_e_volta_ao_religar() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Admin).await;
        networks::ActiveModel {
            name: Set("LAN".to_owned()),
            cidr: Set("127.0.0.0/30".to_owned()),
            scan_enabled: Set(true),
            active: Set(true),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("rede");
        request
            .put("/api/areas/network")
            .json(&json!({ "enabled": false }))
            .await;
        let pausada = backend::services::discovery::queue::schedule_due_networks(&ctx.db)
            .await
            .expect("agendar");
        assert_eq!(pausada, 0, "área desligada não enfileira varredura");

        request
            .put("/api/areas/network")
            .json(&json!({ "enabled": true }))
            .await;
        let retomada = backend::services::discovery::queue::schedule_due_networks(&ctx.db)
            .await
            .expect("agendar");
        assert_eq!(retomada, 1, "religada, a rede volta a ser varrida");
    })
    .await;
}
