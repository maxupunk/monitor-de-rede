//! Armazenamentos: cadastro, segredos e backup guardado num destino.

use backend::{app::App, models::users, services::users::Role};
use loco_rs::{app::AppContext, testing::prelude::*, TestServer};
use sea_orm::{ActiveModelTrait, IntoActiveModel, Set};
use serde_json::{json, Value};
use serial_test::serial;

use super::prepare_data;

async fn session_with_role(request: &mut TestServer, ctx: &AppContext, role: Role) -> users::Model {
    let session = prepare_data::init_operator(ctx).await;
    let mut active = session.user.into_active_model();
    active.role = Set(role.as_str().to_string());
    let user = active.update(&ctx.db).await.expect("papel");
    let (header, value) = prepare_data::auth_header(&session.token);
    request.add_header(header, value);
    user
}

fn body(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or(Value::Null)
}

/// Raiz local isolada por teste: sem ela o backup cairia em `./storage`, na
/// árvore do projeto.
fn isolated_local_root() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temporário");
    std::env::set_var("STORAGE_LOCAL_ROOT", dir.path());
    dir
}

/// O ciclo que dá sentido ao recurso: cadastrar a pasta, mandar a cópia,
/// encontrá-la na lista e restaurar a partir dela.
#[tokio::test]
#[serial]
async fn backup_numa_pasta_local_vai_e_volta() {
    let root = isolated_local_root();
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Admin).await;

        let site = request
            .post("/api/sites")
            .json(&json!({ "name": "Matriz" }))
            .await;
        assert_eq!(site.status_code(), 201, "{}", site.text());

        let created = request
            .post("/api/storages")
            .json(&json!({
                "name": "Disco do servidor",
                "provider": "local",
                "config": { "type": "local", "basePath": "copias" },
                "backupEnabled": true,
                "backupIntervalHours": 24,
                "backupRetention": 7
            }))
            .await;
        assert_eq!(created.status_code(), 201, "{}", created.text());
        let id = body(&created.text())["id"].as_i64().unwrap();
        // Nunca fez backup: o próximo é "agora".
        assert!(body(&created.text())["nextBackupAt"].is_string());

        let tested = request.post(&format!("/api/storages/{id}/test")).await;
        assert_eq!(tested.status_code(), 200, "{}", tested.text());
        assert_eq!(body(&tested.text())["ok"], json!(true), "{}", tested.text());

        let ran = request.post(&format!("/api/storages/{id}/backups")).await;
        assert_eq!(ran.status_code(), 201, "{}", ran.text());
        let key = body(&ran.text())["backup"]["key"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(
            key.starts_with("netmonitor-backups/netmonitor-backup-"),
            "{key}"
        );
        assert!(root.path().join("copias").join(&key).is_file());

        let listed = request.get(&format!("/api/storages/{id}/backups")).await;
        assert_eq!(body(&listed.text()).as_array().map(Vec::len), Some(1));

        let shown = request.get("/api/storages").await;
        assert_eq!(body(&shown.text())[0]["lastBackupStatus"], json!("success"));

        let preview = request
            .post(&format!("/api/storages/{id}/backups/preview"))
            .json(&json!({ "key": key }))
            .await;
        assert_eq!(preview.status_code(), 200, "{}", preview.text());
        let sites = body(&preview.text())["tables"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["table"] == "sites")
            .map(|row| row["rows"].clone());
        assert_eq!(sites, Some(json!(1)));

        // Some o site depois do backup; a restauração o traz de volta.
        let site_id = body(&site.text())["id"].as_i64().unwrap();
        let deleted = request.delete(&format!("/api/sites/{site_id}")).await;
        assert!(deleted.status_code().is_success(), "{}", deleted.text());

        let restored = request
            .post(&format!("/api/storages/{id}/backups/restore"))
            .json(&json!({ "key": key }))
            .await;
        assert_eq!(restored.status_code(), 200, "{}", restored.text());
        let sites = request.get("/api/sites").await;
        assert!(sites.text().contains("Matriz"), "{}", sites.text());
    })
    .await;
    std::env::remove_var("STORAGE_LOCAL_ROOT");
}

/// A credencial entra e não sai: nem na criação, nem no detalhe, nem na
/// lista. Editar sem redigitar a senha mantém a gravada.
#[tokio::test]
#[serial]
async fn o_segredo_do_s3_nunca_volta_para_a_tela() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Admin).await;

        let payload = |secret: &str| {
            json!({
                "name": "Bucket da empresa",
                "provider": "cloudflare_r2",
                "config": {
                    "type": "s3",
                    "bucket": "copias",
                    "endpoint": "https://conta.r2.cloudflarestorage.com",
                    "accessKeyId": "AKIA-TESTE",
                    "secretAccessKey": secret
                },
                "backupEnabled": false,
                "backupIntervalHours": 24,
                "backupRetention": 14
            })
        };

        let created = request
            .post("/api/storages")
            .json(&payload("segredo-real"))
            .await;
        assert_eq!(created.status_code(), 201, "{}", created.text());
        assert!(!created.text().contains("segredo-real"));
        let id = body(&created.text())["id"].as_i64().unwrap();
        assert_eq!(
            body(&created.text())["secretsSet"],
            json!(["secretAccessKey"])
        );
        assert_eq!(body(&created.text())["target"], json!("s3://copias"));

        let updated = request
            .put(&format!("/api/storages/{id}"))
            .json(&payload(""))
            .await;
        assert_eq!(updated.status_code(), 200, "{}", updated.text());
        assert_eq!(
            body(&updated.text())["secretsSet"],
            json!(["secretAccessKey"])
        );

        let detail = request.get(&format!("/api/storages/{id}")).await;
        let list = request.get("/api/storages").await;
        for text in [detail.text(), list.text()] {
            assert!(!text.contains("segredo-real"), "{text}");
        }
    })
    .await;
}

#[tokio::test]
#[serial]
async fn cadastro_invalido_e_recusado_com_a_razao() {
    let _root = isolated_local_root();
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Admin).await;

        // MinIO sem endpoint não tem para onde mandar.
        let minio = request
            .post("/api/storages")
            .json(&json!({
                "name": "MinIO",
                "provider": "minio",
                "config": { "type": "s3", "bucket": "b", "accessKeyId": "k", "secretAccessKey": "s" },
                "backupIntervalHours": 24,
                "backupRetention": 14
            }))
            .await;
        assert_eq!(minio.status_code(), 422, "{}", minio.text());
        assert!(minio.text().contains("endpoint"), "{}", minio.text());

        // Pasta local que escapa da raiz alcançaria o banco do sistema.
        let escape = request
            .post("/api/storages")
            .json(&json!({
                "name": "Fuga",
                "provider": "local",
                "config": { "type": "local", "basePath": "../" },
                "backupIntervalHours": 24,
                "backupRetention": 14
            }))
            .await;
        assert_eq!(escape.status_code(), 422, "{}", escape.text());
    })
    .await;
    std::env::remove_var("STORAGE_LOCAL_ROOT");
}

#[tokio::test]
#[serial]
async fn operador_nao_ve_os_armazenamentos() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Operator).await;
        let response = request.get("/api/storages").await;
        assert_eq!(response.status_code(), 403, "{}", response.text());
    })
    .await;
}
