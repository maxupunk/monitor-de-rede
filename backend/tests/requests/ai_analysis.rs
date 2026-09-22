//! Ferramentas de análise e de logs da IA contra os dois bancos (principal e
//! de logs), e as ações confirmadas pelo usuário.
//!
//! O que se afirma aqui: os logs chegam agrupados por padrão e com repetições
//! colapsadas; a linha do tempo junta alerta, mudança de status e log; ação
//! nunca roda sem passar pela confirmação, e quando roda usa os mesmos
//! serviços da tela.

use backend::{
    app::App,
    models::{
        _entities::{alert_events, devices, maintenance_windows, monitor_results, monitors},
        logs::device_logs,
    },
    services::{
        ai::{
            harness::{
                confirmation,
                tools::{ToolArgs, ToolPolicy, ToolRegistry},
            },
            settings::{self, AiSettings},
        },
        audit::AuditActor,
        syslog::LogsDb,
    },
};
use chrono::{Duration, Utc};
use loco_rs::{prelude::AppContext, testing::prelude::*};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::json;
use serial_test::serial;

const LEITURA: ToolPolicy = ToolPolicy {
    allow_active: false,
    allow_actions: false,
    confirm_active: false,
};

const COM_ACOES: ToolPolicy = ToolPolicy {
    allow_active: false,
    allow_actions: true,
    confirm_active: false,
};

async fn aparelho(ctx: &AppContext) -> devices::Model {
    devices::ActiveModel {
        name: Set("Borda Principal".into()),
        r#type: Set("router".into()),
        ip_address: Set(Some("10.0.0.1".into())),
        is_monitored: Set(true),
        status: Set("up".into()),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap()
}

async fn ping(ctx: &AppContext, device_id: i64) -> monitors::Model {
    monitors::ActiveModel {
        device_id: Set(Some(device_id)),
        r#type: Set("ping".into()),
        name: Set("Ping Borda".into()),
        configuration: Set(json!({ "host": "10.0.0.1" })),
        interval_seconds: Set(60),
        timeout_seconds: Set(5),
        retry_count: Set(3),
        enabled: Set(true),
        status: Set("up".into()),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap()
}

async fn checagem(ctx: &AppContext, monitor_id: i64, minutos_atras: i64, status: &str) {
    let quando = Utc::now() - Duration::minutes(minutos_atras);
    monitor_results::ActiveModel {
        monitor_id: Set(monitor_id),
        status: Set(status.into()),
        started_at: Set(quando.into()),
        finished_at: Set(quando.into()),
        duration_ms: Set(10),
        latency_ms: Set((status == "up").then_some(15.0)),
        message: Set((status != "up").then(|| "Timeout".to_string())),
        created_at: Set(quando.into()),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap();
}

async fn alerta(ctx: &AppContext, device_id: i64, minutos_atras: i64) -> alert_events::Model {
    let inicio = Utc::now() - Duration::minutes(minutos_atras);
    alert_events::ActiveModel {
        device_id: Set(Some(device_id)),
        status: Set("active".into()),
        severity: Set("critical".into()),
        started_at: Set(inicio.into()),
        message: Set(Some("Borda fora do ar".into())),
        created_at: Set(inicio.into()),
        updated_at: Set(inicio.into()),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap()
}

async fn log(
    ctx: &AppContext,
    device_id: Option<i64>,
    severidade: i16,
    minutos_atras: i64,
    mensagem: &str,
) {
    let logs = LogsDb::from_context(ctx).expect("banco de logs de teste");
    let quando = Utc::now() - Duration::minutes(minutos_atras);
    device_logs::ActiveModel {
        device_id: Set(device_id),
        source_ip: Set("10.0.0.1".into()),
        received_at: Set(quando.into()),
        severity: Set(Some(severidade)),
        app_name: Set(Some("interface".into())),
        message: Set(mensagem.into()),
        source: Set("syslog".into()),
        created_at: Set(quando.into()),
        ..Default::default()
    }
    .insert(logs.connection())
    .await
    .expect("gravar log");
}

#[tokio::test]
#[serial]
async fn logs_chegam_agrupados_por_padrao_e_com_repeticoes_colapsadas() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let dev = aparelho(&ctx).await;
        for (i, minuto) in [5, 4, 3, 2, 1].iter().enumerate() {
            log(
                &ctx,
                Some(dev.id),
                3,
                *minuto,
                &format!("ether3 link down (flap {i})"),
            )
            .await;
        }
        log(&ctx, Some(dev.id), 6, 10, "dhcp lease 10.0.0.50 renewed").await;

        let registro = ToolRegistry::new(LEITURA);
        let visao = registro
            .execute(&ctx, "get_logs_overview", r#"{"device": "borda"}"#)
            .await
            .unwrap();
        let digest = &visao.data["digest"];
        assert_eq!(digest["total"], 6);
        assert_eq!(digest["errors"], 5);
        assert_eq!(
            digest["top_error_patterns"][0]["pattern"],
            "ether# link down (flap #)"
        );
        assert_eq!(digest["top_error_patterns"][0]["count"], 5);
        assert_eq!(digest["top_devices"][0]["device"], "Borda Principal");

        let busca = registro
            .execute(
                &ctx,
                "grep",
                r#"{"pattern": "link down", "severity": "error", "output": "lines"}"#,
            )
            .await
            .unwrap();
        assert_eq!(busca.data["matched"], 5);
        let linhas = busca.data["lines"].as_array().unwrap();
        assert_eq!(linhas.len(), 1, "cinco linhas iguais viram uma");
        assert_eq!(linhas[0]["repeated"], 5);
        assert_eq!(linhas[0]["severity"], "erro");

        let invalida = registro
            .execute(&ctx, "grep", r#"{"severity": "gravíssimo"}"#)
            .await
            .unwrap();
        assert!(invalida.data["error"]
            .as_str()
            .unwrap()
            .contains("Severidade"));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn linha_do_tempo_junta_alerta_mudanca_de_status_e_log() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let dev = aparelho(&ctx).await;
        let mon = ping(&ctx, dev.id).await;
        checagem(&ctx, mon.id, 20, "up").await;
        checagem(&ctx, mon.id, 15, "down").await;
        checagem(&ctx, mon.id, 5, "up").await;
        alerta(&ctx, dev.id, 14).await;
        log(&ctx, Some(dev.id), 3, 16, "sfp1 link down").await;
        log(&ctx, Some(dev.id), 6, 16, "informativo que não entra").await;

        let saida = ToolRegistry::new(LEITURA)
            .execute(
                &ctx,
                "get_incident_timeline",
                r#"{"device": "Borda Principal", "minutes": 30}"#,
            )
            .await
            .unwrap();
        let tipos: Vec<&str> = saida.data["events"]
            .as_array()
            .unwrap()
            .iter()
            .map(|evento| evento["kind"].as_str().unwrap())
            .collect();
        assert_eq!(
            tipos,
            vec!["log", "status_change", "alert_opened", "status_change"],
            "cronológico; o log informativo fica fora"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn analises_sem_historico_respondem_sem_inventar() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let dev = aparelho(&ctx).await;
        let mon = ping(&ctx, dev.id).await;
        checagem(&ctx, mon.id, 1, "up").await;

        let registro = ToolRegistry::new(LEITURA);
        let baseline = registro
            .execute(&ctx, "compare_with_baseline", r#"{"device": "borda"}"#)
            .await
            .unwrap();
        assert_eq!(baseline.data["has_baseline"], false);

        let causa = registro
            .execute(&ctx, "analyze_root_cause", "{}")
            .await
            .unwrap();
        assert_eq!(causa.data["open_incidents"], 0);

        let inexistente = registro
            .execute(&ctx, "analyze_root_cause", r#"{"alert_id": 999}"#)
            .await
            .unwrap();
        assert!(inexistente.data["error"].is_string());

        let padrao = registro
            .execute(
                &ctx,
                "get_hourly_pattern",
                &format!(r#"{{"monitor_id": {}, "days": 1}}"#, mon.id),
            )
            .await
            .unwrap();
        let grafico = padrao.chart.expect("gráfico por hora");
        assert_eq!(
            serde_json::to_value(grafico.x_axis).unwrap(),
            "label",
            "o eixo mostra '14h', não uma data"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn acao_so_roda_confirmada_e_pelos_servicos_da_tela() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let dev = aparelho(&ctx).await;
        let evento = alerta(&ctx, dev.id, 5).await;
        let registro = ToolRegistry::new(COM_ACOES);
        let argumentos = format!(r#"{{"alert_id": {}, "minutes": 30}}"#, evento.id);

        assert!(registro.needs_confirmation("silence_alert"));
        assert!(
            registro
                .execute(&ctx, "silence_alert", &argumentos)
                .await
                .is_err(),
            "o chat não executa ação sozinho"
        );
        let resumo = registro
            .preview(&ctx, "silence_alert", &argumentos)
            .await
            .unwrap();
        assert!(resumo.contains("por 30 min"), "{resumo}");
        assert!(resumo.contains("Borda Principal"), "{resumo}");

        let feito = registro
            .execute_confirmed(&ctx, "silence_alert", ToolArgs::parse(&argumentos))
            .await
            .unwrap();
        assert_eq!(feito.data["status"], "silenced");

        let passiva = registro
            .execute_confirmed(&ctx, "get_alerts", ToolArgs::parse("{}"))
            .await;
        assert!(
            passiva.is_err(),
            "a rota de confirmação não é porta de consulta"
        );

        let sem_acoes = ToolRegistry::new(LEITURA)
            .execute_confirmed(&ctx, "silence_alert", ToolArgs::parse(&argumentos))
            .await;
        assert!(
            sem_acoes.is_err(),
            "ação desligada nas configurações não roda"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn confirmacao_cria_janela_e_monitor_em_nome_do_usuario() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let dev = aparelho(&ctx).await;
        let ator = AuditActor {
            user_id: None,
            ip: Some("127.0.0.1".into()),
            user_agent: None,
        };

        let desligado = confirmation::execute_confirmed(
            &ctx,
            &AiSettings::default(),
            "create_monitor",
            json!({ "device": "borda" }),
            ator.clone(),
        )
        .await;
        assert!(desligado.is_err(), "assistente desativado não executa nada");

        let ligado = settings::save(
            &ctx.db,
            AiSettings {
                enabled: true,
                allow_actions: true,
                ..AiSettings::default()
            },
        )
        .await
        .unwrap();

        let janela = confirmation::execute_confirmed(
            &ctx,
            &ligado,
            "create_maintenance_window",
            json!({ "device": "borda", "duration_minutes": 90, "reason": "troca de SFP" }),
            ator.clone(),
        )
        .await
        .unwrap();
        let criada =
            maintenance_windows::Entity::find_by_id(janela.data["window_id"].as_i64().unwrap())
                .one(&ctx.db)
                .await
                .unwrap()
                .expect("janela gravada");
        assert_eq!(criada.device_id, Some(dev.id));
        assert_eq!(criada.description.as_deref(), Some("troca de SFP"));
        assert_eq!((criada.ends_at - criada.starts_at).num_minutes(), 90);

        let monitor = confirmation::execute_confirmed(
            &ctx,
            &ligado,
            "create_monitor",
            json!({ "device": "borda", "type": "tcp", "port": 22 }),
            ator,
        )
        .await
        .unwrap();
        let criado = monitors::Entity::find()
            .filter(monitors::Column::Id.eq(monitor.data["monitor_id"].as_i64().unwrap()))
            .one(&ctx.db)
            .await
            .unwrap()
            .expect("monitor gravado");
        assert_eq!(criado.r#type, "tcp");
        assert_eq!(criado.device_id, Some(dev.id));
        assert_eq!(criado.configuration["host"], "10.0.0.1");
        assert_eq!(criado.configuration["port"], 22);
    })
    .await;
}
