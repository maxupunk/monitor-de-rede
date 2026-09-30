//! Modelo fora da memória: o tempo máximo por pergunta não conta o
//! carregamento.
//!
//! O que se afirma aqui, contra um Ollaya falso que leva mais que o tempo
//! máximo para carregar: o teste da tela espera o carregamento e responde;
//! o chat não espera — segue sem o Laya e deixa o modelo carregando; e dois
//! pedidos ao mesmo tempo carregam o modelo uma vez só.

use std::time::Duration;

use backend::{
    app::App,
    dtos::ai::TestLayaResponse,
    services::ai::{
        harness::tools::ToolGroup,
        laya::{config::AiLayaSettings, runtime::LayaRuntime, tool_routing},
    },
};
use loco_rs::testing::prelude::*;
use serde_json::json;
use serial_test::serial;

use super::{
    fake_ollaya::{self, by_keyword, with_key},
    prepare_data,
};

/// Carregar leva 1,2 s; o tempo máximo por pergunta é 400 ms.
const LOAD: Duration = Duration::from_millis(1_200);

fn settings(base_url: String) -> AiLayaSettings {
    AiLayaSettings {
        enabled: true,
        base_url,
        timeout_ms: 400,
        ..AiLayaSettings::default()
    }
}

#[tokio::test]
#[serial]
async fn teste_da_tela_espera_o_modelo_carregar_e_responde() {
    request_with_config::<App, _, _>(RequestConfig::default(), |request, ctx| async move {
        with_key();
        let ollaya = fake_ollaya::start_cold(by_keyword(&[("", "docker")]), LOAD).await;
        let sessao = prepare_data::init_user_login(&request, &ctx).await;
        let (h, v) = prepare_data::auth_header(&sessao.token);

        let response = request
            .post("/api/ai/laya/test")
            .add_header(h, v)
            .json(&json!({
                "laya": settings(ollaya.base_url.clone()),
                "question": "e os containers?"
            }))
            .await;
        response.assert_status_ok();
        let result: TestLayaResponse = response.json();

        assert!(result.success, "{}", result.message);
        assert_eq!(ollaya.loads(), 1, "carregou uma vez, com prazo próprio");
        assert_eq!(ollaya.calls(), 1);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn chat_nao_espera_o_carregamento_e_deixa_o_modelo_subindo() {
    with_key();
    let ollaya = fake_ollaya::start_cold(by_keyword(&[("", "docker")]), LOAD).await;
    let runtime = LayaRuntime::default();
    let settings = settings(ollaya.base_url.clone());

    let started = std::time::Instant::now();
    let groups = tool_routing::route(
        &runtime,
        &settings,
        "e os containers?",
        &ToolGroup::DEFERRED,
    )
    .await;
    assert!(
        groups.is_empty(),
        "modelo frio: segue com as palavras-chave"
    );
    assert!(started.elapsed() < LOAD, "não esperou o carregamento");
    assert_eq!(ollaya.calls(), 0);

    tokio::time::sleep(LOAD + Duration::from_millis(500)).await;
    assert_eq!(ollaya.loads(), 1, "o carregamento seguiu em segundo plano");
    let groups = tool_routing::route(
        &runtime,
        &settings,
        "e os containers?",
        &ToolGroup::DEFERRED,
    )
    .await;
    assert!(groups.contains(&ToolGroup::Docker), "quente: o Laya decide");
}

#[tokio::test]
#[serial]
async fn pedidos_simultaneos_carregam_uma_vez_so() {
    with_key();
    let ollaya = fake_ollaya::start_cold(by_keyword(&[]), LOAD).await;
    let runtime = LayaRuntime::default();
    let settings = settings(ollaya.base_url.clone());

    let (a, b) = tokio::join!(
        runtime.ensure_loaded(&settings),
        runtime.ensure_loaded(&settings)
    );
    assert!(a.is_ok() && b.is_ok());
    assert_eq!(ollaya.loads(), 1);
}
