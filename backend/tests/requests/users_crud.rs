use backend::{app::App, models::users, services::users::Role};
use loco_rs::testing::prelude::*;
use sea_orm::{ActiveModelTrait, IntoActiveModel, Set};
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

#[tokio::test]
#[serial]
async fn administrador_cria_lista_atualiza_e_exclui_usuario() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Admin).await;

        let created = request
            .post("/api/users")
            .json(&serde_json::json!({
                "name": "Leitura NOC",
                "email": "noc@example.com",
                "password": "SenhaForte1",
                "role": "viewer"
            }))
            .await;
        assert_eq!(created.status_code(), 201, "{}", created.text());
        let body: serde_json::Value = serde_json::from_str(&created.text()).unwrap();
        assert_eq!(body["role"], "viewer");
        assert_eq!(body["active"], true);
        let id = body["id"].as_i64().unwrap();

        let listed = request.get("/api/users").await;
        assert_eq!(listed.status_code(), 200, "{}", listed.text());
        let rows: serde_json::Value = serde_json::from_str(&listed.text()).unwrap();
        assert_eq!(rows.as_array().unwrap().len(), 2);

        let updated = request
            .put(&format!("/api/users/{id}"))
            .json(&serde_json::json!({
                "name": "Operador NOC",
                "email": "noc@example.com",
                "password": "",
                "role": "operator",
                "active": true
            }))
            .await;
        assert_eq!(updated.status_code(), 200, "{}", updated.text());
        let body: serde_json::Value = serde_json::from_str(&updated.text()).unwrap();
        assert_eq!(body["fullName"], "Operador NOC");
        assert_eq!(body["role"], "operator");

        let deleted = request.delete(&format!("/api/users/{id}")).await;
        assert_eq!(deleted.status_code(), 204, "{}", deleted.text());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn visualizador_pode_ler_mas_nao_escrever() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Viewer).await;

        let read = request.get("/api/sites").await;
        assert_eq!(read.status_code(), 200, "{}", read.text());

        let write = request
            .post("/api/sites")
            .json(&serde_json::json!({ "name": "Bloqueado" }))
            .await;
        assert_eq!(write.status_code(), 403, "{}", write.text());
        assert!(write.text().contains("perfil"));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn operador_nao_gerencia_usuarios() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Operator).await;
        let response = request.get("/api/users").await;
        assert_eq!(response.status_code(), 403, "{}", response.text());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn administrador_nao_remove_a_si_mesmo_nem_o_ultimo_admin() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let admin = session_with_role(&mut request, &ctx, Role::Admin).await;

        let deleted = request.delete(&format!("/api/users/{}", admin.id)).await;
        assert_eq!(deleted.status_code(), 409, "{}", deleted.text());

        let demoted = request
            .put(&format!("/api/users/{}", admin.id))
            .json(&serde_json::json!({
                "name": admin.name,
                "email": admin.email,
                "password": "",
                "role": "viewer",
                "active": true
            }))
            .await;
        assert_eq!(demoted.status_code(), 409, "{}", demoted.text());
    })
    .await;
}
