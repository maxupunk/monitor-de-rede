//! `grep` da IA contra os bancos de verdade (principal e de logs).
//!
//! O que se afirma aqui: o casamento é de substring como no grep (inclusive
//! quando o banco pré-filtra), regex e exclusão funcionam, cada modo de
//! saída devolve só a sua forma, o `auto` nunca despeja tudo, o contexto
//! (`-C`) traz os vizinhos da mesma origem, e alertas e checagens também são
//! pesquisáveis.

use backend::{
    app::App,
    models::{
        _entities::{alert_events, devices, monitor_results, monitors},
        logs::device_logs,
    },
    services::{
        ai::harness::tools::{ToolPolicy, ToolRegistry},
        syslog::LogsDb,
    },
};
use chrono::{Duration, Utc};
use loco_rs::{prelude::AppContext, testing::prelude::*};
use sea_orm::{ActiveModelTrait, Set};
use serde_json::{json, Value};
use serial_test::serial;

async fn aparelho(ctx: &AppContext, nome: &str, ip: &str) -> devices::Model {
    devices::ActiveModel {
        name: Set(nome.into()),
        r#type: Set("router".into()),
        ip_address: Set(Some(ip.into())),
        status: Set("up".into()),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap()
}

async fn log(ctx: &AppContext, device_id: i64, segundos_atras: i64, severidade: i16, texto: &str) {
    let logs = LogsDb::from_context(ctx).expect("banco de logs de teste");
    let quando = Utc::now() - Duration::seconds(segundos_atras);
    device_logs::ActiveModel {
        device_id: Set(Some(device_id)),
        source_ip: Set("10.0.0.1".into()),
        received_at: Set(quando.into()),
        severity: Set(Some(severidade)),
        message: Set(texto.into()),
        source: Set("syslog".into()),
        created_at: Set(quando.into()),
        ..Default::default()
    }
    .insert(logs.connection())
    .await
    .expect("gravar log");
}

async fn grep(ctx: &AppContext, args: Value) -> Value {
    ToolRegistry::new(ToolPolicy::passive())
        .execute(ctx, "grep", &args.to_string())
        .await
        .expect("grep")
        .data
}

#[tokio::test]
#[serial]
async fn casa_substring_como_o_grep_e_aceita_regex_com_exclusao() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let borda = aparelho(&ctx, "Borda", "10.0.0.1").await;
        log(&ctx, borda.id, 50, 3, "ether1 link down").await;
        log(&ctx, borda.id, 40, 3, "UPLINK DOWN on sfp1").await;
        log(&ctx, borda.id, 30, 4, "ether9 link flap").await;
        log(&ctx, borda.id, 20, 6, "dhcp lease renewed").await;

        let literal = grep(&ctx, json!({ "pattern": "link down", "output": "lines" })).await;
        assert_eq!(
            literal["matched"], 2,
            "'link down' casa dentro de 'UPLINK DOWN', sem caixa: {literal}"
        );

        let regex = grep(
            &ctx,
            json!({ "pattern": "link (down|flap)", "regex": true, "exclude": "ether9", "output": "count" }),
        )
        .await;
        assert_eq!(regex["matched"], 2);
        assert!(regex.get("lines").is_none(), "count não traz linhas");

        let invalida = grep(&ctx, json!({ "pattern": "(abc", "regex": true })).await;
        assert!(invalida["error"].as_str().unwrap().contains("Regex inválida"));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn cada_modo_devolve_so_a_sua_forma_e_o_auto_resume_quando_nao_cabe() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let borda = aparelho(&ctx, "Borda", "10.0.0.1").await;
        let core = aparelho(&ctx, "Core", "10.0.0.2").await;
        for i in 0..6 {
            log(
                &ctx,
                borda.id,
                100 - i,
                3,
                &format!("login failure for admin from 192.168.1.{i}"),
            )
            .await;
        }
        log(&ctx, core.id, 10, 3, "login failure for root from 10.9.9.9").await;

        let origens = grep(
            &ctx,
            json!({ "pattern": "login failure", "output": "sources" }),
        )
        .await;
        assert_eq!(origens["sources"][0]["origin"], "Borda");
        assert_eq!(origens["sources"][0]["count"], 6);
        assert_eq!(origens["sources"][1]["origin"], "Core");

        let padroes = grep(
            &ctx,
            json!({ "pattern": "login failure", "output": "patterns" }),
        )
        .await;
        assert_eq!(
            padroes["patterns"][0]["pattern"],
            "login failure for admin from #"
        );
        assert_eq!(padroes["patterns"][0]["count"], 6);

        let auto = grep(&ctx, json!({ "pattern": "login failure", "max_lines": 3 })).await;
        assert_eq!(auto["output"], "summary");
        assert!(
            auto.get("lines").is_none(),
            "7 ocorrências não cabem em 3 linhas"
        );
        assert_eq!(auto["summary"]["matched"], 7);
        assert!(auto["summary"]["hint"].is_string());

        let cabe = grep(&ctx, json!({ "pattern": "root", "device": "core" })).await;
        assert_eq!(cabe["output"], "lines");
        assert_eq!(cabe["lines"][0]["origin"], "Core");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn contexto_traz_o_que_veio_antes_e_depois_na_mesma_origem() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let borda = aparelho(&ctx, "Borda", "10.0.0.1").await;
        let core = aparelho(&ctx, "Core", "10.0.0.2").await;
        log(&ctx, borda.id, 60, 6, "sfp1 rx power -28 dBm").await;
        log(&ctx, core.id, 55, 6, "de outra origem, não entra").await;
        log(&ctx, borda.id, 50, 3, "sfp1 link down").await;
        log(&ctx, borda.id, 40, 6, "sfp1 link up").await;

        let saida = grep(
            &ctx,
            json!({ "pattern": "link down", "output": "lines", "context": 1 }),
        )
        .await;
        let linha = &saida["lines"][0];
        assert_eq!(linha["before"][0]["text"], "sfp1 rx power -28 dBm");
        assert_eq!(linha["after"][0]["text"], "sfp1 link up");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn alertas_e_checagens_tambem_sao_pesquisaveis() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let borda = aparelho(&ctx, "Borda", "10.0.0.1").await;
        let agora = Utc::now();
        alert_events::ActiveModel {
            device_id: Set(Some(borda.id)),
            status: Set("active".into()),
            severity: Set("critical".into()),
            started_at: Set(agora.into()),
            message: Set(Some("Perda de pacotes acima de 20%".into())),
            created_at: Set(agora.into()),
            updated_at: Set(agora.into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();

        let monitor = monitors::ActiveModel {
            device_id: Set(Some(borda.id)),
            r#type: Set("http".into()),
            name: Set("Portal".into()),
            configuration: Set(json!({ "url": "http://10.0.0.1" })),
            interval_seconds: Set(60),
            timeout_seconds: Set(5),
            retry_count: Set(3),
            enabled: Set(true),
            status: Set("down".into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();
        for (status, mensagem) in [("down", "TLS handshake timeout"), ("up", "OK")] {
            monitor_results::ActiveModel {
                monitor_id: Set(monitor.id),
                status: Set(status.into()),
                started_at: Set(agora.into()),
                finished_at: Set(agora.into()),
                duration_ms: Set(10),
                message: Set(Some(mensagem.into())),
                created_at: Set(agora.into()),
                ..Default::default()
            }
            .insert(&ctx.db)
            .await
            .unwrap();
        }

        let alertas = grep(&ctx, json!({ "source": "alerts", "pattern": "perda" })).await;
        assert_eq!(alertas["matched"], 1);
        assert_eq!(alertas["lines"][0]["origin"], "Borda");
        assert!(alertas["lines"][0]["text"]
            .as_str()
            .unwrap()
            .starts_with('#'));

        let checagens = grep(
            &ctx,
            json!({ "source": "checks", "pattern": "tls|ok", "regex": true }),
        )
        .await;
        assert_eq!(
            checagens["matched"], 1,
            "só falhas entram: o 'OK' do up fica fora"
        );
        assert_eq!(checagens["lines"][0]["origin"], "Portal");

        let fonte = grep(&ctx, json!({ "source": "tudo" })).await;
        assert!(fonte["error"]
            .as_str()
            .unwrap()
            .contains("logs, alerts, checks"));
    })
    .await;
}
