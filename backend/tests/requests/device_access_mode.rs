//! A forma de acesso do dispositivo — declarada, deduzida e **consumida**.
//!
//! O risco aqui é o mesmo das preferências globais: um campo que grava, relê e
//! não muda nada é indistinguível de um campo quebrado. Por isso os testes que
//! importam não são os de ida e volta, e sim os que verificam que a declaração
//! escolhe o endereço oferecido na ativação de log.

use backend::app::App;
use loco_rs::testing::prelude::*;
use serde_json::Value;
use serial_test::serial;

use super::prepare_data;

async fn autenticado(request: &mut loco_rs::TestServer, ctx: &loco_rs::app::AppContext) {
    let session = prepare_data::init_user_login(request, ctx).await;
    let (header, value) = prepare_data::auth_header(&session.token);
    request.add_header(header, value);
}

async fn cria_dispositivo(request: &loco_rs::TestServer, corpo: Value) -> Value {
    let resposta = request.post("/api/devices").json(&corpo).await;
    assert_eq!(resposta.status_code(), 201, "{}", resposta.text());
    serde_json::from_str(&resposta.text()).expect("json do dispositivo")
}

#[tokio::test]
#[serial]
async fn sem_declaracao_o_sistema_deduz_e_diz_que_deduziu() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        let corpo = cria_dispositivo(
            &request,
            serde_json::json!({
                "name": "roteador", "type": "router", "ipAddress": "192.168.77.1"
            }),
        )
        .await;

        // O campo declarado continua nulo: gravar a dedução ali a congelaria.
        assert!(
            corpo["accessMode"].is_null(),
            "a dedução não pode virar declaração"
        );
        assert_eq!(corpo["effectiveAccessMode"], "local");
        assert_eq!(corpo["accessModeDeclared"], false);
        assert!(
            corpo["accessModeReason"]
                .as_str()
                .is_some_and(|texto| !texto.trim().is_empty()),
            "sem motivo a tela apresentaria um palpite como certeza"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn o_ip_publico_e_deduzido_como_acesso_remoto() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        let corpo = cria_dispositivo(
            &request,
            serde_json::json!({
                "name": "filial", "type": "router", "ipAddress": "200.150.10.1"
            }),
        )
        .await;
        assert_eq!(corpo["effectiveAccessMode"], "remote");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn a_declaracao_e_gravada_e_pode_voltar_para_o_automatico() {
    // O `auto` explícito existe para isto: a tela manda o formulário inteiro, e
    // sem a palavra não haveria como distinguir "voltei ao automático" de "não
    // mexi neste campo" — a declaração ficaria presa para sempre.
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        let corpo = cria_dispositivo(
            &request,
            serde_json::json!({
                "name": "filial", "type": "router",
                "ipAddress": "192.168.90.1", "accessMode": "remote"
            }),
        )
        .await;
        assert_eq!(corpo["accessMode"], "remote");
        assert_eq!(corpo["effectiveAccessMode"], "remote");
        assert_eq!(corpo["accessModeDeclared"], true);

        let id = corpo["id"].as_i64().expect("id");
        let resposta = request
            .put(&format!("/api/devices/{id}"))
            .json(&serde_json::json!({ "accessMode": "auto" }))
            .await;
        assert_eq!(resposta.status_code(), 200, "{}", resposta.text());
        let corpo: Value = serde_json::from_str(&resposta.text()).expect("json");
        assert!(corpo["accessMode"].is_null(), "não voltou ao automático");
        // E a dedução reassume: IP privado, rede local.
        assert_eq!(corpo["effectiveAccessMode"], "local");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn campo_ausente_preserva_a_declaracao_existente() {
    // Outros consumidores da API mandam o `DeviceInput` sem este campo; ausência
    // significa "não mexi", e não "apague".
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        let corpo = cria_dispositivo(
            &request,
            serde_json::json!({
                "name": "filial", "type": "router",
                "ipAddress": "192.168.90.1", "accessMode": "vpn"
            }),
        )
        .await;
        let id = corpo["id"].as_i64().expect("id");

        let resposta = request
            .put(&format!("/api/devices/{id}"))
            .json(&serde_json::json!({ "name": "filial renomeada" }))
            .await;
        let corpo: Value = serde_json::from_str(&resposta.text()).expect("json");
        assert_eq!(corpo["accessMode"], "vpn");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn valor_fora_do_vocabulario_e_recusado_com_a_lista_do_que_vale() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        let resposta = request
            .post("/api/devices")
            .json(&serde_json::json!({
                "name": "x", "type": "router", "accessMode": "nuvem"
            }))
            .await;
        assert_eq!(resposta.status_code(), 422, "{}", resposta.text());
        let texto = resposta.text();
        for aceito in ["auto", "local", "vpn", "remote"] {
            assert!(
                texto.contains(aceito),
                "a mensagem não cita {aceito}: {texto}"
            );
        }
    })
    .await;
}

#[tokio::test]
#[serial]
async fn a_declaracao_escolhe_o_endereco_oferecido_na_ativacao_de_log() {
    // O ponto de consumo. Sem esta asserção o campo seria decorativo: o
    // operador declararia "acesso remoto" e a tela de log continuaria oferecendo
    // o IP da LAN, que o equipamento não alcança.
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        request
            .put("/api/server-addresses")
            .json(&serde_json::json!({
                "overrides": { "lan": "192.168.1.10", "public": "casa.ddns.net" },
                "custom": [], "preferredId": null
            }))
            .await;

        let corpo = cria_dispositivo(
            &request,
            serde_json::json!({
                "name": "filial", "type": "router",
                // IP privado: a dedução diria "rede local", e erraria.
                "ipAddress": "192.168.90.1", "accessMode": "remote"
            }),
        )
        .await;
        let id = corpo["id"].as_i64().expect("id");

        let resposta = request
            .get(&format!("/api/logs/devices/{id}/provision-hints"))
            .await;
        assert_eq!(resposta.status_code(), 200, "{}", resposta.text());
        let dicas: Value = serde_json::from_str(&resposta.text()).expect("json");

        assert_eq!(dicas["accessMode"], "remote");
        assert_eq!(dicas["accessModeDeclared"], true);
        assert_eq!(dicas["suggestedAddressId"], "public");
        assert_eq!(dicas["serverAddress"], "casa.ddns.net");
        assert!(
            dicas["suggestedAddressReason"]
                .as_str()
                .is_some_and(|texto| texto.contains("cadastro")),
            "o motivo precisa dizer que veio do cadastro: {}",
            dicas["suggestedAddressReason"]
        );
    })
    .await;
}
