//! O catálogo de sistemas — **um só**, e usado onde importa.
//!
//! O risco que estes testes cobrem não é o de gravar errado: é o de o catálogo
//! virar um quarto vocabulário paralelo aos três que ele veio substituir. Por
//! isso as asserções são sobre o que muda de comportamento — o sistema
//! declarado no cadastro decide os comandos que a ativação de log vai enviar.

use backend::{
    app::App,
    models::{discovery_results, discovery_runs, networks},
};
use chrono::Utc;
use loco_rs::testing::prelude::*;
use sea_orm::{ActiveModelTrait, Set};
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
async fn o_catalogo_e_servido_e_nao_colide_com_a_rota_de_id() {
    // `/devices/systems` convive com `/devices/{id}`: o `matchit` do axum
    // prioriza o segmento literal. Se um dia deixar de priorizar, é aqui que
    // aparece — e não numa tela em branco.
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        let resposta = request.get("/api/devices/systems").await;
        assert_eq!(resposta.status_code(), 200, "{}", resposta.text());
        let lista: Vec<Value> = serde_json::from_str(&resposta.text()).expect("json");

        let ids: Vec<&str> = lista
            .iter()
            .filter_map(|item| item["id"].as_str())
            .collect();
        assert_eq!(
            ids,
            vec!["routeros", "openwrt", "ubiquiti", "linux", "windows", "mobile", "other"],
            "a ordem do catálogo é a ordem em que as telas listam"
        );

        // As capacidades são o que separa este catálogo de uma lista de nomes:
        // é por elas que a tela sabe o que pode oferecer.
        let routeros = &lista[0];
        assert_eq!(routeros["supportsSyslog"], true);
        assert_eq!(routeros["supportsMacTelnet"], true);
        assert_eq!(routeros["vpnProfile"], "mikrotik");

        let outro = lista.last().expect("other");
        assert_eq!(outro["id"], "other");
        assert_eq!(outro["supportsSyslog"], false);
        assert!(outro["vpnProfile"].is_null());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn identificar_sem_evidencia_ao_vivo_recai_no_cadastro() {
    // `127.0.0.1` só, conforme as diretrizes: nada sai da máquina. **Sem
    // asserção sobre `probed`** — a máquina de quem roda o teste pode ter um
    // servidor SSH ouvindo no loopback, e a sonda o encontraria. O que se
    // afirma aqui é o que não depende do ambiente: nenhum servidor SSH comum
    // (OpenSSH) está no catálogo de assinaturas, então a conclusão precisa
    // continuar vindo do cadastro. Quem cobre o `probed` é o teste sem IP.
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        let resposta = request
            .post("/api/devices/identify")
            .json(&serde_json::json!({
                "ipAddress": "127.0.0.1", "snmpEnabled": false, "vendor": "MikroTik"
            }))
            .await;
        assert_eq!(resposta.status_code(), 200, "{}", resposta.text());
        let corpo: Value = serde_json::from_str(&resposta.text()).expect("json");

        assert_eq!(corpo["operatingSystem"], "routeros");
        assert_eq!(corpo["source"], "cadastro");
        assert_eq!(corpo["accessMode"], "local");
        assert!(
            corpo["accessModeReason"]
                .as_str()
                .is_some_and(|texto| texto.contains("privada") || texto.contains("rede")),
            "a dedução de acesso precisa vir explicada: {}",
            corpo["accessModeReason"]
        );
        assert!(
            corpo["reason"]
                .as_str()
                .is_some_and(|texto| texto.contains("MikroTik")),
            "o motivo precisa citar a evidência: {}",
            corpo["reason"]
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn identificar_sem_ip_cai_no_cadastro_em_vez_de_falhar() {
    // O botão fica desabilitado sem IP, mas a rota não pode depender disso: um
    // 500 aqui viraria erro na tela por um caso que tem resposta boa.
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        let resposta = request
            .post("/api/devices/identify")
            .json(&serde_json::json!({ "model": "OpenWrt One" }))
            .await;
        assert_eq!(resposta.status_code(), 200, "{}", resposta.text());
        let corpo: Value = serde_json::from_str(&resposta.text()).expect("json");
        assert_eq!(corpo["operatingSystem"], "openwrt");
        assert_eq!(corpo["probed"], false);
        assert!(corpo["sysDescr"].is_null());
        assert!(corpo["sshBanner"].is_null());
        assert!(corpo["suggestedModel"].is_null());
        assert!(corpo["suggestedVendor"].is_null());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn identificar_reaproveita_fabricante_e_modelo_da_descoberta() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        let rede = networks::ActiveModel {
            name: Set("Loopback de teste".into()),
            cidr: Set("127.0.0.0/8".into()),
            scan_enabled: Set(false),
            scan_interval: Set(3_600),
            active: Set(true),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("criar rede");
        let agora = Utc::now();
        let run = discovery_runs::ActiveModel {
            network_id: Set(rede.id),
            status: Set("completed".into()),
            started_at: Set(agora.into()),
            finished_at: Set(Some(agora.into())),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("criar descoberta");
        discovery_results::ActiveModel {
            discovery_run_id: Set(run.id),
            ip_address: Set("127.0.0.1".into()),
            hostname: Set(Some("bpi-r3-assistencia".into())),
            vendor: Set(Some("OpenWrt Foundation".into())),
            confidence: Set(95),
            data: Set(Some(serde_json::json!({
                "details": { "snmp": { "model": "OpenWrt One" } }
            }))),
            first_seen_at: Set(agora.into()),
            last_seen_at: Set(agora.into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("criar resultado");

        let resposta = request
            .post("/api/devices/identify")
            .json(&serde_json::json!({ "ipAddress": "127.0.0.1" }))
            .await;
        assert_eq!(resposta.status_code(), 200, "{}", resposta.text());
        let corpo: Value = serde_json::from_str(&resposta.text()).expect("json");

        assert_eq!(corpo["suggestedVendor"], "OpenWrt Foundation");
        assert_eq!(corpo["suggestedModel"], "OpenWrt One");
        assert_eq!(corpo["suggestedName"], "bpi-r3-assistencia");
        assert_eq!(corpo["operatingSystem"], "openwrt");
        assert_eq!(corpo["accessMode"], "local");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn a_aba_de_logs_existe_sem_declaracao_manual_de_sistema() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        let device = cria_dispositivo(
            &request,
            serde_json::json!({
                "name": "openwrt-auto", "type": "router",
                "ipAddress": "192.168.77.16", "vendor": "OpenWrt"
            }),
        )
        .await;
        assert!(
            device["operatingSystem"].is_null(),
            "a detecção deve continuar automática"
        );

        let id = device["id"].as_i64().expect("id");
        let response = request
            .get(&format!("/api/devices/{id}/capabilities"))
            .await;
        assert_eq!(response.status_code(), 200, "{}", response.text());
        let capabilities: Value = serde_json::from_str(&response.text()).expect("json");
        assert_eq!(capabilities["logs"], true);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn sem_declaracao_o_sistema_sai_do_texto_livre_do_cadastro() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        let corpo = cria_dispositivo(
            &request,
            serde_json::json!({
                "name": "borda", "type": "router",
                "ipAddress": "192.168.77.1", "vendor": "MikroTik"
            }),
        )
        .await;

        assert!(
            corpo["operatingSystem"].is_null(),
            "dedução não é declaração"
        );
        assert_eq!(corpo["effectiveOperatingSystem"], "routeros");
        assert_eq!(corpo["operatingSystemSource"], "cadastro");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn a_declaracao_e_gravada_e_pode_voltar_para_o_automatico() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        let corpo = cria_dispositivo(
            &request,
            serde_json::json!({
                "name": "servidor", "type": "server",
                "ipAddress": "192.168.77.2", "vendor": "MikroTik",
                "operatingSystem": "linux"
            }),
        )
        .await;
        assert_eq!(corpo["operatingSystem"], "linux");
        assert_eq!(corpo["effectiveOperatingSystem"], "linux");
        assert_eq!(corpo["operatingSystemSource"], "declarado");

        let id = corpo["id"].as_i64().expect("id");
        let resposta = request
            .put(&format!("/api/devices/{id}"))
            .json(&serde_json::json!({ "operatingSystem": "auto" }))
            .await;
        assert_eq!(resposta.status_code(), 200, "{}", resposta.text());
        let corpo: Value = serde_json::from_str(&resposta.text()).expect("json");
        assert!(
            corpo["operatingSystem"].is_null(),
            "não voltou ao automático"
        );
        // E a dedução reassume o campo de texto livre.
        assert_eq!(corpo["effectiveOperatingSystem"], "routeros");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn sistema_fora_do_catalogo_e_recusado_com_a_lista_do_que_vale() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        let resposta = request
            .post("/api/devices")
            .json(&serde_json::json!({
                "name": "x", "type": "router", "operatingSystem": "cisco-ios"
            }))
            .await;
        assert_eq!(resposta.status_code(), 422, "{}", resposta.text());
        let texto = resposta.text();
        for aceito in ["auto", "routeros", "openwrt", "linux", "windows", "other"] {
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
async fn a_declaracao_do_cadastro_decide_os_comandos_da_ativacao_de_log() {
    // O ponto de consumo. Sem isto o campo seria decorativo: o operador
    // declararia "OpenWrt" e a tela de log continuaria propondo comandos de
    // RouterOS, que o equipamento recusaria linha por linha.
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        let corpo = cria_dispositivo(
            &request,
            serde_json::json!({
                "name": "ap", "type": "ap", "ipAddress": "192.168.77.3",
                // O texto livre diz MikroTik — e a declaração precisa vencê-lo.
                "vendor": "MikroTik", "operatingSystem": "openwrt"
            }),
        )
        .await;
        let id = corpo["id"].as_i64().expect("id");

        let resposta = request
            .get(&format!("/api/logs/devices/{id}/provision-hints"))
            .await;
        assert_eq!(resposta.status_code(), 200, "{}", resposta.text());
        let dicas: Value = serde_json::from_str(&resposta.text()).expect("json");
        assert_eq!(dicas["operatingSystem"], "openwrt");
        assert_eq!(dicas["operatingSystemSource"], "declarado");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn sistema_sem_receita_falha_na_validacao_e_nao_no_equipamento() {
    // `windows` e `other` estão no catálogo e **não** têm comandos de syslog.
    // A recusa precisa vir antes de qualquer conexão: descobrir isso depois de
    // entregar usuário e senha ao servidor seria o pior momento possível.
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        let corpo = cria_dispositivo(
            &request,
            serde_json::json!({
                "name": "estacao", "type": "server", "ipAddress": "127.0.0.1"
            }),
        )
        .await;
        let id = corpo["id"].as_i64().expect("id");

        let resposta = request
            .post(&format!("/api/logs/devices/{id}/provision"))
            .json(&serde_json::json!({
                "protocol": "ssh", "username": "admin", "password": "senha",
                "operatingSystem": "windows", "serverAddress": "192.168.1.10"
            }))
            .await;
        assert_eq!(resposta.status_code(), 422, "{}", resposta.text());
        assert!(
            resposta.text().contains("receita"),
            "a mensagem precisa dizer que não há receita: {}",
            resposta.text()
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn o_dispositivo_criado_pela_vpn_ja_nasce_com_o_sistema_declarado() {
    // Ali o sistema não é dedução: é o perfil que o operador escolheu para
    // gerar a configuração. Perguntar de novo depois seria pedir o que já
    // estava respondido no banco.
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        let servidor = request
            .put("/api/vpn/server")
            .json(&serde_json::json!({ "cidr": "10.8.0.0/24", "listenPort": 51820 }))
            .await;
        assert_eq!(servidor.status_code(), 200, "{}", servidor.text());

        let criado = request
            .post("/api/vpn/peers")
            .json(&serde_json::json!({ "name": "filial", "profile": "mikrotik" }))
            .await;
        assert_eq!(criado.status_code(), 201, "{}", criado.text());
        let corpo: Value = serde_json::from_str(&criado.text()).expect("json");

        // `mikrotik` é o nome do gerador de configuração; `routeros` é o do
        // sistema. A tradução mora no catálogo, e é ela que se verifica aqui.
        assert_eq!(corpo["device"]["operatingSystem"], "routeros");
        assert_eq!(corpo["device"]["accessMode"], "vpn");
    })
    .await;
}
