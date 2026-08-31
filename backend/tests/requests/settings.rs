//! Preferências globais — e a prova de que cada uma **muda o comportamento**.
//!
//! Um teste que só gravasse e relesse o valor não pegaria a regressão que
//! importa aqui: a tela existia há tempos gravando nada, e gravar sem consumir
//! seria o mesmo engano com uma camada a mais. Por isso cada preferência é
//! verificada no seu ponto de consumo, não no seu ponto de gravação.

use backend::app::App;
use loco_rs::testing::prelude::*;
use serial_test::serial;

use super::prepare_data;

async fn autenticado(request: &mut loco_rs::TestServer, ctx: &loco_rs::app::AppContext) {
    let session = prepare_data::init_user_login(request, ctx).await;
    let (header, value) = prepare_data::auth_header(&session.token);
    request.add_header(header, value);
}

async fn grava(request: &loco_rs::TestServer, corpo: serde_json::Value) -> String {
    let resposta = request.put("/api/settings").json(&corpo).await;
    assert_eq!(resposta.status_code(), 200, "{}", resposta.text());
    resposta.text()
}

#[tokio::test]
#[serial]
async fn sem_nada_gravado_a_api_devolve_os_padroes_antigos() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        let resposta = request.get("/api/settings").await;
        assert_eq!(resposta.status_code(), 200, "{}", resposta.text());
        let corpo: serde_json::Value = serde_json::from_str(&resposta.text()).unwrap();

        assert_eq!(corpo["defaultPingIntervalSeconds"], 60);
        assert_eq!(corpo["defaultSnmpCommunity"], "public");
        assert_eq!(corpo["autoDiscoveryEnabled"], true);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn o_intervalo_padrao_governa_o_monitor_novo_sem_intervalo_proprio() {
    // O ponto de consumo. Sem esta asserção, a preferência seria só um número
    // guardado num JSON.
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        grava(
            &request,
            serde_json::json!({
                "defaultPingIntervalSeconds": 300,
                "defaultSnmpCommunity": "public",
                "autoDiscoveryEnabled": true
            }),
        )
        .await;

        let criado = request
            .post("/api/monitors")
            .json(&serde_json::json!({
                "name": "Ping sem intervalo", "type": "ping",
                "configuration": { "host": "127.0.0.1" }
            }))
            .await;
        assert_eq!(criado.status_code(), 201, "{}", criado.text());
        let corpo: serde_json::Value = serde_json::from_str(&criado.text()).unwrap();
        assert_eq!(
            corpo["intervalSeconds"], 300,
            "a preferência não foi aplicada"
        );

        // E o intervalo declarado continua vencendo a preferência.
        let explicito = request
            .post("/api/monitors")
            .json(&serde_json::json!({
                "name": "Ping com intervalo", "type": "ping", "intervalSeconds": 30,
                "configuration": { "host": "127.0.0.1" }
            }))
            .await;
        let corpo: serde_json::Value = serde_json::from_str(&explicito.text()).unwrap();
        assert_eq!(corpo["intervalSeconds"], 30);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn a_comunidade_padrao_e_gravada_no_dispositivo_com_snmp_ligado() {
    // Gravada no cadastro, e não aplicada só na coleta: assim ela fica visível,
    // e trocar a preferência depois não repassa em silêncio a comunidade de
    // equipamentos que já estavam funcionando.
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        grava(
            &request,
            serde_json::json!({
                "defaultPingIntervalSeconds": 60,
                "defaultSnmpCommunity": "comunidade-da-casa",
                "autoDiscoveryEnabled": true
            }),
        )
        .await;

        let com_snmp = request
            .post("/api/devices")
            .json(&serde_json::json!({
                "name": "switch", "type": "switch", "snmpEnabled": true
            }))
            .await;
        assert_eq!(com_snmp.status_code(), 201, "{}", com_snmp.text());
        let corpo: serde_json::Value = serde_json::from_str(&com_snmp.text()).unwrap();
        assert_eq!(corpo["snmpCommunity"], "comunidade-da-casa");

        // Sem SNMP, nada é inventado: a coluna continua nula.
        let sem_snmp = request
            .post("/api/devices")
            .json(&serde_json::json!({ "name": "host", "type": "server" }))
            .await;
        let corpo: serde_json::Value = serde_json::from_str(&sem_snmp.text()).unwrap();
        assert!(corpo["snmpCommunity"].is_null());

        // E a comunidade informada vence a preferência.
        let propria = request
            .post("/api/devices")
            .json(&serde_json::json!({
                "name": "outro", "type": "switch", "snmpEnabled": true,
                "snmpCommunity": "propria"
            }))
            .await;
        let corpo: serde_json::Value = serde_json::from_str(&propria.text()).unwrap();
        assert_eq!(corpo["snmpCommunity"], "propria");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn a_trava_de_descoberta_impede_o_enfileiramento_sem_apagar_a_rede() {
    use backend::services::discovery::queue::schedule_due_networks;

    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        let site = request
            .post("/api/sites")
            .json(&serde_json::json!({ "name": "Matriz" }))
            .await;
        let site: serde_json::Value = serde_json::from_str(&site.text()).unwrap();
        let rede = request
            .post("/api/networks")
            .json(&serde_json::json!({
                "name": "LAN", "cidr": "192.168.50.0/30", "scanEnabled": true,
                "siteId": site["id"].as_i64().unwrap()
            }))
            .await;
        assert_eq!(rede.status_code(), 201, "{}", rede.text());

        grava(
            &request,
            serde_json::json!({
                "defaultPingIntervalSeconds": 60,
                "defaultSnmpCommunity": "public",
                "autoDiscoveryEnabled": false
            }),
        )
        .await;

        let enfileiradas = schedule_due_networks(&ctx.db).await.expect("agendar");
        assert_eq!(enfileiradas, 0, "a trava global não segurou o agendamento");

        // Religar devolve o parque ao que era, sem reconfigurar rede por rede:
        // o `scan_enabled` de cada uma continuou intacto.
        grava(
            &request,
            serde_json::json!({
                "defaultPingIntervalSeconds": 60,
                "defaultSnmpCommunity": "public",
                "autoDiscoveryEnabled": true
            }),
        )
        .await;
        let enfileiradas = schedule_due_networks(&ctx.db).await.expect("agendar");
        assert_eq!(enfileiradas, 1, "religar precisava voltar a enfileirar");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn valores_invalidos_sao_recusados_com_mensagem_em_portugues() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        let curto = request
            .put("/api/settings")
            .json(&serde_json::json!({
                "defaultPingIntervalSeconds": 1,
                "defaultSnmpCommunity": "public",
                "autoDiscoveryEnabled": true
            }))
            .await;
        assert_eq!(curto.status_code(), 422, "{}", curto.text());
        assert!(
            curto.text().contains("intervalo padrão"),
            "{}",
            curto.text()
        );

        let vazia = request
            .put("/api/settings")
            .json(&serde_json::json!({
                "defaultPingIntervalSeconds": 60,
                "defaultSnmpCommunity": "   ",
                "autoDiscoveryEnabled": true
            }))
            .await;
        assert_eq!(vazia.status_code(), 422, "{}", vazia.text());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn a_rota_exige_sessao() {
    request_with_config::<App, _, _>(RequestConfig::default(), |request, _ctx| async move {
        assert_eq!(request.get("/api/settings").await.status_code(), 401);
        assert_eq!(
            request.get("/api/settings/onboarding").await.status_code(),
            401
        );
        assert_eq!(
            request
                .post("/api/settings/onboarding/complete")
                .await
                .status_code(),
            401
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn database_size_retorna_tamanho_e_tipo_do_banco() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        let resposta = request.get("/api/settings/database-size").await;
        assert_eq!(resposta.status_code(), 200, "{}", resposta.text());
        let corpo: serde_json::Value = serde_json::from_str(&resposta.text()).unwrap();

        assert!(corpo["sizeBytes"].as_i64().is_some_and(|v| v > 0));
        assert_eq!(corpo["dbType"], "sqlite");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn onboarding_status_e_conclusao_funcionam_corretamente() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        let status_resp = request.get("/api/settings/onboarding").await;
        assert_eq!(status_resp.status_code(), 200, "{}", status_resp.text());
        let status_json: serde_json::Value = serde_json::from_str(&status_resp.text()).unwrap();

        assert_eq!(status_json["completed"], false);
        assert_eq!(status_json["needsOnboarding"], true);

        // Marca como concluído
        let complete_resp = request.post("/api/settings/onboarding/complete").await;
        assert_eq!(complete_resp.status_code(), 200, "{}", complete_resp.text());

        let status_depois = request.get("/api/settings/onboarding").await;
        assert_eq!(status_depois.status_code(), 200, "{}", status_depois.text());
        let depois_json: serde_json::Value = serde_json::from_str(&status_depois.text()).unwrap();

        assert_eq!(depois_json["completed"], true);
        assert_eq!(depois_json["needsOnboarding"], false);
        assert!(depois_json["completedAt"].is_string());
    })
    .await;
}
