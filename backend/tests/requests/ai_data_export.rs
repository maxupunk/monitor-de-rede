//! `/api/ai/data-export` — o pacote com as conversas, os comandos e as
//! execuções em equipamentos da IA, para treino e revisão.
//!
//! O que se afirma aqui: só o administrador baixa (é o histórico de todos);
//! o resumo conta o que o pacote leva; o `.zip` traz cada arquivo, com a
//! conversa que falhou separada da que não falhou e só as execuções pedidas
//! pela IA.

use std::io::{Cursor, Read};

use backend::app::App;
use chrono::{Duration, Utc};
use loco_rs::{app::AppContext, testing::prelude::*};
use netmonitor_core::models::{ai_conversations, devices, plugin_runs};
use netmonitor_core::roles::Role;
use sea_orm::{ActiveModelTrait, Set};
use serde_json::{json, Value};
use serial_test::serial;

use super::prepare_data::session_with_role;

async fn conversa(ctx: &AppContext, user_id: i64, messages: Value, dias_atras: i64) {
    let quando = Utc::now() - Duration::days(dias_atras);
    ai_conversations::ActiveModel {
        user_id: Set(user_id),
        title: Set("conversa".into()),
        message_count: Set(i32::try_from(messages.as_array().unwrap().len()).unwrap()),
        messages: Set(messages),
        created_at: Set(quando.into()),
        updated_at: Set(quando.into()),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .expect("conversa");
}

async fn execucao(ctx: &AppContext, device_id: i64, origin: &str) {
    plugin_runs::ActiveModel {
        device_id: Set(device_id),
        action: Set("ssh".into()),
        origin: Set(origin.into()),
        status: Set("success".into()),
        transcript: Set(json!([{ "send": "uptime", "recv": "up 3 days" }])),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .expect("execução");
}

fn arquivo(zip: &mut zip::ZipArchive<Cursor<Vec<u8>>>, nome: &str) -> String {
    let mut texto = String::new();
    zip.by_name(nome)
        .unwrap_or_else(|_| panic!("{nome} no pacote"))
        .read_to_string(&mut texto)
        .unwrap();
    texto
}

fn linhas(texto: &str) -> Vec<Value> {
    texto
        .lines()
        .map(|linha| serde_json::from_str(linha).unwrap())
        .collect()
}

#[tokio::test]
#[serial]
async fn so_o_administrador_exporta() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Operator).await;
        let resumo = request.get("/api/ai/data-export/summary").await;
        assert_eq!(resumo.status_code(), 403, "{}", resumo.text());
        let pacote = request.get("/api/ai/data-export").await;
        assert_eq!(pacote.status_code(), 403, "{}", pacote.text());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn pacote_separa_falhas_e_leva_so_as_execucoes_da_ia() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let admin = session_with_role(&mut request, &ctx, Role::Admin).await;

        conversa(
            &ctx,
            admin.id,
            json!([
                { "role": "user", "content": "ping no gateway" },
                { "role": "assistant", "content": "2 ms", "toolCalls": [
                    { "id": "c1", "name": "ping", "arguments": { "host": "10.0.0.1" },
                      "result": { "latencyMs": 2 }, "status": "done" }
                ]},
            ]),
            1,
        )
        .await;
        conversa(
            &ctx,
            admin.id,
            json!([
                { "role": "user", "content": "reinicie o AP" },
                { "role": "assistant", "content": "Não consegui.", "toolCalls": [
                    { "name": "reboot", "status": "error", "result": { "error": "timeout" } }
                ]},
            ]),
            2,
        )
        .await;
        conversa(
            &ctx,
            admin.id,
            json!([{ "role": "user", "content": "antiga" }]),
            40,
        )
        .await;

        let device = devices::ActiveModel {
            name: Set("ap-sala".into()),
            r#type: Set("router".into()),
            is_monitored: Set(true),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();
        execucao(&ctx, device.id, "ai").await;
        execucao(&ctx, device.id, "user").await;

        let resumo = request.get("/api/ai/data-export/summary?days=30").await;
        assert_eq!(resumo.status_code(), 200, "{}", resumo.text());
        let resumo: Value = resumo.json();
        assert_eq!(resumo["conversations"], 2, "a de 40 dias fica fora");
        assert_eq!(resumo["messages"], 4);
        assert_eq!(resumo["users"], 1);
        assert_eq!(resumo["deviceRuns"], 1, "só a execução pedida pela IA");

        let tudo: Value = request.get("/api/ai/data-export/summary").await.json();
        assert_eq!(tudo["conversations"], 3);

        let invalido = request.get("/api/ai/data-export/summary?days=0").await;
        assert_eq!(invalido.status_code(), 422, "{}", invalido.text());

        let pacote = request.get("/api/ai/data-export?days=30").await;
        assert_eq!(pacote.status_code(), 200, "{}", pacote.text());
        assert_eq!(pacote.header("content-type"), "application/zip");
        let disposicao = pacote.header("content-disposition");
        let disposicao = disposicao.to_str().unwrap();
        assert!(
            disposicao.starts_with("attachment; filename=\"netmonitor-ia-dados-"),
            "{disposicao}"
        );

        let mut zip = zip::ZipArchive::new(Cursor::new(pacote.as_bytes().to_vec())).unwrap();
        assert!(arquivo(&mut zip, "LEIAME.md").contains("conversas.jsonl"));

        let conversas = linhas(&arquivo(&mut zip, "conversas.jsonl"));
        assert_eq!(conversas.len(), 2);
        assert_eq!(conversas[0]["outcome"], "noFailures");
        assert_eq!(conversas[0]["user"]["name"], admin.name);
        assert_eq!(conversas[1]["outcome"], "withFailures");
        assert_eq!(conversas[1]["signals"]["failures"][0]["detail"], "timeout");

        let limpas = linhas(&arquivo(&mut zip, "treino/sem-falhas.jsonl"));
        assert_eq!(limpas.len(), 1);
        assert_eq!(
            limpas[0]["messages"][1]["tool_calls"][0]["function"]["name"],
            "ping"
        );
        assert_eq!(
            linhas(&arquivo(&mut zip, "treino/com-falhas.jsonl")).len(),
            1
        );

        let execucoes = linhas(&arquivo(&mut zip, "execucoes-em-equipamentos.jsonl"));
        assert_eq!(execucoes.len(), 1);
        assert_eq!(execucoes[0]["device"]["name"], "ap-sala");
        assert_eq!(execucoes[0]["transcript"][0]["send"], "uptime");

        let manifesto: Value = serde_json::from_str(&arquivo(&mut zip, "manifesto.json")).unwrap();
        assert_eq!(manifesto["periodDays"], 30);
        assert_eq!(manifesto["totals"]["conversationsWithFailures"], 1);
        assert_eq!(manifesto["totals"]["deviceRuns"], 1);
        assert_eq!(manifesto["totals"]["toolFailures"], json!({ "reboot": 1 }));
    })
    .await;
}
