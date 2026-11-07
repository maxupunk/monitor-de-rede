//! Papel por área (ADR 019): o guard da API, o cadastro e a sessão.

use backend::app::App;
use loco_rs::testing::prelude::*;
use netmonitor_core::models::users;
use sea_orm::{ActiveModelTrait, IntoActiveModel, Set};
use serial_test::serial;

use super::prepare_data;

/// Sessão de um usuário com o papel global e os papéis por área dados.
async fn sessao(
    request: &mut loco_rs::TestServer,
    ctx: &loco_rs::app::AppContext,
    role: &str,
    area_roles: serde_json::Value,
) -> users::Model {
    let session = prepare_data::init_operator(ctx).await;
    let mut active = session.user.into_active_model();
    active.role = Set(role.to_owned());
    active.area_roles = Set(Some(area_roles));
    let user = active.update(&ctx.db).await.unwrap();
    let (header, value) = prepare_data::auth_header(&session.token);
    request.add_header(header, value);
    user
}

#[tokio::test]
#[serial]
async fn sem_papel_na_area_e_sem_acesso_e_leitor_le_mas_nao_muda() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        sessao(
            &mut request,
            &ctx,
            "operator",
            serde_json::json!({ "network": "viewer" }),
        )
        .await;

        // Sem papel na VPN: sem acesso, com a área e o caminho na mensagem.
        let vpn = request.get("/api/vpn/peers").await;
        assert_eq!(vpn.status_code(), 403, "{}", vpn.text());
        let corpo: serde_json::Value = serde_json::from_str(&vpn.text()).unwrap();
        assert_eq!(corpo["area"], "vpn");
        assert!(corpo["message"].as_str().unwrap().contains("VPN"));

        // Leitor na Rede: lê, não muda.
        assert_eq!(request.get("/api/networks").await.status_code(), 200);
        let criar = request
            .post("/api/networks")
            .json(&serde_json::json!({ "name": "LAN", "cidr": "10.9.0.0/24" }))
            .await;
        assert_eq!(criar.status_code(), 403, "{}", criar.text());

        // Núcleo pelo papel global (operator): lê e muda.
        assert_eq!(request.get("/api/devices").await.status_code(), 200);
        let site = request
            .post("/api/sites")
            .json(&serde_json::json!({ "name": "Matriz" }))
            .await;
        assert_ne!(site.status_code(), 403, "{}", site.text());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn operador_da_area_muda_nela_mesmo_sendo_leitor_no_nucleo() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        sessao(
            &mut request,
            &ctx,
            "viewer",
            serde_json::json!({ "network": "operator" }),
        )
        .await;

        let criar = request
            .post("/api/networks")
            .json(&serde_json::json!({ "name": "LAN", "cidr": "10.9.0.0/24" }))
            .await;
        assert_ne!(criar.status_code(), 403, "{}", criar.text());
        let site = request
            .post("/api/sites")
            .json(&serde_json::json!({ "name": "Matriz" }))
            .await;
        assert_eq!(site.status_code(), 403, "leitor no Núcleo");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn admin_atribui_papeis_por_area_e_a_sessao_os_le() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        sessao(&mut request, &ctx, "admin", serde_json::json!({})).await;

        let criado = request
            .post("/api/users")
            .json(&serde_json::json!({
                "name": "Operador Docker",
                "email": "docker@example.com",
                "password": "SenhaForte1",
                "role": "viewer",
                "areaRoles": { "servers": "operator" }
            }))
            .await;
        assert_eq!(criado.status_code(), 201, "{}", criado.text());
        let corpo: serde_json::Value = serde_json::from_str(&criado.text()).unwrap();
        assert_eq!(
            corpo["areaRoles"],
            serde_json::json!({ "servers": "operator" })
        );

        let admin_por_area = request
            .post("/api/users")
            .json(&serde_json::json!({
                "name": "Errado",
                "email": "errado@example.com",
                "password": "SenhaForte1",
                "role": "viewer",
                "areaRoles": { "servers": "admin" }
            }))
            .await;
        assert_eq!(
            admin_por_area.status_code(),
            422,
            "{}",
            admin_por_area.text()
        );

        let eu = request.get("/api/auth/me").await;
        let eu: serde_json::Value = serde_json::from_str(&eu.text()).unwrap();
        assert_eq!(eu["role"], "admin");
        assert!(eu["areaRoles"].is_object());
    })
    .await;
}
