//! Fase 2 do roadmap de ajustes: **escopo de regra é escolha, não herança**.
//!
//! O que se afirma aqui é o recorte que a aba Regras do dispositivo usa. Uma
//! regra global criada de dentro de um equipamento precisa aparecer na tela em
//! que nasceu — sumir dela é indistinguível de a criação ter falhado —, sem
//! que o recorte "só deste dispositivo" deixe de existir para quem o quer.

use backend::app::App;
use loco_rs::testing::prelude::*;
use serde_json::{json, Value};
use serial_test::serial;

use super::prepare_data;

async fn autenticado(request: &mut loco_rs::TestServer, ctx: &loco_rs::app::AppContext) {
    let session = prepare_data::init_user_login(request, ctx).await;
    let (header, value) = prepare_data::auth_header(&session.token);
    request.add_header(header, value);
}

async fn cria_dispositivo(request: &loco_rs::TestServer, nome: &str, ip: &str) -> i64 {
    let resposta = request
        .post("/api/devices")
        .json(&json!({ "name": nome, "type": "router", "ipAddress": ip }))
        .await;
    assert_eq!(resposta.status_code(), 201, "{}", resposta.text());
    serde_json::from_str::<Value>(&resposta.text()).unwrap()["id"]
        .as_i64()
        .expect("id do dispositivo")
}

async fn cria_regra(request: &loco_rs::TestServer, nome: &str, device_id: Option<i64>) -> i64 {
    let resposta = request
        .post("/api/alert-rules")
        .json(&json!({
            "name": nome,
            "deviceId": device_id,
            "condition": { "field": "latencyMs", "operator": "gt", "value": 150 },
            "severity": "warning",
        }))
        .await;
    assert_eq!(resposta.status_code(), 201, "{}", resposta.text());
    serde_json::from_str::<Value>(&resposta.text()).unwrap()["id"]
        .as_i64()
        .expect("id da regra")
}

fn nomes(corpo: &str) -> Vec<String> {
    serde_json::from_str::<Vec<Value>>(corpo)
        .expect("lista de regras")
        .into_iter()
        .map(|regra| regra["name"].as_str().unwrap_or_default().to_string())
        .collect()
}

#[tokio::test]
#[serial]
async fn o_recorte_por_dispositivo_continua_significando_so_aquele_dispositivo() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;
        let alvo = cria_dispositivo(&request, "Roteador alvo", "192.168.80.1").await;
        let outro = cria_dispositivo(&request, "Roteador vizinho", "192.168.80.2").await;
        cria_regra(&request, "Latência do alvo", Some(alvo)).await;
        cria_regra(&request, "Latência do vizinho", Some(outro)).await;
        cria_regra(&request, "Latência de qualquer um", None).await;

        let resposta = request
            .get(&format!("/api/alert-rules?deviceId={alvo}"))
            .await;
        assert_eq!(resposta.status_code(), 200, "{}", resposta.text());
        let listadas = nomes(&resposta.text());
        assert!(listadas.contains(&"Latência do alvo".to_string()));
        assert!(
            !listadas.contains(&"Latência de qualquer um".to_string()),
            "sem `includeGlobal`, o recorte é só o dispositivo — outros consumidores dependem disso"
        );
        assert!(!listadas.contains(&"Latência do vizinho".to_string()));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn com_include_global_a_regra_de_parque_aparece_na_aba_do_dispositivo() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;
        let alvo = cria_dispositivo(&request, "Roteador alvo", "192.168.81.1").await;
        let outro = cria_dispositivo(&request, "Roteador vizinho", "192.168.81.2").await;
        cria_regra(&request, "Latência do alvo", Some(alvo)).await;
        cria_regra(&request, "Latência do vizinho", Some(outro)).await;
        cria_regra(&request, "Latência de qualquer um", None).await;

        let resposta = request
            .get(&format!(
                "/api/alert-rules?deviceId={alvo}&includeGlobal=true"
            ))
            .await;
        assert_eq!(resposta.status_code(), 200, "{}", resposta.text());
        let listadas = nomes(&resposta.text());
        assert!(
            listadas.contains(&"Latência de qualquer um".to_string()),
            "a regra global também é avaliada nas checagens deste equipamento"
        );
        assert!(listadas.contains(&"Latência do alvo".to_string()));
        assert!(
            !listadas.contains(&"Latência do vizinho".to_string()),
            "acrescentar as globais não é o mesmo que devolver o inventário inteiro"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn uma_regra_de_site_nao_e_global() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;
        let alvo = cria_dispositivo(&request, "Roteador alvo", "192.168.82.1").await;

        let site = request
            .post("/api/sites")
            .json(&json!({ "name": "Matriz" }))
            .await;
        assert_eq!(site.status_code(), 201, "{}", site.text());
        let site_id = serde_json::from_str::<Value>(&site.text()).unwrap()["id"]
            .as_i64()
            .expect("id do site");

        let criada = request
            .post("/api/alert-rules")
            .json(&json!({
                "name": "Latência da matriz",
                "siteId": site_id,
                "condition": { "field": "latencyMs", "operator": "gt", "value": 150 },
            }))
            .await;
        assert_eq!(criada.status_code(), 201, "{}", criada.text());

        let resposta = request
            .get(&format!(
                "/api/alert-rules?deviceId={alvo}&includeGlobal=true"
            ))
            .await;
        let listadas = nomes(&resposta.text());
        assert!(
            !listadas.contains(&"Latência da matriz".to_string()),
            "uma regra de site tem escopo; global é a que não tem nenhum"
        );
    })
    .await;
}
