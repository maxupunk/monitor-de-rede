//! Deploy contínuo pela API (ADR 021): o assistente grava tudo ou nada, o
//! segredo entra e não sai, o papel por área decide quem vê e quem muda, o
//! webhook pede aprovação para produção sem repetir a mesma versão, e a
//! aprovação, a recusa e o cancelamento mudam o estado uma vez só.

use backend::app::App;
use loco_rs::{app::AppContext, testing::prelude::*, TestServer};
use netmonitor_core::models::users;
use netmonitor_servers::{
    deploys::{oidc::GithubClaims, webhook, RunStatus},
    dtos::deploys::HookInput,
};
use sea_orm::{ActiveModelTrait, IntoActiveModel, Set};
use serde_json::{json, Value};
use serial_test::serial;

use super::{agent_harness::enrolled_agent, prepare_data};

async fn session(
    request: &mut TestServer,
    ctx: &AppContext,
    role: &str,
    area_roles: Value,
) -> users::Model {
    let session = prepare_data::init_operator(ctx).await;
    let mut active = session.user.into_active_model();
    active.role = Set(role.to_owned());
    active.area_roles = Set(Some(area_roles));
    let user = active.update(&ctx.db).await.expect("papel");
    let (header, value) = prepare_data::auth_header(&session.token);
    request.add_header(header, value);
    user
}

fn body(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or(Value::Null)
}

fn setup(probe_id: i64, project: &str, requires_approval: bool) -> Value {
    json!({
        "application": {
            "name": "SGCv4",
            "slug": "sgc",
            "repository": "master-sistemas/sgcv4",
            "credentialId": null,
            "composeFiles": ["docker-compose.yml"],
            "imageTagVariable": "IMAGE_TAG",
            "images": [{ "name": "sgc-api", "dockerfile": "apps/api/Dockerfile", "context": "." }],
            "rollout": "sequential"
        },
        "target": {
            "name": "Produção",
            "probeId": probe_id,
            "project": project,
            "reference": "main",
            "requiresApproval": requires_approval,
            "waitTimeoutSecs": 180,
            "autoRollback": true,
            "keepImages": 3,
            "enabled": true
        },
        "env": "APP_KEY=segredo-da-aplicacao\nDB_HOST=postgres\n"
    })
}

fn claims(repository: &str, git_ref: &str, sha: &str) -> GithubClaims {
    serde_json::from_value(json!({
        "repository": repository,
        "ref": git_ref,
        "sha": sha,
        "actor": "maxuel",
        "exp": chrono::Utc::now().timestamp() + 300,
    }))
    .expect("claims")
}

#[tokio::test]
#[serial]
async fn assistente_grava_tudo_ou_nada_e_o_segredo_nao_sai() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session(&mut request, &ctx, "admin", json!({})).await;
        let (agent, _) = enrolled_agent(&ctx, "debian-prod").await;

        let created = request
            .post("/api/deploys/setup")
            .json(&setup(agent.id, "sgc", true))
            .await;
        assert_eq!(created.status_code(), 201, "{}", created.text());

        let overview = request.get("/api/deploys").await;
        assert_eq!(overview.status_code(), 200, "{}", overview.text());
        assert!(
            !overview.text().contains("segredo-da-aplicacao"),
            "valor do .env na visão geral"
        );
        let overview = body(&overview.text());
        let target = &overview["applications"][0]["targets"][0];
        assert_eq!(target["project"], json!("sgc"));
        assert_eq!(target["env"]["keys"], json!(["APP_KEY", "DB_HOST"]));
        assert_eq!(overview["hookPath"], json!("/api/deploy-hooks/github"));

        // O mesmo projeto no mesmo agente brigaria pelos containers: recusado,
        // e a aplicação nova do segundo assistente não fica pela metade.
        let mut again = setup(agent.id, "sgc", false);
        again["application"]["slug"] = json!("sgc-outro");
        let refused = request.post("/api/deploys/setup").json(&again).await;
        assert_eq!(refused.status_code(), 409, "{}", refused.text());
        let overview = body(&request.get("/api/deploys").await.text());
        assert_eq!(overview["applications"].as_array().map(Vec::len), Some(1));

        // Credencial: o token entra e não volta.
        let credential = request
            .post("/api/deploys/credentials")
            .json(&json!({ "name": "GitHub", "username": "bot", "secret": "ghp_token_secreto" }))
            .await;
        assert_eq!(credential.status_code(), 201, "{}", credential.text());
        assert!(!credential.text().contains("ghp_token_secreto"));
        assert!(!request
            .get("/api/deploys")
            .await
            .text()
            .contains("ghp_token_secreto"));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn leitor_da_area_ve_mas_nao_muda_nem_abre_o_env() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let (agent, _) = enrolled_agent(&ctx, "debian-prod").await;
        let (application, target) = netmonitor_servers::deploys::applications::setup(
            &ctx.db,
            &serde_json::from_value(setup(agent.id, "sgc", true)).expect("setup"),
            None,
        )
        .await
        .expect("assistente");
        assert_eq!(application.slug, "sgc");

        session(
            &mut request,
            &ctx,
            "operator",
            json!({ "servers": "operator" }),
        )
        .await;
        assert_eq!(request.get("/api/deploys").await.status_code(), 200);
        let env = request
            .get(&format!("/api/deploys/targets/{}/env", target.id))
            .await;
        assert_eq!(
            env.status_code(),
            403,
            "operador abriu o .env: {}",
            env.text()
        );
        let deploy = request
            .post(&format!("/api/deploys/targets/{}/deploy", target.id))
            .json(&json!({ "when": "now" }))
            .await;
        assert_eq!(deploy.status_code(), 403, "{}", deploy.text());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn webhook_sem_token_do_github_e_recusado() {
    request_with_config::<App, _, _>(RequestConfig::default(), |request, _ctx| async move {
        let without = request
            .post("/api/deploy-hooks/github")
            .json(&json!({}))
            .await;
        assert_eq!(without.status_code(), 401, "{}", without.text());
        let forged = request
            .post("/api/deploy-hooks/github")
            .add_header("Authorization", "Bearer nao.e.um.jwt")
            .json(&json!({}))
            .await;
        assert_eq!(forged.status_code(), 401, "{}", forged.text());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn push_na_main_pede_aprovacao_uma_vez_e_a_aprovacao_entra_na_fila() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session(&mut request, &ctx, "admin", json!({})).await;
        let (agent, _) = enrolled_agent(&ctx, "debian-prod").await;
        let created = request
            .post("/api/deploys/setup")
            .json(&setup(agent.id, "sgc", true))
            .await;
        assert_eq!(created.status_code(), 201, "{}", created.text());

        let sha = "a".repeat(40);
        let push = claims("Master-Sistemas/SGCv4", "refs/heads/main", &sha);
        let input = HookInput {
            commit_message: Some("feat: NF-e".into()),
            image_tag: None,
        };
        let first = webhook::handle(&ctx, &push, &input).await.expect("webhook");
        assert_eq!(first.runs.len(), 1, "{}", first.message);
        assert_eq!(first.runs[0].status, RunStatus::AwaitingApproval);
        let run_id = first.runs[0].id;

        // Reexecutar o workflow da mesma versão não cria outro deploy.
        let again = webhook::handle(&ctx, &push, &input).await.expect("webhook");
        assert!(again.runs.is_empty(), "{}", again.message);
        // Branch que nenhum alvo acompanha: nada a fazer.
        let other = claims(
            "master-sistemas/sgcv4",
            "refs/heads/develop",
            &"b".repeat(40),
        );
        assert!(webhook::handle(&ctx, &other, &input)
            .await
            .expect("webhook")
            .runs
            .is_empty());
        // Repositório que ninguém cadastrou.
        let unknown = claims("outro/repo", "refs/heads/main", &sha);
        assert!(webhook::handle(&ctx, &unknown, &input).await.is_err());

        let changes = request.get(&format!("/api/deploys/runs/{run_id}")).await;
        assert_eq!(body(&changes.text())["commitMessage"], json!("feat: NF-e"));

        let approved = request
            .post(&format!("/api/deploys/runs/{run_id}/approve"))
            .json(&json!({ "when": "now" }))
            .await;
        assert_eq!(approved.status_code(), 200, "{}", approved.text());
        assert_eq!(body(&approved.text())["status"], json!("queued"));

        // Aprovar de novo não aprova duas vezes.
        let twice = request
            .post(&format!("/api/deploys/runs/{run_id}/approve"))
            .json(&json!({ "when": "now" }))
            .await;
        assert_eq!(twice.status_code(), 409, "{}", twice.text());

        // Na fila ainda dá para cancelar.
        let cancelled = request
            .post(&format!("/api/deploys/runs/{run_id}/cancel"))
            .json(&json!({}))
            .await;
        assert_eq!(cancelled.status_code(), 200, "{}", cancelled.text());
        assert_eq!(body(&cancelled.text())["status"], json!("cancelled"));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn versao_nova_substitui_a_pendente_e_a_recusa_fica_no_historico() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session(&mut request, &ctx, "admin", json!({})).await;
        let (agent, _) = enrolled_agent(&ctx, "debian-prod").await;
        let created = request
            .post("/api/deploys/setup")
            .json(&setup(agent.id, "sgc", true))
            .await;
        let target_id = body(&created.text())["targetId"].as_i64().expect("alvo");
        let input = HookInput::default();

        let old = webhook::handle(
            &ctx,
            &claims("master-sistemas/sgcv4", "refs/heads/main", &"a".repeat(40)),
            &input,
        )
        .await
        .expect("primeiro push");
        let new = webhook::handle(
            &ctx,
            &claims("master-sistemas/sgcv4", "refs/heads/main", &"b".repeat(40)),
            &input,
        )
        .await
        .expect("segundo push");

        let history = body(
            &request
                .get(&format!("/api/deploys/targets/{target_id}/runs"))
                .await
                .text(),
        );
        let status_of = |id: i64| {
            history
                .as_array()
                .and_then(|runs| runs.iter().find(|run| run["id"] == json!(id)))
                .map(|run| run["status"].clone())
        };
        assert_eq!(status_of(old.runs[0].id), Some(json!("superseded")));
        assert_eq!(status_of(new.runs[0].id), Some(json!("awaiting_approval")));

        let rejected = request
            .post(&format!("/api/deploys/runs/{}/reject", new.runs[0].id))
            .json(&json!({ "reason": "falta testar a NF-e" }))
            .await;
        assert_eq!(rejected.status_code(), 200, "{}", rejected.text());
        let rejected = body(&rejected.text());
        assert_eq!(rejected["status"], json!("rejected"));
        assert!(rejected["error"]
            .as_str()
            .is_some_and(|reason| reason.contains("falta testar a NF-e")));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn env_guarda_versoes_e_mostra_so_os_nomes_que_mudam() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session(&mut request, &ctx, "admin", json!({})).await;
        let (agent, _) = enrolled_agent(&ctx, "debian-prod").await;
        let created = request
            .post("/api/deploys/setup")
            .json(&setup(agent.id, "sgc", false))
            .await;
        let target_id = body(&created.text())["targetId"].as_i64().expect("alvo");

        let next = "APP_KEY=outro-segredo\nNOVA=1\n";
        let preview = request
            .post(&format!("/api/deploys/targets/{target_id}/env/preview"))
            .json(&json!({ "content": next }))
            .await;
        assert_eq!(preview.status_code(), 200, "{}", preview.text());
        assert!(!preview.text().contains("outro-segredo"));
        let preview = body(&preview.text());
        assert_eq!(preview["added"], json!(["NOVA"]));
        assert_eq!(preview["changed"], json!(["APP_KEY"]));
        assert_eq!(preview["removed"], json!(["DB_HOST"]));

        let invalid = request
            .post(&format!("/api/deploys/targets/{target_id}/env/preview"))
            .json(&json!({ "content": "isto não é env" }))
            .await;
        assert_eq!(invalid.status_code(), 422, "{}", invalid.text());

        let saved = request
            .put(&format!("/api/deploys/targets/{target_id}/env"))
            .json(&json!({ "content": next, "redeploy": false }))
            .await;
        assert_eq!(saved.status_code(), 200, "{}", saved.text());

        let versions = body(
            &request
                .get(&format!("/api/deploys/targets/{target_id}/env/versions"))
                .await
                .text(),
        );
        assert_eq!(versions.as_array().map(Vec::len), Some(2));
        assert_eq!(versions[0]["current"], json!(true));
        let current = body(
            &request
                .get(&format!("/api/deploys/targets/{target_id}/env"))
                .await
                .text(),
        );
        assert_eq!(current["content"], json!(next));

        // Republicar sem versão no ar não tem o que republicar.
        let deploy = request
            .post(&format!("/api/deploys/targets/{target_id}/deploy"))
            .json(&json!({ "when": "now" }))
            .await;
        assert_eq!(deploy.status_code(), 422, "{}", deploy.text());
    })
    .await;
}
