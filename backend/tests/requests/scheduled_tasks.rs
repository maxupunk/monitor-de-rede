//! Tarefas agendadas: catálogo, conferência, cadastro, "rodar agora" e
//! histórico pela API; o laço que roda o vencido, pula o horário perdido e
//! falha quando o dono perdeu a permissão; e o mesmo cadastro pelo chat,
//! só com confirmação.

use std::time::Duration;

use axum::http::StatusCode;
use backend::{
    app::App,
    models::{_entities::audit_logs, scheduled_task_runs, scheduled_tasks, users},
    services::{
        ai::{
            harness::tools::{AiToolRisk, ToolArgs, ToolPolicy, ToolRegistry, Verdict},
            settings::AiAutomationMode,
        },
        audit::AuditActor,
        scheduled_tasks::{
            model::{ScheduledTaskInput, ScheduledTaskOrigin},
            runner, service, Author,
        },
        users::Role,
    },
};
use chrono::Utc;
use loco_rs::{app::AppContext, testing::prelude::*};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, PaginatorTrait, QueryFilter, Set,
};
use serde_json::{json, Value};
use serial_test::serial;

use super::prepare_data;

const COM_ACOES: ToolPolicy = ToolPolicy {
    allow_active: false,
    actions: AiAutomationMode::Confirm,
    confirm_active: false,
    interactive: true,
    container_actions: AiAutomationMode::Off,
};

fn json_of(text: &str) -> Value {
    serde_json::from_str(text).expect("resposta JSON")
}

async fn with_role(ctx: &AppContext, user: users::Model, role: Role) -> users::Model {
    let mut active = user.into_active_model();
    active.role = Set(role.as_str().to_string());
    active.update(&ctx.db).await.unwrap()
}

async fn session_with_role(
    request: &mut loco_rs::TestServer,
    ctx: &AppContext,
    role: Role,
) -> users::Model {
    let session = prepare_data::init_operator(ctx).await;
    let user = with_role(ctx, session.user, role).await;
    let (header, value) = prepare_data::auth_header(&session.token);
    request.add_header(header, value);
    user
}

async fn author(ctx: &AppContext, user: &users::Model) -> Author {
    Author::load(
        &ctx.db,
        AuditActor {
            user_id: Some(user.id),
            ..AuditActor::default()
        },
        ScheduledTaskOrigin::User,
    )
    .await
    .unwrap()
}

fn resumo(cron: &str) -> ScheduledTaskInput {
    ScheduledTaskInput {
        name: "Resumo do sistema".into(),
        description: None,
        tool: "get_system_summary".into(),
        arguments: json!({}),
        cron: cron.into(),
        enabled: true,
    }
}

/// Espera a execução à parte terminar (o "rodar agora" responde antes).
async fn finished_run(ctx: &AppContext, run_id: i64) -> scheduled_task_runs::Model {
    for _ in 0..100 {
        let run = scheduled_task_runs::Entity::find_by_id(run_id)
            .one(&ctx.db)
            .await
            .unwrap()
            .expect("execução gravada");
        if run.finished_at.is_some() {
            return run;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("a execução {run_id} não terminou");
}

async fn set_next_run(ctx: &AppContext, id: i64, minutes_ago: i64) {
    let task = scheduled_tasks::Entity::find_by_id(id)
        .one(&ctx.db)
        .await
        .unwrap()
        .unwrap();
    let mut active = task.into_active_model();
    active.next_run_at = Set(Some(
        (Utc::now() - chrono::Duration::minutes(minutes_ago)).into(),
    ));
    active.update(&ctx.db).await.unwrap();
}

#[tokio::test]
#[serial]
async fn cadastro_conferencia_rodar_agora_e_historico() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Operator).await;

        let catalogo = json_of(&request.get("/api/scheduled-tasks/catalog").await.text());
        let nomes: Vec<&str> = catalogo
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect();
        assert!(nomes.contains(&"ping_host"));
        assert!(nomes.contains(&"silence_alert"));
        assert!(!nomes.contains(&"ask_user"));
        assert!(!nomes.contains(&"device_ssh_exec"));
        let container = catalogo
            .as_array()
            .unwrap()
            .iter()
            .find(|tool| tool["name"] == "docker_container_action")
            .unwrap();
        assert_eq!(container["adminOnly"], true);
        assert_eq!(container["kind"], "container");

        let conferida = request
            .post("/api/scheduled-tasks/preview")
            .json(&json!({ "tool": "get_system_summary", "arguments": {}, "cron": "0 3 * * *" }))
            .await;
        conferida.assert_status_ok();
        let conferida = json_of(&conferida.text());
        assert_eq!(conferida["valid"], true, "{conferida}");
        assert_eq!(conferida["scheduleLabel"], "todo dia às 03:00");
        assert_eq!(conferida["nextRuns"].as_array().unwrap().len(), 3);
        assert_eq!(conferida["risk"], "low");

        let errada = json_of(
            &request
                .post("/api/scheduled-tasks/preview")
                .json(&json!({ "tool": "device_ssh_exec", "arguments": {}, "cron": "0 3 * *" }))
                .await
                .text(),
        );
        assert_eq!(errada["valid"], false);
        let erros = errada["errors"].as_array().unwrap();
        assert!(erros.iter().any(|e| e.as_str().unwrap().contains("cron")), "{errada}");
        assert!(
            erros.iter().any(|e| e.as_str().unwrap().contains("não pode ser agendada")),
            "{errada}"
        );

        let criada = request
            .post("/api/scheduled-tasks")
            .json(&json!({
                "name": "Resumo de manhã",
                "tool": "get_system_summary",
                "arguments": { },
                "cron": "0 7 * * 1-5"
            }))
            .await;
        criada.assert_status(StatusCode::CREATED);
        let criada = json_of(&criada.text());
        let id = criada["id"].as_i64().unwrap();
        assert_eq!(criada["scheduleLabel"], "de segunda a sexta às 07:00");
        assert_eq!(criada["origin"], "user");
        assert_eq!(criada["ownerName"], "operator");
        assert!(criada["nextRunAt"].is_string());

        request
            .post("/api/scheduled-tasks")
            .json(&json!({ "name": "Ruim", "tool": "ping_host", "arguments": { "inventado": 1 }, "cron": "toda hora" }))
            .await
            .assert_status(StatusCode::UNPROCESSABLE_ENTITY);

        let pausada = json_of(
            &request
                .put(&format!("/api/scheduled-tasks/{id}/enabled"))
                .json(&json!({ "enabled": false }))
                .await
                .text(),
        );
        assert_eq!(pausada["enabled"], false);
        assert!(pausada["nextRunAt"].is_null(), "pausada não tem próxima");

        let editada = json_of(
            &request
                .put(&format!("/api/scheduled-tasks/{id}"))
                .json(&json!({
                    "name": "Resumo de meia em meia hora",
                    "tool": "get_system_summary",
                    "cron": "*/30 * * * *",
                    "enabled": true
                }))
                .await
                .text(),
        );
        assert_eq!(editada["scheduleLabel"], "a cada 30 minutos");
        assert!(editada["nextRunAt"].is_string());

        let rodando = request
            .post(&format!("/api/scheduled-tasks/{id}/run"))
            .await;
        rodando.assert_status(StatusCode::ACCEPTED);
        let run_id = json_of(&rodando.text())["runId"].as_i64().unwrap();
        let run = finished_run(&ctx, run_id).await;
        assert_eq!(run.status, "success", "{:?}", run.message);
        assert_eq!(run.trigger, "manual");

        let historico = json_of(
            &request
                .get(&format!("/api/scheduled-tasks/{id}/runs"))
                .await
                .text(),
        );
        assert_eq!(historico["meta"]["total"], 1);
        assert_eq!(historico["data"][0]["status"], "success");
        assert!(historico["data"][0]["result"].is_object());

        let lista = json_of(&request.get("/api/scheduled-tasks").await.text());
        assert_eq!(lista[0]["lastStatus"], "success");

        request
            .delete(&format!("/api/scheduled-tasks/{id}"))
            .await
            .assert_status(StatusCode::NO_CONTENT);
        request
            .get(&format!("/api/scheduled-tasks/{id}"))
            .await
            .assert_status(StatusCode::NOT_FOUND);
        assert_eq!(
            scheduled_task_runs::Entity::find().count(&ctx.db).await.unwrap(),
            0,
            "o histórico vai junto"
        );

        let auditadas = audit_logs::Entity::find()
            .filter(audit_logs::Column::ResourceType.eq("scheduled_task"))
            .count(&ctx.db)
            .await
            .unwrap();
        assert!(auditadas >= 4, "criar, pausar, editar e excluir: {auditadas}");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn visualizador_le_mas_nao_agenda_e_operador_nao_mexe_em_container() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let usuario = session_with_role(&mut request, &ctx, Role::Viewer).await;
        request.get("/api/scheduled-tasks").await.assert_status_ok();
        request
            .get("/api/scheduled-tasks/catalog")
            .await
            .assert_status_ok();
        request
            .post("/api/scheduled-tasks")
            .json(&json!({ "name": "x", "tool": "get_system_summary", "cron": "0 3 * * *" }))
            .await
            .assert_status(StatusCode::FORBIDDEN);

        with_role(&ctx, usuario, Role::Operator).await;
        let recusa = request
            .post("/api/scheduled-tasks")
            .json(&json!({
                "name": "Reiniciar web",
                "tool": "docker_container_action",
                "arguments": { "host": "agent-1", "container": "web", "action": "restart" },
                "cron": "0 4 * * *"
            }))
            .await;
        recusa.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
        assert!(recusa.text().contains("administrador"), "{}", recusa.text());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn agendador_roda_o_vencido_pula_o_perdido_e_falha_sem_permissao() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let dono = prepare_data::init_operator(&ctx).await.user;
        let outro = prepare_data::create_user(&ctx, "outro", "outro@loco.com", "Outro-1234").await;
        let autor = author(&ctx, &dono).await;
        let autor_outro = author(&ctx, &outro).await;

        let vencida = service::create(&ctx, &resumo("*/5 * * * *"), &autor)
            .await
            .unwrap();
        let perdida = service::create(&ctx, &resumo("0 3 * * *"), &autor)
            .await
            .unwrap();
        let sem_dono = service::create(&ctx, &resumo("*/10 * * * *"), &autor_outro)
            .await
            .unwrap();
        let pausada = service::create(
            &ctx,
            &ScheduledTaskInput {
                enabled: false,
                ..resumo("* * * * *")
            },
            &autor,
        )
        .await
        .unwrap();
        set_next_run(&ctx, vencida.id, 1).await;
        set_next_run(&ctx, perdida.id, 6 * 60).await;
        set_next_run(&ctx, sem_dono.id, 1).await;
        with_role(&ctx, outro, Role::Viewer).await;

        let agora = Utc::now();
        let execucoes = runner::run_due(&ctx, agora).await.unwrap();
        assert_eq!(execucoes.len(), 2, "a vencida e a sem permissão começam");
        for execucao in execucoes {
            execucao.await.unwrap().unwrap();
        }

        let estado = |id: i64| {
            let db = ctx.db.clone();
            async move {
                scheduled_tasks::Entity::find_by_id(id)
                    .one(&db)
                    .await
                    .unwrap()
                    .unwrap()
            }
        };
        let rodou = estado(vencida.id).await;
        assert_eq!(rodou.last_status.as_deref(), Some("success"));
        assert!(rodou.next_run_at.unwrap() > agora, "a próxima foi gravada");

        let pulou = estado(perdida.id).await;
        assert_eq!(pulou.last_status.as_deref(), Some("skipped"));
        assert!(
            pulou
                .last_message
                .as_deref()
                .unwrap()
                .contains("fora do ar"),
            "{:?}",
            pulou.last_message
        );
        assert!(pulou.next_run_at.unwrap() > agora);

        let falhou = estado(sem_dono.id).await;
        assert_eq!(falhou.last_status.as_deref(), Some("failed"));
        assert!(
            falhou
                .last_message
                .as_deref()
                .unwrap()
                .contains("não pode mais rodar"),
            "{:?}",
            falhou.last_message
        );

        assert!(estado(pausada.id).await.next_run_at.is_none());
        assert!(
            runner::run_due(&ctx, Utc::now()).await.unwrap().is_empty(),
            "nada vence duas vezes"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn ia_agenda_altera_roda_e_exclui_so_com_confirmacao() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let dono = prepare_data::init_operator(&ctx).await.user;
        let ator = AuditActor {
            user_id: Some(dono.id),
            ..AuditActor::default()
        };
        let registro = ToolRegistry::new(COM_ACOES);
        let pedido = json!({
            "name": "Resumo diário",
            "tool": "get_system_summary",
            "cron": "0 7 * * *"
        })
        .to_string();

        assert!(registro.needs_confirmation("create_scheduled_task"));
        assert!(
            registro
                .execute(&ctx, "create_scheduled_task", &pedido)
                .await
                .is_err(),
            "o chat não agenda sozinho"
        );
        let resumo = registro
            .preview(&ctx, "create_scheduled_task", &pedido)
            .await
            .unwrap();
        assert!(resumo.starts_with("Agendar \"Resumo diário\""), "{resumo}");
        assert!(resumo.contains("todo dia às 07:00"), "{resumo}");

        let invalido = registro
            .preview(
                &ctx,
                "create_scheduled_task",
                &json!({ "name": "x", "tool": "ask_user", "cron": "0 7 * * *" }).to_string(),
            )
            .await
            .unwrap();
        assert!(invalido.starts_with("Ação inválida"), "{invalido}");

        let acao = ToolArgs::from_value(json!({
            "name": "Silenciar à noite",
            "tool": "silence_alert",
            "arguments": { "alert_id": 1, "minutes": 60 },
            "cron": "0 22 * * *"
        }));
        let avaliada = registro
            .assess(&ctx, "create_scheduled_task", &acao)
            .await
            .unwrap();
        assert_eq!(
            avaliada.risk.level,
            AiToolRisk::High,
            "ação sozinha sempre pergunta"
        );
        assert!(matches!(avaliada.verdict, Verdict::Ask));

        let sem_usuario = registro
            .execute_confirmed(&ctx, "create_scheduled_task", ToolArgs::parse(&pedido))
            .await
            .unwrap();
        assert!(
            sem_usuario.data["error"].is_string(),
            "{}",
            sem_usuario.data
        );

        let criada = registro
            .execute_confirmed(
                &ctx,
                "create_scheduled_task",
                ToolArgs::parse(&pedido).with_actor(ator.clone()),
            )
            .await
            .unwrap();
        let id = criada.data["task_id"].as_i64().expect("tarefa criada");
        let gravada = scheduled_tasks::Entity::find_by_id(id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(gravada.origin, "ai");
        assert_eq!(gravada.user_id, Some(dono.id));

        let lista = registro
            .execute(&ctx, "list_scheduled_tasks", "{}")
            .await
            .unwrap();
        assert_eq!(lista.data["total"], 1);
        assert_eq!(lista.data["tasks"][0]["schedule"], "todo dia às 07:00");

        let pausa = json!({ "task_id": id, "enabled": false }).to_string();
        let avaliada = registro
            .assess(&ctx, "update_scheduled_task", &ToolArgs::parse(&pausa))
            .await
            .unwrap();
        assert_eq!(avaliada.risk.level, AiToolRisk::Low);
        registro
            .execute_confirmed(
                &ctx,
                "update_scheduled_task",
                ToolArgs::parse(&pausa).with_actor(ator.clone()),
            )
            .await
            .unwrap();
        assert!(!service::find(&ctx.db, id).await.unwrap().enabled);

        let rodou = registro
            .execute_confirmed(
                &ctx,
                "run_scheduled_task",
                ToolArgs::from_value(json!({ "task_id": id })).with_actor(ator.clone()),
            )
            .await
            .unwrap();
        assert_eq!(rodou.data["status"], "success", "{}", rodou.data);

        let exclusao = json!({ "task_id": id }).to_string();
        let aviso = registro
            .preview(&ctx, "delete_scheduled_task", &exclusao)
            .await
            .unwrap();
        assert!(
            aviso.contains("A execução registrada também será apagada"),
            "{aviso}"
        );
        registro
            .execute_confirmed(
                &ctx,
                "delete_scheduled_task",
                ToolArgs::parse(&exclusao).with_actor(ator),
            )
            .await
            .unwrap();
        assert!(service::find(&ctx.db, id).await.is_err());
    })
    .await;
}
