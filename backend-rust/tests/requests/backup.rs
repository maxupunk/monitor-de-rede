//! Backup e restauração das configurações.

use backend_rust::{app::App, models::_entities::monitor_results};
use chrono::Utc;
use loco_rs::{testing::prelude::*, TestServer};
use sea_orm::{ActiveModelTrait, EntityTrait, PaginatorTrait, Set};
use serial_test::serial;

use super::prepare_data;

/// Cria um site, uma rede, um dispositivo e um monitor, e devolve o id do
/// dispositivo — o suficiente para exercitar o grafo de FKs no arquivo.
async fn semear(request: &TestServer) -> (i64, i64) {
    let site = request
        .post("/api/sites")
        .json(&serde_json::json!({ "name": "Matriz" }))
        .await;
    assert_eq!(site.status_code(), 201, "{}", site.text());
    let site: serde_json::Value = serde_json::from_str(&site.text()).unwrap();
    let site_id = site["id"].as_i64().unwrap();

    let device = request
        .post("/api/devices")
        .json(&serde_json::json!({
            "name": "rt-core", "type": "router", "ipAddress": "10.0.0.1", "siteId": site_id
        }))
        .await;
    assert_eq!(device.status_code(), 201, "{}", device.text());
    let device: serde_json::Value = serde_json::from_str(&device.text()).unwrap();
    let device_id = device["id"].as_i64().unwrap();

    let monitor = request
        .post("/api/monitors")
        .json(&serde_json::json!({
            "name": "Ping rt-core", "type": "ping", "deviceId": device_id,
            "configuration": { "host": "10.0.0.1" }
        }))
        .await;
    assert_eq!(monitor.status_code(), 201, "{}", monitor.text());

    (site_id, device_id)
}

/// O ciclo completo: exportar, destruir a configuração e restaurar.
///
/// O que o teste guarda é a promessa que dá sentido ao recurso — os **ids
/// voltam iguais**. Se a restauração renumerasse, todo link de topologia,
/// regra de alerta e monitor apontaria para o equipamento errado, e nada disso
/// apareceria como erro: apareceria como gráfico trocado.
#[tokio::test]
#[serial]
async fn exportar_e_restaurar_devolve_a_configuracao_com_os_mesmos_ids() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let (site_id, device_id) = semear(&request).await;

        let exported = request.get("/api/backup/export").await;
        assert_eq!(exported.status_code(), 200, "{}", exported.text());
        assert!(exported
            .headers()
            .get("content-disposition")
            .unwrap()
            .to_str()
            .unwrap()
            .contains("netmonitor-backup-"));
        let arquivo: serde_json::Value = serde_json::from_str(&exported.text()).unwrap();
        assert_eq!(arquivo["formatVersion"], 1);
        assert_eq!(arquivo["tables"]["sites"].as_array().unwrap().len(), 1);
        assert_eq!(arquivo["tables"]["devices"].as_array().unwrap().len(), 1);
        // Telemetria e contas de acesso não entram no arquivo.
        assert!(arquivo["tables"].get("metrics").is_none());
        assert!(arquivo["tables"].get("users").is_none());

        // A prévia lê o arquivo sem escrever nada.
        let preview = request.post("/api/backup/preview").json(&arquivo).await;
        assert_eq!(preview.status_code(), 200, "{}", preview.text());
        let preview: serde_json::Value = serde_json::from_str(&preview.text()).unwrap();
        assert!(preview["totalRows"].as_u64().unwrap() >= 3);

        // O operador apaga o site — e leva junto rede, dispositivo e monitor.
        assert_eq!(
            request
                .delete(&format!("/api/sites/{site_id}"))
                .await
                .status_code(),
            204
        );
        let vazio = request.get("/api/devices").await;
        let vazio: serde_json::Value = serde_json::from_str(&vazio.text()).unwrap();
        assert!(vazio.as_array().unwrap().is_empty());

        let restored = request.post("/api/backup/restore").json(&arquivo).await;
        assert_eq!(restored.status_code(), 200, "{}", restored.text());

        // O dispositivo voltou com o mesmo id, e o vínculo com o site também.
        let device = request.get(&format!("/api/devices/{device_id}")).await;
        assert_eq!(device.status_code(), 200, "{}", device.text());
        let device: serde_json::Value = serde_json::from_str(&device.text()).unwrap();
        assert_eq!(device["name"], "rt-core");
        assert_eq!(device["siteId"].as_i64(), Some(site_id));

        // O monitor voltou apontando para o dispositivo certo.
        let monitors = request.get("/api/monitors").await;
        let monitors: serde_json::Value = serde_json::from_str(&monitors.text()).unwrap();
        let lista = monitors.as_array().unwrap();
        assert_eq!(lista.len(), 1);
        assert_eq!(lista[0]["deviceId"].as_i64(), Some(device_id));

        // Cadastrar depois da restauração não pode colidir com um id restaurado
        // — é o que a sequência realinhada garante no PostgreSQL.
        let novo = request
            .post("/api/sites")
            .json(&serde_json::json!({ "name": "Filial" }))
            .await;
        assert_eq!(novo.status_code(), 201, "{}", novo.text());
    })
    .await;
}

/// A restauração troca a configuração, mas não expulsa quem está logado.
///
/// `users` fica de fora do arquivo justamente para isto: restaurar um backup de
/// outra instalação não pode trocar as credenciais de quem está com a tela
/// aberta.
#[tokio::test]
#[serial]
async fn restaurar_nao_derruba_a_sessao_nem_apaga_usuarios() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let exported = request.get("/api/backup/export").await;
        let arquivo: serde_json::Value = serde_json::from_str(&exported.text()).unwrap();
        assert_eq!(
            request
                .post("/api/backup/restore")
                .json(&arquivo)
                .await
                .status_code(),
            200
        );

        // Mesmo token, mesma sessão.
        assert_eq!(request.get("/api/sites").await.status_code(), 200);
        let usuarios = backend_rust::models::users::Entity::find()
            .count(&ctx.db)
            .await
            .unwrap();
        assert!(usuarios >= 1, "a restauração apagou os usuários");
    })
    .await;
}

/// Histórico pendurado em dispositivo restaurado é apagado, não reaproveitado.
///
/// Sem isso, um `monitor_results` antigo continuaria colado ao id do monitor —
/// que depois da restauração pode ser de outro equipamento. O gráfico mostraria
/// medição do vizinho sem nenhum sinal de erro.
#[tokio::test]
#[serial]
async fn restaurar_limpa_o_historico_dependente() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        semear(&request).await;
        let monitors = request.get("/api/monitors").await;
        let monitors: serde_json::Value = serde_json::from_str(&monitors.text()).unwrap();
        let monitor_id = monitors[0]["id"].as_i64().unwrap();

        monitor_results::ActiveModel {
            monitor_id: Set(monitor_id),
            status: Set("up".into()),
            started_at: Set(Utc::now().into()),
            finished_at: Set(Utc::now().into()),
            duration_ms: Set(12),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("gravar resultado");

        let exported = request.get("/api/backup/export").await;
        let arquivo: serde_json::Value = serde_json::from_str(&exported.text()).unwrap();
        assert_eq!(
            request
                .post("/api/backup/restore")
                .json(&arquivo)
                .await
                .status_code(),
            200
        );

        let restantes = monitor_results::Entity::find()
            .count(&ctx.db)
            .await
            .unwrap();
        assert_eq!(restantes, 0, "o histórico sobreviveu à restauração");
    })
    .await;
}

/// Um arquivo de versão desconhecida é recusado **antes** de apagar qualquer
/// coisa — restaurar é destrutivo, e falhar no meio é o pior desfecho.
#[tokio::test]
#[serial]
async fn arquivo_de_versao_desconhecida_e_recusado_sem_tocar_no_banco() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        semear(&request).await;

        let recusado = request
            .post("/api/backup/restore")
            .json(&serde_json::json!({
                "formatVersion": 99, "appVersion": "futuro",
                "generatedAt": "2030-01-01T00:00:00Z", "tables": {}
            }))
            .await;
        assert_eq!(recusado.status_code(), 422, "{}", recusado.text());

        let devices = request.get("/api/devices").await;
        let devices: serde_json::Value = serde_json::from_str(&devices.text()).unwrap();
        assert_eq!(devices.as_array().unwrap().len(), 1);
    })
    .await;
}

/// Sem JWT não se exporta nem se restaura — o arquivo carrega `token_hash` de
/// probe, community SNMP e as chaves cifradas da VPN.
#[tokio::test]
#[serial]
async fn backup_exige_autenticacao() {
    request_with_config::<App, _, _>(RequestConfig::default(), |request, _ctx| async move {
        assert_eq!(request.get("/api/backup/export").await.status_code(), 401);
        assert_eq!(
            request
                .post("/api/backup/restore")
                .json(&serde_json::json!({}))
                .await
                .status_code(),
            401
        );
    })
    .await;
}
