//! Requisições do motor de alertas e do protocolo de probes (Fases 6 e 7).

use backend::{
    app::App,
    models::{_entities::probe_tasks, monitors, probes},
    services::{
        probes::{dispatcher, DEFAULT_VPN_PROBE_TOKEN},
        shared::crypto::sha256_hex,
    },
};
use chrono::{Duration, Utc};
use loco_rs::testing::prelude::*;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serial_test::serial;

use super::prepare_data;

fn json_of(text: &str) -> serde_json::Value {
    serde_json::from_str(text).expect("resposta JSON")
}

/// Cria um probe com token conhecido e devolve `(id, token cru)`.
async fn criar_probe(
    ctx: &loco_rs::app::AppContext,
    name: &str,
    token: &str,
    last_seen_at: Option<chrono::DateTime<Utc>>,
) -> i64 {
    probes::ActiveModel {
        name: Set(name.into()),
        token_hash: Set(sha256_hex(token)),
        status: Set("pending".into()),
        last_seen_at: Set(last_seen_at.map(Into::into)),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .expect("probe criado")
    .id
}

/// `probe_tasks` tem FK para `monitors`: a fila só aceita monitor existente.
async fn criar_monitor(ctx: &loco_rs::app::AppContext, name: &str, probe_id: Option<i64>) -> i64 {
    monitors::ActiveModel {
        probe_id: Set(probe_id),
        r#type: Set("ping".into()),
        name: Set(name.into()),
        configuration: Set(serde_json::json!({ "host": "127.0.0.1" })),
        interval_seconds: Set(60),
        timeout_seconds: Set(5),
        retry_count: Set(3),
        enabled: Set(true),
        status: Set("unknown".into()),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .expect("monitor criado")
    .id
}

// --- Fase 6 -----------------------------------------------------------------

#[tokio::test]
#[serial]
async fn o_catalogo_e_idempotente_e_traz_os_templates_dos_dois_roadmaps() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let catalogo = json_of(&request.get("/api/alert-rules/catalog").await.text());
        let templates = catalogo["templates"].as_array().expect("templates");
        // 18 do roadmap de alertas + 7 padrões de log (Fase 6 do de syslog).
        assert_eq!(templates.len(), 25);
        assert_eq!(catalogo["categories"]["disponibilidade"], "Disponibilidade");
        // Campos que a tela lê em cada template.
        assert!(templates[0]["applied"].is_boolean());
        assert!(templates[0].get("durationSeconds").is_some());

        // O `ensure_defaults` do initializer já provisionou os recomendados no
        // boot desta instalação nova: eles chegam marcados como aplicados.
        // São 13 — os 7 originais mais 6 padrões de log; `log_config_changed`
        // fica de fora por ser rastro de auditoria, não problema.
        let recomendados: Vec<_> = templates
            .iter()
            .filter(|item| item["recommended"] == true)
            .collect();
        assert_eq!(recomendados.len(), 13);
        for template in &recomendados {
            assert_eq!(template["applied"], true, "{}", template["key"]);
            assert!(template["ruleId"].is_i64());
        }

        // Aplicar um template fora do conjunto básico cria a regra.
        let aplicar = request
            .post("/api/alert-rules/catalog")
            .json(&serde_json::json!({ "keys": ["latency_critical", "tcp_connect_slow"] }))
            .await;
        assert_eq!(aplicar.status_code(), 201);
        let aplicado = json_of(&aplicar.text());
        assert_eq!(aplicado["created"].as_array().unwrap().len(), 2);
        assert!(aplicado["skipped"].as_array().unwrap().is_empty());
        assert_eq!(aplicado["created"][0]["isEnabled"], true);

        // Reaplicar não duplica: as duas chaves voltam em `skipped`.
        let repetido = json_of(
            &request
                .post("/api/alert-rules/catalog")
                .json(&serde_json::json!({ "keys": ["latency_critical", "device_offline"] }))
                .await
                .text(),
        );
        assert!(repetido["created"].as_array().unwrap().is_empty());
        assert_eq!(repetido["skipped"][0]["reason"], "already_exists");

        // Chave inexistente é reportada, não cria nada e não é erro.
        let desconhecida = json_of(
            &request
                .post("/api/alert-rules/catalog")
                .json(&serde_json::json!({ "keys": ["nao_existe"] }))
                .await
                .text(),
        );
        assert_eq!(desconhecida["skipped"][0]["reason"], "unknown_template");

        // Lista vazia é 422 com a mensagem que o snackbar exibe.
        let vazio = request
            .post("/api/alert-rules/catalog")
            .json(&serde_json::json!({ "keys": [] }))
            .await;
        assert_eq!(vazio.status_code(), 422);
        assert!(json_of(&vazio.text())["message"]
            .as_str()
            .unwrap()
            .contains("ao menos uma regra"));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn crud_de_regras_preserva_o_contrato_do_frontend() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let criada = request
            .post("/api/alert-rules")
            .json(&serde_json::json!({
                "name": "Latência alta",
                "type": "latency_high",
                "condition": { "field": "latencyMs", "operator": "gt", "value": 200 },
                "severity": "warning",
                "durationSeconds": 300
            }))
            .await;
        assert_eq!(criada.status_code(), 201);
        let regra = json_of(&criada.text());
        assert_eq!(regra["isEnabled"], true);
        assert_eq!(regra["durationSeconds"], 300);
        assert_eq!(regra["condition"]["operator"], "gt");
        let id = regra["id"].as_i64().unwrap();

        // Condição inválida é 422, não 500.
        let invalida = request
            .post("/api/alert-rules")
            .json(&serde_json::json!({ "name": "X", "condition": { "field": "status" } }))
            .await;
        assert_eq!(invalida.status_code(), 422);

        // PUT parcial: o toggle da lista manda só `enabled` e não pode zerar o resto.
        let atualizada = json_of(
            &request
                .put(&format!("/api/alert-rules/{id}"))
                .json(&serde_json::json!({ "enabled": false }))
                .await
                .text(),
        );
        assert_eq!(atualizada["enabled"], false);
        assert_eq!(atualizada["isEnabled"], false);
        assert_eq!(atualizada["name"], "Latência alta");
        assert_eq!(atualizada["durationSeconds"], 300);
        assert_eq!(atualizada["condition"]["value"], 200);

        assert_eq!(request.get("/api/alert-rules").await.status_code(), 200);
        assert_eq!(
            request
                .delete(&format!("/api/alert-rules/{id}"))
                .await
                .status_code(),
            204
        );
        assert_eq!(
            request
                .put(&format!("/api/alert-rules/{id}"))
                .json(&serde_json::json!({ "enabled": true }))
                .await
                .status_code(),
            404
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn monitor_caido_dispara_alerta_e_a_volta_o_resolve() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        // `device_offline` é recomendado: o initializer já o provisionou no
        // boot. A chamada abaixo confirma isso em vez de criar uma duplicata.
        let catalogo = json_of(
            &request
                .post("/api/alert-rules/catalog")
                .json(&serde_json::json!({ "keys": ["device_offline"] }))
                .await
                .text(),
        );
        assert_eq!(catalogo["skipped"][0]["reason"], "already_exists");

        // Porta 9 fechada em loopback: o TCP falha de forma determinística.
        let monitor = json_of(
            &request
                .post("/api/monitors")
                .json(&serde_json::json!({
                    "name": "TCP fechado", "type": "tcp",
                    "target": "127.0.0.1", "port": 9
                }))
                .await
                .text(),
        );
        let monitor_id = monitor["id"].as_i64().unwrap();

        assert_eq!(
            request
                .post(&format!("/api/monitors/{monitor_id}/run"))
                .await
                .status_code(),
            200
        );

        let alertas = json_of(&request.get("/api/alerts").await.text());
        let abertos = alertas.as_array().expect("modo array sem ?page");
        assert_eq!(abertos.len(), 1);
        assert_eq!(abertos[0]["status"], "active");
        assert_eq!(abertos[0]["severity"], "critical");
        assert_eq!(abertos[0]["scopeKey"], format!("monitor:{monitor_id}"));
        assert_eq!(
            abertos[0]["title"],
            "Dispositivo sem resposta — TCP fechado"
        );
        assert!(abertos[0]["silencedUntil"].is_null());

        // Rodar de novo não abre um segundo alerta para (regra, alvo).
        request
            .post(&format!("/api/monitors/{monitor_id}/run"))
            .await;
        let ainda_um = json_of(&request.get("/api/alerts").await.text());
        assert_eq!(ainda_um.as_array().unwrap().len(), 1);

        // Modo dual: com `?page` vem o envelope `{ data, meta }`.
        let paginado = json_of(&request.get("/api/alerts?page=1&limit=10").await.text());
        assert_eq!(paginado["meta"]["total"], 1);
        assert_eq!(paginado["meta"]["currentPage"], 1);
        assert_eq!(paginado["data"].as_array().unwrap().len(), 1);

        // O histórico do monitor sempre é paginado.
        let do_monitor = json_of(
            &request
                .get(&format!("/api/monitors/{monitor_id}/alerts"))
                .await
                .text(),
        );
        assert_eq!(do_monitor["meta"]["total"], 1);

        // Desabilitar o monitor normaliza os alertas abertos.
        assert_eq!(
            request
                .post(&format!("/api/monitors/{monitor_id}/disable"))
                .await
                .status_code(),
            200
        );
        let resolvidos = json_of(&request.get("/api/alerts").await.text());
        assert_eq!(resolvidos[0]["status"], "resolved");
        assert!(!resolvidos[0]["resolvedAt"].is_null());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn reconhecer_e_silenciar_alteram_o_estado_sem_fechar_o_alerta() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let monitor = json_of(
            &request
                .post("/api/monitors")
                .json(&serde_json::json!({
                    "name": "TCP fechado", "type": "tcp",
                    "target": "127.0.0.1", "port": 9
                }))
                .await
                .text(),
        );
        let monitor_id = monitor["id"].as_i64().unwrap();
        request
            .post(&format!("/api/monitors/{monitor_id}/run"))
            .await;
        // Desabilitar evita que o `acknowledge` reexecute a checagem e resolva.
        request
            .post(&format!("/api/monitors/{monitor_id}/disable"))
            .await;

        // O disable resolveu o alerta anterior; reabre um novo para o teste.
        request
            .post(&format!("/api/monitors/{monitor_id}/enable"))
            .await;
        request
            .post(&format!("/api/monitors/{monitor_id}/run"))
            .await;
        let alerta_id = json_of(&request.get("/api/alerts").await.text())
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["status"] == "active")
            .expect("alerta ativo")["id"]
            .as_i64()
            .unwrap();

        let silenciado = json_of(
            &request
                .post(&format!("/api/alerts/{alerta_id}/silence"))
                .json(&serde_json::json!({ "minutes": 30 }))
                .await
                .text(),
        );
        assert_eq!(silenciado["event"]["status"], "silenced");
        assert!(!silenciado["event"]["silencedUntil"].is_null());
        assert!(silenciado["message"]
            .as_str()
            .unwrap()
            .contains("silenciado por 30 minutos"));

        assert_eq!(
            request
                .post("/api/alerts/999999/silence")
                .await
                .status_code(),
            404
        );

        let verificados = json_of(&request.post("/api/alerts/verify-all").await.text());
        assert!(verificados["totalChecked"].as_u64().unwrap() >= 1);
    })
    .await;
}

// --- Fase 7 -----------------------------------------------------------------

#[tokio::test]
#[serial]
async fn o_protocolo_do_agente_vive_fora_do_jwt() {
    request_with_config::<App, _, _>(RequestConfig::default(), |request, ctx| async move {
        let probe_id = criar_probe(&ctx, "probe-lan", "token-lan", None).await;

        // Sem token: 401, e sem JWT nenhum envolvido.
        assert_eq!(
            request.post("/api/probes/heartbeat").await.status_code(),
            401
        );
        assert_eq!(request.get("/api/probes/tasks").await.status_code(), 401);

        // Token errado também é 401.
        assert_eq!(
            request
                .post("/api/probes/heartbeat")
                .add_header("x-probe-token", "errado")
                .json(&serde_json::json!({}))
                .await
                .status_code(),
            401
        );

        let batida = request
            .post("/api/probes/heartbeat")
            .add_header("x-probe-token", "token-lan")
            .json(&serde_json::json!({ "version": "9.9.9" }))
            .await;
        assert_eq!(batida.status_code(), 200);
        let corpo = json_of(&batida.text());
        assert_eq!(corpo["status"], "ok");
        assert_eq!(corpo["probeId"], probe_id);

        let salvo = probes::Entity::find_by_id(probe_id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(salvo.status, "online");
        assert_eq!(salvo.version.as_deref(), Some("9.9.9"));
        assert!(salvo.last_seen_at.is_some());

        // O token também pode vir no corpo (agentes anteriores ao cabeçalho).
        assert_eq!(
            request
                .post("/api/probes/heartbeat")
                .json(&serde_json::json!({ "token": "token-lan" }))
                .await
                .status_code(),
            200
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn probe_revogado_perde_o_acesso_e_a_fila() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let probe_id = criar_probe(&ctx, "probe-revogado", "token-rev", Some(Utc::now())).await;
        let monitor_id = criar_monitor(&ctx, "Ping remoto", Some(probe_id)).await;
        dispatcher::dispatch_task(
            &ctx.db,
            probe_id,
            &dispatcher::ProbeTask {
                id: "task-1-1".into(),
                monitor_id,
                task_type: "ping".into(),
                timeout_ms: 5_000,
                payload: serde_json::json!({ "host": "127.0.0.1" }),
            },
        )
        .await
        .expect("tarefa enfileirada");

        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);
        assert_eq!(
            request
                .post(&format!("/api/probes/{probe_id}/revoke"))
                .await
                .status_code(),
            200
        );

        // A fila do revogado é esvaziada junto.
        assert_eq!(
            probe_tasks::Entity::find()
                .all(&ctx.db)
                .await
                .unwrap()
                .len(),
            0
        );
        // E o token deixa de autenticar.
        assert_eq!(
            request
                .post("/api/probes/heartbeat")
                .add_header("x-probe-token", "token-rev")
                .json(&serde_json::json!({}))
                .await
                .status_code(),
            401
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn tarefa_vencida_e_descartada_e_nao_reentregue() {
    request_with_config::<App, _, _>(RequestConfig::default(), |request, ctx| async move {
        let probe_id = criar_probe(&ctx, "probe-ttl", "token-ttl", Some(Utc::now())).await;
        let fresca = criar_monitor(&ctx, "Monitor fresco", Some(probe_id)).await;
        let vencida = criar_monitor(&ctx, "Monitor vencido", Some(probe_id)).await;

        // Uma tarefa fresca e uma nascida antes do TTL.
        for (monitor_id, idade) in [
            (fresca, 0_i64),
            (vencida, dispatcher::TASK_TTL_SECONDS + 30),
        ] {
            probe_tasks::ActiveModel {
                probe_id: Set(probe_id),
                monitor_id: Set(monitor_id),
                task_id: Set(format!("task-{monitor_id}")),
                r#type: Set("ping".into()),
                timeout_ms: Set(5_000),
                payload: Set(serde_json::json!({})),
                created_at: Set((Utc::now() - Duration::seconds(idade)).into()),
                ..Default::default()
            }
            .insert(&ctx.db)
            .await
            .expect("tarefa gravada");
        }

        let entregues = json_of(
            &request
                .get("/api/probes/tasks")
                .add_header("x-probe-token", "token-ttl")
                .await
                .text(),
        );
        let tasks = entregues["tasks"].as_array().expect("lista de tarefas");
        assert_eq!(tasks.len(), 1, "só a tarefa dentro do TTL é entregue");
        assert_eq!(tasks[0]["monitorId"], fresca);
        assert_eq!(tasks[0]["type"], "ping");
        assert_eq!(tasks[0]["timeoutMs"], 5_000);

        // As duas somem da fila: a vencida não pode voltar no próximo polling.
        assert_eq!(
            probe_tasks::Entity::find()
                .all(&ctx.db)
                .await
                .unwrap()
                .len(),
            0
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn uma_tarefa_pendente_por_monitor() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let probe_id = criar_probe(&ctx, "probe-fila", "token-fila", Some(Utc::now())).await;
        let monitor_id = criar_monitor(&ctx, "Ping remoto", Some(probe_id)).await;
        for sequencia in 0..3 {
            dispatcher::dispatch_task(
                &ctx.db,
                probe_id,
                &dispatcher::ProbeTask {
                    id: format!("task-{monitor_id}-{sequencia}"),
                    monitor_id,
                    task_type: "ping".into(),
                    timeout_ms: 5_000,
                    payload: serde_json::json!({ "host": "127.0.0.1" }),
                },
            )
            .await
            .expect("tarefa enfileirada");
        }

        let pendentes = probe_tasks::Entity::find().all(&ctx.db).await.unwrap();
        assert_eq!(pendentes.len(), 1, "substituição, não acúmulo");
        assert_eq!(pendentes[0].task_id, format!("task-{monitor_id}-2"));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn o_resultado_reportado_pelo_probe_vira_historico() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header.clone(), value.clone());

        let probe_id = criar_probe(&ctx, "probe-res", "token-res", Some(Utc::now())).await;
        let monitor = json_of(
            &request
                .post("/api/monitors")
                .json(&serde_json::json!({
                    "name": "Ping remoto", "type": "ping",
                    "target": "127.0.0.1", "probeId": probe_id
                }))
                .await
                .text(),
        );
        let monitor_id = monitor["id"].as_i64().unwrap();

        let enviado = request
            .post("/api/probes/results")
            .add_header("x-probe-token", "token-res")
            .json(&serde_json::json!({
                "results": [{
                    "monitorId": monitor_id,
                    "taskId": "task-1-1",
                    "result": {
                        "success": true, "status": "up",
                        "startedAt": "2026-08-11T10:00:00Z",
                        "finishedAt": "2026-08-11T10:00:01Z",
                        "durationMs": 1000, "message": "ok",
                        "metrics": [{ "name": "latency", "value": 12.5, "unit": "ms" }],
                        "data": {}
                    }
                }]
            }))
            .await;
        assert_eq!(enviado.status_code(), 200);
        let corpo = json_of(&enviado.text());
        assert_eq!(corpo["status"], "processed");
        assert_eq!(corpo["count"], 1);

        let resultados = json_of(
            &request
                .get(&format!("/api/monitors/{monitor_id}/results"))
                .await
                .text(),
        );
        assert_eq!(resultados["meta"]["total"], 1);
        assert_eq!(resultados["data"][0]["status"], "up");
        assert_eq!(resultados["data"][0]["latencyMs"], 12.5);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn o_token_compartilhado_do_vpn_probe_autentica() {
    request_with_config::<App, _, _>(RequestConfig::default(), |request, ctx| async move {
        // Dois agentes com o **mesmo** hash: é por isso que `token_hash` não é
        // único e a autenticação não pode usar `.one()` cru.
        criar_probe(&ctx, "vpn-probe-a", DEFAULT_VPN_PROBE_TOKEN, None).await;
        criar_probe(&ctx, "vpn-probe-b", DEFAULT_VPN_PROBE_TOKEN, None).await;

        let resposta = request
            .post("/api/probes/heartbeat")
            .add_header("x-probe-token", DEFAULT_VPN_PROBE_TOKEN)
            .json(&serde_json::json!({}))
            .await;
        assert_eq!(resposta.status_code(), 200);
        assert_eq!(json_of(&resposta.text())["status"], "ok");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn o_ciclo_do_scheduler_despacha_para_probe_vivo_e_marca_o_morto_offline() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let vivo = criar_probe(&ctx, "probe-vivo", "token-vivo", Some(Utc::now())).await;
        let morto = criar_probe(
            &ctx,
            "probe-morto",
            "token-morto",
            Some(Utc::now() - Duration::seconds(600)),
        )
        .await;
        // O watchdog só rebaixa quem está "em serviço".
        for id in [vivo, morto] {
            let mut ativo: probes::ActiveModel = probes::Entity::find_by_id(id)
                .one(&ctx.db)
                .await
                .unwrap()
                .unwrap()
                .into();
            ativo.status = Set("online".into());
            ativo.update(&ctx.db).await.unwrap();
        }

        let monitor_vivo = json_of(
            &request
                .post("/api/monitors")
                .json(&serde_json::json!({
                    "name": "Remoto vivo", "type": "tcp",
                    "target": "127.0.0.1", "port": 9, "probeId": vivo
                }))
                .await
                .text(),
        )["id"]
            .as_i64()
            .unwrap();

        backend::tasks::scheduler_run::run_cycle(&ctx)
            .await
            .expect("ciclo do scheduler");

        // Probe vivo: a checagem virou tarefa na fila, não resultado local.
        let fila = probe_tasks::Entity::find().all(&ctx.db).await.unwrap();
        assert_eq!(fila.len(), 1);
        assert_eq!(fila[0].monitor_id, monitor_vivo);
        assert_eq!(fila[0].probe_id, vivo);
        assert_eq!(fila[0].r#type, "tcp");

        // Probe sem heartbeat foi rebaixado; o vivo continua online.
        let estado = |id: i64| {
            let db = ctx.db.clone();
            async move {
                probes::Entity::find_by_id(id)
                    .one(&db)
                    .await
                    .unwrap()
                    .unwrap()
                    .status
            }
        };
        assert_eq!(estado(morto).await, "offline");
        assert_eq!(estado(vivo).await, "online");
    })
    .await;
}
