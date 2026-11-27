//! "Analisar com a IA" de um console (`POST /api/ai/log-analysis`).
//!
//! O que se afirma aqui: com a IA ligada, o console vai ao provedor sem as
//! credenciais e volta como relatório estruturado; com ela desligada, a
//! resposta diz onde ligar; e a rota recusa console vazio e quem só vê.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use backend::app::App;
use loco_rs::testing::prelude::*;
use netmonitor_assistant::{
    drivers::traits::{AiChatChunk, AiChatOptions, AiChunkStream, AiDriver, AiMessage, AiTool},
    dtos::{LogAnalysisInput, TestConnectionResponse},
    log_analysis::{self, LogFindingKind},
    proactive::runner::DriverFactory,
    settings::{self, AiSettings},
};
use netmonitor_core::roles::Role;
use netmonitor_nucleo::services::shared::errors::AppResult;
use serde_json::{json, Value};
use serial_test::serial;

use super::prepare_data;

/// Responde o JSON dado e guarda o que recebeu.
struct Leitor {
    resposta: &'static str,
    recebido: Arc<Mutex<String>>,
}

#[async_trait]
impl AiDriver for Leitor {
    fn id(&self) -> &'static str {
        "fake"
    }

    fn display_name(&self) -> &'static str {
        "Fake"
    }

    async fn chat_stream(
        &self,
        messages: &[AiMessage],
        _tools: &[AiTool],
        _options: AiChatOptions,
    ) -> AppResult<AiChunkStream> {
        let texto = messages
            .iter()
            .filter_map(|m| m.content.clone())
            .collect::<Vec<_>>()
            .join("\n");
        *self.recebido.lock().unwrap() = texto;
        let chunks = vec![Ok(AiChatChunk {
            text_delta: Some(self.resposta.to_string()),
            ..AiChatChunk::default()
        })];
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

struct Falso {
    resposta: &'static str,
    recebido: Arc<Mutex<String>>,
}

#[async_trait]
impl DriverFactory for Falso {
    async fn create(&self, _settings: &AiSettings) -> AppResult<Box<dyn AiDriver>> {
        Ok(Box::new(Leitor {
            resposta: self.resposta,
            recebido: Arc::clone(&self.recebido),
        }))
    }
}

async fn ia(ctx: &loco_rs::prelude::AppContext, enabled: bool) {
    settings::save(
        &ctx.db,
        AiSettings {
            enabled,
            ..AiSettings::default()
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
#[serial]
async fn console_vai_sem_credenciais_e_volta_estruturado() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        ia(&ctx, true).await;
        let recebido = Arc::new(Mutex::new(String::new()));
        let falso = Falso {
            resposta: r#"{"summary": "O npm falhou.", "findings": [{"kind": "error", "line": 3, "excerpt": "npm ERR! code E401", "message": "Token do registry recusado", "suggestion": "Confira o segredo de build NPM_TOKEN"}]}"#,
            recebido: Arc::clone(&recebido),
        };

        let relatorio = log_analysis::analyze(
            &ctx.db,
            &falso,
            LogAnalysisInput {
                source: "Compilação #7 · api".into(),
                content: "#1 clone https://x-access-token:ghs_segredo123@github.com/o/r\n\
                          NPM_TOKEN=npm_abcdef\n\
                          npm ERR! code E401"
                    .into(),
            },
        )
        .await
        .unwrap();

        let enviado = recebido.lock().unwrap().clone();
        assert!(!enviado.contains("ghs_segredo123"), "{enviado}");
        assert!(!enviado.contains("npm_abcdef"), "{enviado}");
        assert!(enviado.contains("3| npm ERR! code E401"), "{enviado}");
        assert!(relatorio.masked);
        assert!(relatorio.structured);
        assert_eq!(relatorio.source, "Compilação #7 · api");
        assert_eq!(relatorio.findings.len(), 1);
        assert_eq!(relatorio.findings[0].kind, LogFindingKind::Error);
        assert_eq!(relatorio.findings[0].line, Some(3));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn ia_desligada_explica_onde_ligar() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        ia(&ctx, false).await;
        let falso = Falso {
            resposta: "nunca",
            recebido: Arc::new(Mutex::new(String::new())),
        };
        let erro = log_analysis::analyze(
            &ctx.db,
            &falso,
            LogAnalysisInput {
                source: "Console".into(),
                content: "npm ERR! boom".into(),
            },
        )
        .await
        .expect_err("sem IA, sem análise");
        assert!(
            erro.to_string()
                .contains("Configurações › Inteligência artificial"),
            "{erro}"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn rota_recusa_console_vazio_e_quem_so_ve() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        ia(&ctx, true).await;
        let sessao = prepare_data::init_user_login(&request, &ctx).await;
        let (h, v) = prepare_data::auth_header(&sessao.token);
        let vazio = request
            .post("/api/ai/log-analysis")
            .add_header(h, v)
            .json(&json!({ "source": "Compilação #1", "content": "   " }))
            .await;
        assert!(vazio.status_code().is_client_error(), "{}", vazio.text());
        let corpo: Value = vazio.json();
        assert!(corpo.to_string().contains("vazio"), "{corpo}");

        prepare_data::session_with_role(&mut request, &ctx, Role::Viewer).await;
        let visitante = request
            .post("/api/ai/log-analysis")
            .json(&json!({ "source": "Compilação #1", "content": "npm ERR! boom" }))
            .await;
        assert_eq!(visitante.status_code(), 403, "{}", visitante.text());
    })
    .await;
}
