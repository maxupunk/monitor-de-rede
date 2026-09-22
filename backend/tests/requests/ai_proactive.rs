//! IA proativa com um provedor falso: resumo de incidente gravado no alerta e
//! resumo periódico respeitando o agendamento.
//!
//! O driver é injetado pelo `DriverFactory`, então nenhum teste sai para a
//! rede nem depende de chave de API.

use async_trait::async_trait;
use backend::{
    app::App,
    dtos::ai::TestConnectionResponse,
    models::_entities::{alert_events, devices},
    services::{
        ai::{
            drivers::traits::{
                AiChatChunk, AiChatOptions, AiChunkStream, AiDriver, AiMessage, AiTool, AiUsage,
            },
            proactive::{
                config::{AiDigestSchedule, AiProactiveSettings},
                digest, incident,
                runner::DriverFactory,
            },
            settings::{self, AiSettings},
        },
        shared::errors::AppResult,
    },
};
use chrono::{Local, TimeZone, Utc};
use loco_rs::{prelude::AppContext, testing::prelude::*};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde_json::json;
use serial_test::serial;

/// Responde sempre o mesmo texto, sem chamar ferramenta.
struct Resposta(&'static str);

#[async_trait]
impl AiDriver for Resposta {
    fn id(&self) -> &'static str {
        "fake"
    }

    fn display_name(&self) -> &'static str {
        "Fake"
    }

    async fn chat_stream(
        &self,
        _messages: &[AiMessage],
        _tools: &[AiTool],
        _options: AiChatOptions,
    ) -> AppResult<AiChunkStream> {
        let chunks = vec![
            Ok(AiChatChunk {
                text_delta: Some(self.0.to_string()),
                ..AiChatChunk::default()
            }),
            Ok(AiChatChunk {
                usage: Some(AiUsage {
                    prompt_tokens: 900,
                    completion_tokens: 40,
                }),
                ..AiChatChunk::default()
            }),
        ];
        Ok(Box::pin(futures::stream::iter(chunks)))
    }

    async fn test_connection(&self) -> AppResult<TestConnectionResponse> {
        Ok(TestConnectionResponse {
            success: true,
            latency_ms: 0.0,
            message: String::new(),
            model: None,
        })
    }
}

struct Falso(&'static str);

impl DriverFactory for Falso {
    fn create(&self, _settings: &AiSettings) -> AppResult<Box<dyn AiDriver>> {
        Ok(Box::new(Resposta(self.0)))
    }
}

async fn liga(ctx: &AppContext, proactive: AiProactiveSettings) {
    settings::save(
        &ctx.db,
        AiSettings {
            enabled: true,
            proactive,
            ..AiSettings::default()
        },
    )
    .await
    .unwrap();
}

async fn alerta(ctx: &AppContext) -> alert_events::Model {
    let dev = devices::ActiveModel {
        name: Set("Borda".into()),
        r#type: Set("router".into()),
        ip_address: Set(Some("10.0.0.1".into())),
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
        message: Set(Some("Borda fora do ar".into())),
        data: Set(Some(json!({ "silencedUntil": null, "origem": "motor" }))),
        created_at: Set(Utc::now().into()),
        updated_at: Set(Utc::now().into()),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap()
}

#[tokio::test]
#[serial]
async fn resumo_de_incidente_fica_gravado_no_alerta_uma_vez_so() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        liga(
            &ctx,
            AiProactiveSettings {
                incident_summaries: true,
                ..AiProactiveSettings::default()
            },
        )
        .await;
        let evento = alerta(&ctx).await;
        let falso = Falso("Uplink da borda caiu; trocar o SFP.");

        let resumo = incident::summarize_alert(&ctx, &falso, evento.id)
            .await
            .unwrap()
            .expect("resumo gerado");
        assert_eq!(resumo.text, "Uplink da borda caiu; trocar o SFP.");
        assert_eq!(resumo.prompt_tokens, 900);

        let gravado = alert_events::Entity::find_by_id(evento.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        let data = gravado.data.expect("data do alerta");
        assert_eq!(data[incident::SUMMARY_FIELD]["text"], resumo.text);
        assert_eq!(data["origem"], "motor", "o resto do data fica intacto");

        assert!(
            incident::summarize_alert(&ctx, &falso, evento.id)
                .await
                .unwrap()
                .is_none(),
            "não paga duas vezes pelo mesmo alerta"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn resumo_periodico_sai_uma_vez_por_periodo_e_fica_consultavel() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let falso = Falso("Rede estável; 1 incidente resolvido.");
        let agora = Local.with_ymd_and_hms(2026, 9, 21, 9, 0, 0).unwrap();

        liga(&ctx, AiProactiveSettings::default()).await;
        assert!(
            !digest::run_if_due(&ctx, &falso, agora).await.unwrap(),
            "desligado por padrão"
        );
        assert!(digest::latest(&ctx.db).await.unwrap().is_none());

        liga(
            &ctx,
            AiProactiveSettings {
                digest: AiDigestSchedule::Daily,
                digest_hour: 8,
                ..AiProactiveSettings::default()
            },
        )
        .await;
        assert!(digest::run_if_due(&ctx, &falso, agora).await.unwrap());
        assert!(
            !digest::run_if_due(&ctx, &falso, agora + chrono::Duration::hours(3))
                .await
                .unwrap(),
            "mesmo dia: não repete"
        );

        let ultimo = digest::latest(&ctx.db)
            .await
            .unwrap()
            .expect("resumo guardado");
        assert_eq!(ultimo.text, "Rede estável; 1 incidente resolvido.");
        assert_eq!(ultimo.period_hours, 24);
    })
    .await;
}
