//! Conexões de banco pela API: cadastro, senha, validações, permissão e — com
//! um PostgreSQL local — backup e restauração de ponta a ponta.

use backend::{app::App, models::users, services::users::Role};
use loco_rs::{app::AppContext, testing::prelude::*, TestServer};
use sea_orm::{ActiveModelTrait, IntoActiveModel, Set};
use serde_json::{json, Value};
use serial_test::serial;

use super::{agent_harness::tunnel_agent, prepare_data};

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

fn connection(name: &str, port: i32, password: &str, storage: Option<i64>) -> Value {
    json!({
        "name": name,
        "engine": "postgres",
        "host": "127.0.0.1",
        "port": port,
        "username": "postgres",
        "password": password,
        "sslMode": "disable",
        "databases": [],
        "storageDestinationId": storage,
        "backupEnabled": storage.is_some(),
        "backupIntervalHours": 24,
        "backupRetention": 2
    })
}

#[tokio::test]
#[serial]
async fn a_senha_entra_e_nao_sai_e_editar_sem_ela_mantem() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Admin).await;

        let created = request
            .post("/api/databases")
            .json(&connection("ERP", 5432, "senha-secreta", None))
            .await;
        assert_eq!(created.status_code(), 201, "{}", created.text());
        assert!(!created.text().contains("senha-secreta"));
        let created = body(&created.text());
        assert_eq!(created["passwordSet"], json!(true));
        let id = created["id"].as_i64().unwrap();

        let mut edit = connection("ERP produção", 5432, "", None);
        edit["password"] = Value::Null;
        let updated = request
            .put(&format!("/api/databases/{id}"))
            .json(&edit)
            .await;
        assert_eq!(updated.status_code(), 200, "{}", updated.text());
        assert_eq!(body(&updated.text())["name"], json!("ERP produção"));

        let row = backend::services::databases::service::load(&ctx.db, id)
            .await
            .unwrap();
        assert_eq!(
            row.target().password,
            "senha-secreta",
            "a senha gravada se perdeu"
        );

        let list = request.get("/api/databases").await;
        assert!(!list.text().contains("senha-secreta"));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn backup_automatico_sem_armazenamento_e_recusado() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Admin).await;
        let mut input = connection("Sem destino", 5432, "x", None);
        input["backupEnabled"] = json!(true);
        let response = request.post("/api/databases").json(&input).await;
        assert_eq!(response.status_code(), 422, "{}", response.text());
        assert!(
            response.text().contains("armazenamento"),
            "{}",
            response.text()
        );

        let manual = request
            .post("/api/databases")
            .json(&connection("Manual", 5432, "x", None))
            .await;
        let id = body(&manual.text())["id"].as_i64().unwrap();
        let run = request.post(&format!("/api/databases/{id}/backups")).await;
        assert_eq!(run.status_code(), 422, "{}", run.text());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn servidor_que_nao_responde_volta_com_a_razao() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Admin).await;
        let response = request
            .post("/api/databases/probe")
            .json(&json!({
                "engine": "mysql", "host": "127.0.0.1", "port": 1,
                "username": "root", "password": "x", "sslMode": "disable"
            }))
            .await;
        assert_eq!(response.status_code(), 400, "{}", response.text());
        assert!(response.text().contains("conectar"), "{}", response.text());
    })
    .await;
}

/// "Acessar a partir de": a conexão guarda o agente, a lista de agentes diz
/// quem serve, e o teste pela ponte devolve a recusa do agente com a razão.
#[tokio::test]
#[serial]
async fn conexao_pela_ponte_do_agente() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Admin).await;
        let probe = tunnel_agent(&ctx, "filial-sp", "10.0.0.20:5432").await;

        let agents = body(&request.get("/api/databases/agents").await.text());
        let agent = agents
            .as_array()
            .unwrap()
            .iter()
            .find(|agent| agent["id"] == json!(probe.id))
            .expect("agente na lista");
        assert_eq!(agent["connected"], json!(true));
        assert_eq!(agent["allowed"], json!(true));
        assert!(
            agent.get("supported").is_none(),
            "sem modo \"agente antigo\" no v2"
        );

        let mut input = connection("ERP da filial", 5432, "x", None);
        input["host"] = json!("10.0.0.20");
        input["viaProbeId"] = json!(probe.id);
        let created = request.post("/api/databases").json(&input).await;
        assert_eq!(created.status_code(), 201, "{}", created.text());
        let created = body(&created.text());
        assert_eq!(created["viaProbeId"], json!(probe.id));
        assert_eq!(created["viaProbeName"], json!("filial-sp"));

        // Outro host da filial, fora da lista local do agente.
        let refused = request
            .post("/api/databases/probe")
            .json(&json!({
                "engine": "postgres", "host": "10.0.0.99", "port": 5432,
                "username": "postgres", "password": "x", "sslMode": "disable",
                "viaProbeId": probe.id
            }))
            .await;
        assert_eq!(refused.status_code(), 400, "{}", refused.text());
        assert!(
            refused.text().contains("AGENT_DATABASE_TARGETS"),
            "{}",
            refused.text()
        );

        let mut ghost = connection("Agente que não existe", 5432, "x", None);
        ghost["viaProbeId"] = json!(999_999);
        let ghost = request.post("/api/databases").json(&ghost).await;
        assert_eq!(ghost.status_code(), 422, "{}", ghost.text());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn operador_nao_ve_as_conexoes() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Operator).await;
        let response = request.get("/api/databases").await;
        assert_eq!(response.status_code(), 403, "{}", response.text());
    })
    .await;
}

/// Ponta a ponta com um PostgreSQL local (`NETMONITOR_TEST_POSTGRES`).
#[tokio::test]
#[serial]
async fn backup_e_restauracao_pela_api() {
    let Ok(raw) = std::env::var("NETMONITOR_TEST_POSTGRES") else {
        return;
    };
    let parts: Vec<&str> = raw.splitn(4, ':').collect();
    let (port, password) = (parts[1].parse::<i32>().unwrap(), parts[3].to_string());
    let root = tempfile::tempdir().unwrap();
    std::env::set_var("STORAGE_LOCAL_ROOT", root.path());

    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Admin).await;
        let storage = request
            .post("/api/storages")
            .json(&json!({
                "name": "Disco", "provider": "local",
                "config": { "type": "local", "basePath": "copias" },
                "backupEnabled": false, "backupIntervalHours": 24, "backupRetention": 7
            }))
            .await;
        let storage_id = body(&storage.text())["id"].as_i64().unwrap();

        // Um banco só para este teste, com uma tabela.
        let source = format!("nm_api_{}", &uuid::Uuid::new_v4().simple().to_string()[..8]);
        let url = |db: &str| format!("postgres://postgres:{password}@127.0.0.1:{port}/{db}");
        let mut admin = <sqlx::PgConnection as sqlx::Connection>::connect(&url("postgres"))
            .await
            .unwrap();
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE DATABASE {source}")))
            .execute(&mut admin)
            .await
            .unwrap();
        let mut conn = <sqlx::PgConnection as sqlx::Connection>::connect(&url(&source))
            .await
            .unwrap();
        sqlx::raw_sql("CREATE TABLE t (id serial PRIMARY KEY, nome text); INSERT INTO t (nome) SELECT 'n' || g FROM generate_series(1, 100) g")
            .execute(&mut conn)
            .await
            .unwrap();

        let mut input = connection("Local", port, &password, Some(storage_id));
        input["databases"] = json!([source]);
        let created = request.post("/api/databases").json(&input).await;
        assert_eq!(created.status_code(), 201, "{}", created.text());
        let id = body(&created.text())["id"].as_i64().unwrap();

        let started = request.post(&format!("/api/databases/{id}/backups")).await;
        assert_eq!(started.status_code(), 202, "{}", started.text());
        assert_eq!(body(&started.text())["kind"], json!("backup"));

        // O backup roda em segundo plano: espera a linha do histórico.
        let mut history = Value::Null;
        for _ in 0..100 {
            let list = request.get(&format!("/api/databases/{id}/backups")).await;
            history = body(&list.text());
            if history[0]["status"] != json!("running") && history[0]["status"].is_string() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        assert_eq!(history[0]["status"], json!("success"), "{history}");
        assert_eq!(history[0]["rows"], json!(100));
        let backup_id = history[0]["id"].as_i64().unwrap();
        let key = history[0]["objectKey"].as_str().unwrap();
        assert!(root.path().join("copias").join(key).is_file());

        // Substituir sem confirmar o nome é recusado.
        let unconfirmed = request
            .post(&format!("/api/databases/backups/{backup_id}/restore"))
            .json(&json!({ "targetConnectionId": id, "database": source, "mode": "replace" }))
            .await;
        assert_eq!(unconfirmed.status_code(), 422, "{}", unconfirmed.text());

        let copy = format!("{source}_copia");
        let restored = request
            .post(&format!("/api/databases/backups/{backup_id}/restore"))
            .json(&json!({ "targetConnectionId": id, "database": copy, "mode": "new_database" }))
            .await;
        assert_eq!(restored.status_code(), 202, "{}", restored.text());

        let mut count = 0_i64;
        for _ in 0..100 {
            if let Ok(mut restored) =
                <sqlx::PgConnection as sqlx::Connection>::connect(&url(&copy)).await
            {
                if let Ok(n) = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM t")
                    .fetch_one(&mut restored)
                    .await
                {
                    count = n;
                    break;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        assert_eq!(count, 100);

        let _ = sqlx::Connection::close(conn).await;
        for database in [&source, &copy] {
            sqlx::raw_sql(sqlx::AssertSqlSafe(format!("DROP DATABASE IF EXISTS {database} WITH (FORCE)")))
                .execute(&mut admin)
                .await
                .unwrap();
        }
    })
    .await;
    std::env::remove_var("STORAGE_LOCAL_ROOT");
}
