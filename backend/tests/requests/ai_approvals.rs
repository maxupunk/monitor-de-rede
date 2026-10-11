//! Modo automático com critério e pedidos de aprovação da IA.
//!
//! O que se afirma aqui: no automático, a regra de cada ferramenta decide —
//! reativar uma regra roda, desativar ou excluir pergunta, e a pergunta não
//! tem atalho para executar; o pedido guardado no balcão é confirmado fora do
//! chat executando a proposta do servidor; e a proposta decidida numa tela
//! não roda de novo em outra.

use backend::{
    app::App,
    models::_entities::{alert_rules, devices},
    services::ai::{
        approvals::{self, ApprovalRequest},
        harness::tools::{AiToolRisk, RiskAssessment, ToolArgs, ToolPolicy, ToolRegistry, Verdict},
        settings::{self, AiAutomationMode, AiSettings},
    },
};
use loco_rs::{prelude::AppContext, testing::prelude::*};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde_json::{json, Value};
use serial_test::serial;

use super::prepare_data;

const AUTOMATICO: ToolPolicy = ToolPolicy {
    allow_active: false,
    actions: AiAutomationMode::Auto,
    confirm_active: false,
    interactive: true,
    container_actions: AiAutomationMode::Off,
};

async fn regra(ctx: &AppContext) -> alert_rules::Model {
    let device = devices::ActiveModel {
        name: Set("Borda".into()),
        r#type: Set("router".into()),
        ip_address: Set(Some("10.0.0.1".into())),
        is_monitored: Set(true),
        status: Set("up".into()),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap();
    alert_rules::ActiveModel {
        device_id: Set(Some(device.id)),
        name: Set("Latência alta".into()),
        r#type: Set("custom".into()),
        condition: Set(json!({ "field": "latencyMs", "operator": "gt", "value": 150 })),
        severity: Set("warning".into()),
        enabled: Set(true),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap()
}

async fn ligar_assistente(ctx: &AppContext) {
    settings::save(
        &ctx.db,
        AiSettings {
            enabled: true,
            action_mode: AiAutomationMode::Auto,
            ..AiSettings::default()
        },
    )
    .await
    .unwrap();
}

fn pedir(ctx: &AppContext, call_id: &str, user_id: i64, arguments: Value) {
    approvals::open(
        ctx,
        ApprovalRequest {
            call_id,
            user_id,
            tool: "toggle_alert_rule",
            arguments,
            summary: "Desativar a regra",
            risk: &RiskAssessment::high("Desativa a regra: os alertas dela deixam de abrir"),
            conversation_key: None,
        },
    );
}

async fn ativa(ctx: &AppContext, id: i64) -> bool {
    alert_rules::Entity::find_by_id(id)
        .one(&ctx.db)
        .await
        .unwrap()
        .expect("regra existe")
        .enabled
}

#[tokio::test]
#[serial]
async fn automatico_roda_o_risco_baixo_e_pergunta_o_alto() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let rule = regra(&ctx).await;
        let registro = ToolRegistry::new(AUTOMATICO);
        assert!(!registro.needs_confirmation("toggle_alert_rule"));

        let desativar =
            ToolArgs::parse(&json!({ "rule_id": rule.id, "enabled": false }).to_string());
        let avaliado = registro
            .assess(&ctx, "toggle_alert_rule", &desativar)
            .await
            .unwrap();
        assert_eq!(avaliado.risk.level, AiToolRisk::High);
        assert!(matches!(avaliado.verdict, Verdict::Ask));
        assert!(
            registro
                .execute_in(&ctx, "toggle_alert_rule", desativar)
                .await
                .is_err(),
            "risco alto não roda sem confirmação, nem chamado direto"
        );
        assert!(ativa(&ctx, rule.id).await);

        let excluir = ToolArgs::parse(&json!({ "rule_id": rule.id }).to_string());
        let avaliado = registro
            .assess(&ctx, "delete_alert_rule", &excluir)
            .await
            .unwrap();
        assert!(matches!(avaliado.verdict, Verdict::Ask));

        let reconhecer = ToolArgs::parse(&json!({ "alert_id": 1 }).to_string());
        let avaliado = registro
            .assess(&ctx, "acknowledge_alert", &reconhecer)
            .await
            .unwrap();
        assert_eq!(avaliado.risk.level, AiToolRisk::Low);
        assert!(matches!(avaliado.verdict, Verdict::Run(_)));

        let curto = ToolArgs::parse(&json!({ "alert_id": 1, "minutes": 30 }).to_string());
        let longo = ToolArgs::parse(&json!({ "alert_id": 1, "minutes": 240 }).to_string());
        let silenciar_curto = registro
            .assess(&ctx, "silence_alert", &curto)
            .await
            .unwrap();
        let silenciar_longo = registro
            .assess(&ctx, "silence_alert", &longo)
            .await
            .unwrap();
        assert_eq!(silenciar_curto.risk.level, AiToolRisk::Medium);
        assert!(matches!(silenciar_curto.verdict, Verdict::Run(_)));
        assert!(matches!(silenciar_longo.verdict, Verdict::Ask));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn pedido_confirmado_fora_do_chat_executa_a_proposta_guardada_uma_vez() {
    request_with_config::<App, _, _>(RequestConfig::default(), |request, ctx| async move {
        ligar_assistente(&ctx).await;
        let sessao = prepare_data::init_operator(&ctx).await;
        let (h, v) = prepare_data::auth_header(&sessao.token);
        let rule = regra(&ctx).await;
        pedir(
            &ctx,
            "chamada-1",
            sessao.user.id,
            json!({ "rule_id": rule.id, "enabled": false }),
        );

        let feito = request
            .post("/api/ai/approvals/chamada-1/confirm")
            .add_header(h.clone(), v.clone())
            .await;
        assert_eq!(feito.status_code(), 200, "{}", feito.text());
        assert_eq!(feito.json::<Value>()["result"]["enabled"], false);
        assert!(!ativa(&ctx, rule.id).await);

        let de_novo = request
            .post("/api/ai/approvals/chamada-1/confirm")
            .add_header(h.clone(), v.clone())
            .await;
        assert_eq!(de_novo.status_code(), 409, "decidida não roda duas vezes");

        let pelo_card = request
            .post("/api/ai/tools/execute")
            .add_header(h.clone(), v.clone())
            .json(&json!({
                "name": "toggle_alert_rule",
                "arguments": { "rule_id": rule.id, "enabled": true },
                "callId": "chamada-1",
            }))
            .await;
        assert_eq!(
            pelo_card.status_code(),
            409,
            "o card de outra aba também é recusado"
        );
        assert!(!ativa(&ctx, rule.id).await);

        let inexistente = request
            .post("/api/ai/approvals/nunca-existiu/confirm")
            .add_header(h, v)
            .await;
        assert_eq!(inexistente.status_code(), 404);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn pedido_cancelado_nao_roda_pelo_card_de_outra_aba() {
    request_with_config::<App, _, _>(RequestConfig::default(), |request, ctx| async move {
        ligar_assistente(&ctx).await;
        let sessao = prepare_data::init_operator(&ctx).await;
        let (h, v) = prepare_data::auth_header(&sessao.token);
        let rule = regra(&ctx).await;
        pedir(
            &ctx,
            "chamada-2",
            sessao.user.id,
            json!({ "rule_id": rule.id, "enabled": false }),
        );

        let cancelado = request
            .post("/api/ai/approvals/chamada-2/dismiss")
            .add_header(h.clone(), v.clone())
            .await;
        assert_eq!(cancelado.status_code(), 200, "{}", cancelado.text());

        let pelo_card = request
            .post("/api/ai/tools/execute")
            .add_header(h.clone(), v.clone())
            .json(&json!({
                "name": "toggle_alert_rule",
                "arguments": { "rule_id": rule.id, "enabled": false },
                "callId": "chamada-2",
            }))
            .await;
        assert_eq!(pelo_card.status_code(), 409);
        assert!(ativa(&ctx, rule.id).await);

        // Sem pendência no balcão (reinício, por exemplo), o card segue
        // valendo do jeito de sempre.
        let sem_pendencia = request
            .post("/api/ai/tools/execute")
            .add_header(h, v)
            .json(&json!({
                "name": "toggle_alert_rule",
                "arguments": { "rule_id": rule.id, "enabled": false },
                "callId": "depois-do-reinicio",
            }))
            .await;
        assert_eq!(sem_pendencia.status_code(), 200, "{}", sem_pendencia.text());
        assert!(!ativa(&ctx, rule.id).await);
    })
    .await;
}
