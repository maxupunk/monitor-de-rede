//! `GET`/`PUT /api/server-addresses` — a lista "por onde os equipamentos
//! alcançam este servidor".
//!
//! O que só aparece aqui é o ciclo completo: detectado → corrigido → de volta
//! ao detectado. Um teste unitário do serviço não exercita a serialização nem o
//! formato que a tela lê.

use backend::app::App;
use loco_rs::testing::prelude::*;
use serial_test::serial;

use super::prepare_data;

async fn autenticado(request: &mut loco_rs::TestServer, ctx: &loco_rs::app::AppContext) {
    let session = prepare_data::init_user_login(request, ctx).await;
    let (header, value) = prepare_data::auth_header(&session.token);
    request.add_header(header, value);
}

#[tokio::test]
#[serial]
async fn a_lista_nasce_com_os_tres_tipos_e_o_motivo_de_cada_um() {
    // Tipo sem endereço continua na lista: escondê-lo faria o operador não
    // descobrir que aquela situação existe.
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        let resposta = request.get("/api/server-addresses").await;
        assert_eq!(resposta.status_code(), 200, "{}", resposta.text());
        let corpo: serde_json::Value = serde_json::from_str(&resposta.text()).unwrap();

        let dados = corpo["data"].as_array().expect("lista");
        assert_eq!(dados.len(), 3);
        let tipos: Vec<&str> = dados
            .iter()
            .map(|item| item["kind"].as_str().unwrap())
            .collect();
        assert_eq!(tipos, vec!["lan", "vpn", "public"]);

        for item in dados {
            assert!(
                !item["description"].as_str().unwrap_or_default().is_empty(),
                "toda entrada precisa dizer quando usá-la: {item}"
            );
            assert!(
                !item["source"].as_str().unwrap_or_default().is_empty(),
                "e de onde veio (ou por que não veio): {item}"
            );
        }
    })
    .await;
}

#[tokio::test]
#[serial]
async fn a_correcao_e_gravada_e_pode_ser_desfeita_sem_apagar_o_detectado() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        let gravou = request
            .put("/api/server-addresses")
            .json(&serde_json::json!({
                "overrides": { "lan": "192.168.1.10" },
                "custom": [],
                "preferredId": "lan"
            }))
            .await;
        assert_eq!(gravou.status_code(), 200, "{}", gravou.text());
        let corpo: serde_json::Value = serde_json::from_str(&gravou.text()).unwrap();
        assert_eq!(corpo["data"][0]["value"], "192.168.1.10");
        assert_eq!(corpo["data"][0]["overridden"], true);
        assert_eq!(corpo["data"][0]["source"], "corrigido por você");
        assert_eq!(corpo["preferredId"], "lan");

        // Correção em branco significa "voltar ao detectado", não gravar vazio.
        let desfez = request
            .put("/api/server-addresses")
            .json(&serde_json::json!({ "overrides": { "lan": "" }, "custom": [] }))
            .await;
        assert_eq!(desfez.status_code(), 200, "{}", desfez.text());
        let corpo: serde_json::Value = serde_json::from_str(&desfez.text()).unwrap();
        assert_eq!(corpo["data"][0]["overridden"], false);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn o_endereco_que_aponta_para_o_proprio_equipamento_e_recusado() {
    // É o defeito que originou o recurso: `localhost` gravado num roteador o faz
    // mandar o log para si mesmo, sem erro em lugar nenhum.
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        for ruim in ["localhost", "127.0.0.1", "::1"] {
            let resposta = request
                .put("/api/server-addresses")
                .json(&serde_json::json!({ "overrides": { "lan": ruim }, "custom": [] }))
                .await;
            assert_eq!(
                resposta.status_code(),
                422,
                "aceitou {ruim}: {}",
                resposta.text()
            );
            assert!(
                resposta
                    .text()
                    .contains("aponta o equipamento para ele mesmo"),
                "{}",
                resposta.text()
            );
        }
    })
    .await;
}

#[tokio::test]
#[serial]
async fn o_personalizado_ganha_id_do_servidor_e_exige_nome() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        let sem_nome = request
            .put("/api/server-addresses")
            .json(&serde_json::json!({
                "overrides": {},
                "custom": [{ "label": "  ", "value": "172.20.0.5" }]
            }))
            .await;
        assert_eq!(sem_nome.status_code(), 422, "{}", sem_nome.text());

        let ok = request
            .put("/api/server-addresses")
            .json(&serde_json::json!({
                "overrides": {},
                "custom": [{ "label": "Filial Norte", "value": "172.20.0.5" }]
            }))
            .await;
        assert_eq!(ok.status_code(), 200, "{}", ok.text());
        let corpo: serde_json::Value = serde_json::from_str(&ok.text()).unwrap();
        let dados = corpo["data"].as_array().unwrap();
        assert_eq!(dados.len(), 4);
        assert_eq!(dados[3]["label"], "Filial Norte");
        assert_eq!(dados[3]["kind"], "custom");
        // A tela precisa do id sorteado para editar depois sem duplicar.
        assert!(
            dados[3]["id"].as_str().unwrap().starts_with("custom:"),
            "{}",
            dados[3]
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn a_rota_exige_sessao() {
    request_with_config::<App, _, _>(RequestConfig::default(), |request, _ctx| async move {
        assert_eq!(
            request.get("/api/server-addresses").await.status_code(),
            401
        );
    })
    .await;
}
