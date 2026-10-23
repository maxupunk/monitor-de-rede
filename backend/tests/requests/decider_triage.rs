//! Triagem do resumo automático de incidente pelo Decisor.
//!
//! O que se afirma aqui, contra o Ollama falso: no modo "decidir", um alerta
//! que o Decisor não acha acionável fica marcado como suprimido, com a opinião
//! gravada sem apagar o resto do `data`; no modo "só registrar", a opinião é
//! gravada e nada é suprimido; desligada, não há consulta; e fora do ar, a
//! triagem não dá opinião — o resumo segue como sempre.

use backend::{
    app::App,
    models::_entities::{alert_events, devices},
    services::ai::{
        decider::config::{AiDeciderSettings, DeciderTriageMode},
        proactive::triage::{self, TRIAGE_FIELD},
        settings,
    },
};
use chrono::Utc;
use loco_rs::{prelude::AppContext, testing::prelude::*};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde_json::json;
use serial_test::serial;

use super::fake_ollama::{self, by_keyword, FakeOllama};

async fn alerta(ctx: &AppContext) -> alert_events::Model {
    let dev = devices::ActiveModel {
        name: Set("Switch do andar".into()),
        r#type: Set("switch".into()),
        status: Set("down".into()),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap();
    alert_events::ActiveModel {
        device_id: Set(Some(dev.id)),
        status: Set("active".into()),
        severity: Set("critical".into()),
        started_at: Set(Utc::now().into()),
        message: Set(Some("Switch do andar fora do ar".into())),
        data: Set(Some(json!({ "origem": "motor" }))),
        created_at: Set(Utc::now().into()),
        updated_at: Set(Utc::now().into()),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap()
}

/// Um Ollama que acha todo alerta um sintoma, nunca acionável.
async fn cetico() -> FakeOllama {
    fake_ollama::start(by_keyword(&[("", "symptom")])).await
}

async fn modo(ctx: &AppContext, fake: &FakeOllama, mode: DeciderTriageMode) {
    let mut decider = fake.settings();
    decider.features.incident_triage = mode;
    settings::save_decider(&ctx.db, decider).await.unwrap();
}

#[tokio::test]
#[serial]
async fn decidir_suprime_o_que_nao_e_acionavel_e_preserva_o_data() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let fake = cetico().await;
        modo(&ctx, &fake, DeciderTriageMode::Enforce).await;
        let evento = alerta(&ctx).await;

        let (opinion, _) = triage::assess(&ctx, evento.id)
            .await
            .unwrap()
            .expect("opinião");

        assert!(opinion.suppressed);
        assert!(opinion.symptom > 90.0 && opinion.actionable < 10.0);
        assert_eq!(opinion.model, "laya:322m-multilingual-bf16");
        let data = alert_events::Entity::find_by_id(evento.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap()
            .data
            .unwrap();
        assert_eq!(data["origem"], "motor", "o resto do data fica");
        assert_eq!(data[TRIAGE_FIELD]["suppressed"], true);
        let state = fake.states.lock().unwrap()[0].clone();
        assert!(state.contains("Switch do andar"), "{state}");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn so_registrar_nunca_suprime() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let fake = cetico().await;
        modo(&ctx, &fake, DeciderTriageMode::Shadow).await;
        let evento = alerta(&ctx).await;

        let (opinion, _) = triage::assess(&ctx, evento.id)
            .await
            .unwrap()
            .expect("opinião");
        assert!(!opinion.suppressed);
        assert_eq!(opinion.mode, DeciderTriageMode::Shadow);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn desligada_nao_consulta_e_fora_do_ar_nao_opina() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let fake = cetico().await;
        modo(&ctx, &fake, DeciderTriageMode::Off).await;
        let evento = alerta(&ctx).await;
        assert!(triage::assess(&ctx, evento.id).await.unwrap().is_none());
        assert_eq!(fake.calls(), 0);

        let mut down = AiDeciderSettings {
            enabled: true,
            base_url: "http://127.0.0.1:9".into(),
            timeout_ms: 300,
            ..AiDeciderSettings::default()
        };
        down.features.incident_triage = DeciderTriageMode::Enforce;
        settings::save_decider(&ctx.db, down).await.unwrap();
        backend::services::ai::decider::runtime::DeciderRuntime::from_context(&ctx).invalidate();
        assert!(
            triage::assess(&ctx, evento.id).await.unwrap().is_none(),
            "sem opinião o resumo segue como sempre"
        );
    })
    .await;
}
