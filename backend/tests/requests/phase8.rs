//! Requisições do domínio VPN WireGuard (Fase 8).
//!
//! Os testes escrevem o `wg0.conf` num diretório temporário: `WG_CONFIG_DIR` é
//! apontado para o `temp_dir` do processo antes de qualquer chamada, senão o
//! serviço tentaria gravar em `/config`.

use backend::{
    app::App,
    models::{vpn_peers, vpn_servers},
    services::vpn::{key_generator, secret_store},
};
use loco_rs::testing::prelude::*;
use sea_orm::EntityTrait;
use serial_test::serial;

use super::prepare_data;

fn json_of(text: &str) -> serde_json::Value {
    serde_json::from_str(text).expect("resposta JSON")
}

/// Isola a escrita do `wg0.conf` e devolve o diretório usado.
fn isolar_volume_wireguard(nome: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("wg-{nome}-{}", std::process::id()));
    std::env::set_var("WG_CONFIG_DIR", &dir);
    dir
}

#[tokio::test]
#[serial]
async fn o_painel_reporta_a_vpn_nao_configurada_antes_do_primeiro_put() {
    let _dir = isolar_volume_wireguard("painel");
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let painel = json_of(&request.get("/api/vpn/server").await.text());
        assert_eq!(painel["configured"], false);
        assert!(painel["server"].is_null());
        assert_eq!(painel["peersTotal"], 0);
        assert_eq!(painel["persistentKeepalive"], 25);
        // Os cinco cards do wizard vêm mesmo sem servidor: é o que a tela
        // precisa para desenhar a escolha de perfil.
        assert_eq!(painel["profiles"].as_array().unwrap().len(), 5);

        // Sem servidor não há IP a sugerir — 400 com mensagem, não 500.
        let sem_servidor = request.get("/api/vpn/peers/next-ip").await;
        assert_eq!(sem_servidor.status_code(), 400);
        assert!(json_of(&sem_servidor.text())["message"]
            .as_str()
            .unwrap()
            .contains("ainda não foi configurado"));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn configurar_o_servidor_gera_chaves_e_escreve_o_wg0_conf() {
    let dir = isolar_volume_wireguard("servidor");
    let _ = std::fs::remove_dir_all(&dir);
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let salvo = request
            .put("/api/vpn/server")
            .json(&serde_json::json!({
                "cidr": "10.9.0.0/24",
                "listenPort": 51821,
                "publicEndpoint": "vpn.exemplo.com.br",
                "mtu": 1380
            }))
            .await;
        assert_eq!(salvo.status_code(), 200);
        let corpo = json_of(&salvo.text());
        assert_eq!(corpo["cidr"], "10.9.0.0/24");
        assert_eq!(corpo["serverAddress"], "10.9.0.1");
        assert_eq!(corpo["server"]["listenPort"], 51_821);
        assert_eq!(corpo["server"]["mtu"], 1_380);
        assert!(corpo["message"].as_str().unwrap().contains("sem derrubar"));

        // A chave pública é uma chave WireGuard de verdade, e a privada não sai.
        let public_key = corpo["server"]["publicKey"].as_str().unwrap();
        assert!(key_generator::is_valid_key(public_key));
        assert!(!salvo.text().contains("privateKey"));

        // A privada foi cifrada em repouso e decifra de volta.
        let servidor = vpn_servers::Entity::find()
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        let private_key = servidor.private_key().expect("chave decifra");
        assert_ne!(private_key, servidor.private_key_encrypted);
        assert_eq!(
            key_generator::derive_public_key(&private_key).unwrap(),
            public_key
        );
    })
    .await;

    // O arquivo foi escrito no volume, com a chave privada dentro.
    let conf = std::fs::read_to_string(dir.join("wg0.conf")).expect("wg0.conf escrito");
    assert!(conf.contains("[Interface]"));
    assert!(conf.contains("Address = 10.9.0.1/24"));
    assert!(conf.contains("ListenPort = 51821"));
    assert!(conf.contains("MTU = 1380"));
    // Isolamento entre peers é o padrão (matriz #36).
    assert!(conf.contains("-o wg0 -j DROP"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
#[serial]
async fn cidr_invalido_e_recusado_antes_de_qualquer_escrita() {
    let _dir = isolar_volume_wireguard("cidr");
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let recusado = request
            .put("/api/vpn/server")
            .json(&serde_json::json!({ "cidr": "10.8.0.0/99" }))
            .await;
        assert_eq!(recusado.status_code(), 422);
        assert!(json_of(&recusado.text())["message"]
            .as_str()
            .unwrap()
            .contains("CIDR inválido"));
        // Nada foi criado.
        assert!(vpn_servers::Entity::find()
            .one(&ctx.db)
            .await
            .unwrap()
            .is_none());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn o_wizard_cria_o_peer_com_device_monitores_e_artefato() {
    let dir = isolar_volume_wireguard("peer");
    let _ = std::fs::remove_dir_all(&dir);
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);
        request
            .put("/api/vpn/server")
            .json(&serde_json::json!({
                "cidr": "10.8.0.0/24", "publicEndpoint": "vpn.exemplo.com.br"
            }))
            .await;

        // O primeiro IP livre pula o endereço do servidor.
        let sugestao = json_of(&request.get("/api/vpn/peers/next-ip").await.text());
        assert_eq!(sugestao["ipAddress"], "10.8.0.2");
        assert_eq!(sugestao["cidr"], "10.8.0.0/24");

        let criado = request
            .post("/api/vpn/peers")
            .json(&serde_json::json!({
                "name": "Filial 01", "profile": "mikrotik", "snmpEnabled": true
            }))
            .await;
        assert_eq!(criado.status_code(), 201);
        let corpo = json_of(&criado.text());
        let peer_id = corpo["peer"]["id"].as_i64().unwrap();

        assert_eq!(corpo["peer"]["deviceProfile"], "mikrotik");
        assert_eq!(corpo["peer"]["connectionStatus"], "awaiting");
        assert_eq!(corpo["peer"]["persistentKeepalive"], 25);
        // O dispositivo nasceu como roteador, no IP alocado.
        assert_eq!(corpo["device"]["ipAddress"], "10.8.0.2");
        assert_eq!(corpo["device"]["type"], "router");
        assert_eq!(corpo["device"]["snmpEnabled"], true);

        // O artefato traz a chave privada **uma vez** e o resumo sem ela.
        let artefato = &corpo["artifact"];
        assert_eq!(artefato["profile"], "mikrotik");
        assert_eq!(artefato["delivery"], "copy");
        assert!(artefato["qrSvg"].is_null(), "MikroTik não usa QR Code");
        assert!(!artefato["content"]
            .as_str()
            .unwrap()
            .contains("INDISPONIVEL"));
        assert!(!serde_json::to_string(&artefato["summary"])
            .unwrap()
            .contains("private-key"));

        // Ping e SNMP foram provisionados para o dispositivo.
        let monitores = json_of(&request.get("/api/monitors").await.text());
        let nomes: Vec<String> = monitores
            .as_array()
            .unwrap()
            .iter()
            .map(|monitor| monitor["name"].as_str().unwrap_or_default().to_string())
            .collect();
        assert!(nomes.contains(&"Ping Filial 01".to_string()));
        assert!(nomes.contains(&"SNMP Filial 01".to_string()));

        // Matriz #33: a criação guarda a chave no cofre, a primeira leitura a
        // consome e a segunda já vem com o placeholder.
        let primeira = json_of(
            &request
                .get(&format!("/api/vpn/peers/{peer_id}/config"))
                .await
                .text(),
        );
        assert!(!primeira["content"]
            .as_str()
            .unwrap()
            .contains("CHAVE-PRIVADA-INDISPONIVEL"));

        let segunda = json_of(
            &request
                .get(&format!("/api/vpn/peers/{peer_id}/config"))
                .await
                .text(),
        );
        assert!(segunda["content"]
            .as_str()
            .unwrap()
            .contains("CHAVE-PRIVADA-INDISPONIVEL"));
        // E o script avisa o operador em vez de falhar no equipamento.
        assert!(segunda["content"].as_str().unwrap().contains("ATENCAO"));

        // A chave pré-compartilhada nunca aparece na listagem.
        let listagem = request.get("/api/vpn/peers").await;
        assert_eq!(listagem.status_code(), 200);
        let peers = json_of(&listagem.text());
        assert_eq!(peers.as_array().unwrap().len(), 1);
        assert!(!listagem.text().contains("presharedKey"));
        assert_eq!(peers[0]["needsFirewallHint"], false);
        assert_eq!(peers[0]["device"]["name"], "Filial 01");

        let psk = vpn_peers::Entity::find_by_id(peer_id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert!(psk.preshared_key().unwrap().is_some(), "a PSK foi gerada");
    })
    .await;

    // O peer entrou no `wg0.conf` com `/32`.
    let conf = std::fs::read_to_string(dir.join("wg0.conf")).expect("wg0.conf");
    assert!(conf.contains("# Filial 01"));
    assert!(conf.contains("AllowedIPs = 10.8.0.2/32"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
#[serial]
async fn rotacionar_entrega_chave_nova_e_qrcode_so_no_perfil_movel() {
    let dir = isolar_volume_wireguard("rotacao");
    let _ = std::fs::remove_dir_all(&dir);
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        secret_store::client_key_store().clear();
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);
        request
            .put("/api/vpn/server")
            .json(&serde_json::json!({ "publicEndpoint": "vpn.exemplo.com.br" }))
            .await;

        let criado = json_of(
            &request
                .post("/api/vpn/peers")
                .json(&serde_json::json!({ "name": "Celular", "profile": "mobile" }))
                .await
                .text(),
        );
        let peer_id = criado["peer"]["id"].as_i64().unwrap();
        let chave_original = criado["peer"]["publicKey"].as_str().unwrap().to_string();
        // Perfil móvel já vem com o QR Code renderizado na mesma resposta.
        assert!(criado["artifact"]["qrSvg"]
            .as_str()
            .unwrap()
            .contains("<svg"));

        // A criação guarda a chave no cofre sem consumi-la: a primeira leitura
        // ainda entrega o QR Code de verdade.
        let primeira = request
            .get(&format!("/api/vpn/peers/{peer_id}/qrcode"))
            .await;
        assert_eq!(primeira.status_code(), 200);
        assert!(json_of(&primeira.text())["svg"]
            .as_str()
            .unwrap()
            .contains("<svg"));

        // Consumida a chave, o QR Code vira 409 (matriz #34).
        let conflito = request
            .get(&format!("/api/vpn/peers/{peer_id}/qrcode"))
            .await;
        assert_eq!(conflito.status_code(), 409);
        assert!(json_of(&conflito.text())["message"]
            .as_str()
            .unwrap()
            .contains("Rotacione as chaves"));

        let rotacionado = json_of(
            &request
                .post(&format!("/api/vpn/peers/{peer_id}/rotate"))
                .await
                .text(),
        );
        let chave_nova = rotacionado["peer"]["publicKey"].as_str().unwrap();
        assert_ne!(
            chave_nova, chave_original,
            "a chave anterior foi invalidada"
        );
        assert!(rotacionado["artifact"]["qrSvg"]
            .as_str()
            .unwrap()
            .contains("<svg"));

        // Rotacionar repõe a chave no cofre: o QR Code volta a ser entregue.
        let qr = request
            .get(&format!("/api/vpn/peers/{peer_id}/qrcode"))
            .await;
        assert_eq!(qr.status_code(), 200);
        assert_eq!(json_of(&qr.text())["profile"], "mobile");
        // E só uma vez, de novo.
        assert_eq!(
            request
                .get(&format!("/api/vpn/peers/{peer_id}/qrcode"))
                .await
                .status_code(),
            409
        );
    })
    .await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
#[serial]
async fn revogar_libera_o_ip_e_tira_o_peer_do_wg0_conf() {
    let dir = isolar_volume_wireguard("revogacao");
    let _ = std::fs::remove_dir_all(&dir);
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);
        request
            .put("/api/vpn/server")
            .json(&serde_json::json!({ "publicEndpoint": "vpn.exemplo.com.br" }))
            .await;

        let criado = json_of(
            &request
                .post("/api/vpn/peers")
                .json(&serde_json::json!({ "name": "Filial 01", "profile": "linux" }))
                .await
                .text(),
        );
        let peer_id = criado["peer"]["id"].as_i64().unwrap();
        assert_eq!(criado["device"]["ipAddress"], "10.8.0.2");

        // Segundo peer pega o IP seguinte.
        let segundo = json_of(
            &request
                .post("/api/vpn/peers")
                .json(&serde_json::json!({ "name": "Filial 02", "profile": "linux" }))
                .await
                .text(),
        );
        assert_eq!(segundo["device"]["ipAddress"], "10.8.0.3");

        let revogado = request.delete(&format!("/api/vpn/peers/{peer_id}")).await;
        assert_eq!(revogado.status_code(), 200);
        assert!(json_of(&revogado.text())["message"]
            .as_str()
            .unwrap()
            .contains("cortado imediatamente"));

        // Matriz #41: o IP volta a ficar livre porque o device foi removido.
        assert_eq!(
            json_of(&request.get("/api/vpn/peers/next-ip").await.text())["ipAddress"],
            "10.8.0.2"
        );
        assert!(vpn_peers::Entity::find_by_id(peer_id)
            .one(&ctx.db)
            .await
            .unwrap()
            .is_none());
    })
    .await;

    let conf = std::fs::read_to_string(dir.join("wg0.conf")).expect("wg0.conf");
    assert!(!conf.contains("# Filial 01"), "o revogado saiu do arquivo");
    assert!(conf.contains("# Filial 02"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
#[serial]
async fn renomear_acompanha_os_monitores_gerados_e_nao_os_editados_a_mao() {
    let dir = isolar_volume_wireguard("rename");
    let _ = std::fs::remove_dir_all(&dir);
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);
        request
            .put("/api/vpn/server")
            .json(&serde_json::json!({ "publicEndpoint": "vpn.exemplo.com.br" }))
            .await;

        let criado = json_of(
            &request
                .post("/api/vpn/peers")
                .json(&serde_json::json!({
                    "name": "Filial 01", "profile": "linux", "snmpEnabled": true
                }))
                .await
                .text(),
        );
        let peer_id = criado["peer"]["id"].as_i64().unwrap();

        // Um monitor renomeado à mão não pode ser arrastado pelo rename.
        let monitores = json_of(&request.get("/api/monitors").await.text());
        let snmp_id = monitores
            .as_array()
            .unwrap()
            .iter()
            .find(|monitor| monitor["name"] == "SNMP Filial 01")
            .expect("monitor SNMP provisionado")["id"]
            .as_i64()
            .unwrap();
        request
            .put(&format!("/api/monitors/{snmp_id}"))
            .json(&serde_json::json!({ "name": "SNMP do uplink", "type": "snmp" }))
            .await;

        let renomeado = request
            .patch(&format!("/api/vpn/peers/{peer_id}"))
            .json(&serde_json::json!({ "name": "Filial Centro" }))
            .await;
        assert_eq!(renomeado.status_code(), 200);
        assert_eq!(
            json_of(&renomeado.text())["device"]["name"],
            "Filial Centro"
        );

        let nomes: Vec<String> = json_of(&request.get("/api/monitors").await.text())
            .as_array()
            .unwrap()
            .iter()
            .map(|monitor| monitor["name"].as_str().unwrap_or_default().to_string())
            .collect();
        assert!(nomes.contains(&"Ping Filial Centro".to_string()));
        assert!(nomes.contains(&"SNMP do uplink".to_string()), "{nomes:?}");

        // Nome vazio é recusado.
        assert_eq!(
            request
                .patch(&format!("/api/vpn/peers/{peer_id}"))
                .json(&serde_json::json!({ "name": "   " }))
                .await
                .status_code(),
            400
        );
    })
    .await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
#[serial]
async fn as_dicas_de_firewall_saem_no_dialeto_do_equipamento() {
    let dir = isolar_volume_wireguard("hints");
    let _ = std::fs::remove_dir_all(&dir);
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);
        request
            .put("/api/vpn/server")
            .json(&serde_json::json!({ "publicEndpoint": "vpn.exemplo.com.br" }))
            .await;

        let criado = json_of(
            &request
                .post("/api/vpn/peers")
                .json(&serde_json::json!({ "name": "Roteador", "profile": "openwrt" }))
                .await
                .text(),
        );
        let peer_id = criado["peer"]["id"].as_i64().unwrap();

        let hints = json_of(
            &request
                .post(&format!("/api/vpn/peers/{peer_id}/firewall-hints"))
                .await
                .text(),
        );
        assert_eq!(hints["profile"], "openwrt");
        assert_eq!(hints["label"], "OpenWrt");
        assert!(hints["content"]
            .as_str()
            .unwrap()
            .contains("uci add firewall zone"));

        // O OpenWrt entrega as duas variantes de gerenciador de pacotes.
        let variantes = criado["artifact"]["variants"].as_array().unwrap();
        assert_eq!(variantes.len(), 2);
        assert_eq!(variantes[0]["id"], "opkg");
        assert_eq!(variantes[1]["id"], "apk");
    })
    .await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
#[serial]
async fn perfil_desconhecido_e_ip_fora_da_faixa_sao_recusados() {
    let dir = isolar_volume_wireguard("validacao");
    let _ = std::fs::remove_dir_all(&dir);
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);
        request
            .put("/api/vpn/server")
            .json(&serde_json::json!({ "publicEndpoint": "vpn.exemplo.com.br" }))
            .await;

        let perfil = request
            .post("/api/vpn/peers")
            .json(&serde_json::json!({ "name": "X", "profile": "cisco" }))
            .await;
        assert_eq!(perfil.status_code(), 400);
        assert!(json_of(&perfil.text())["message"]
            .as_str()
            .unwrap()
            .contains("não suportado"));

        // Sem nome ou sem perfil: 400 com a mensagem do wizard.
        assert_eq!(
            request
                .post("/api/vpn/peers")
                .json(&serde_json::json!({ "profile": "linux" }))
                .await
                .status_code(),
            400
        );

        let fora = request
            .post("/api/vpn/peers")
            .json(&serde_json::json!({
                "name": "X", "profile": "linux", "ipAddress": "192.168.0.5"
            }))
            .await;
        assert_eq!(fora.status_code(), 400);
        assert!(json_of(&fora.text())["message"]
            .as_str()
            .unwrap()
            .contains("não pertence à faixa"));

        // O endereço do servidor é reservado.
        let reservado = request
            .post("/api/vpn/peers")
            .json(&serde_json::json!({
                "name": "X", "profile": "linux", "ipAddress": "10.8.0.1"
            }))
            .await;
        assert_eq!(reservado.status_code(), 400);
        assert!(json_of(&reservado.text())["message"]
            .as_str()
            .unwrap()
            .contains("reservado para o servidor"));
    })
    .await;
    let _ = std::fs::remove_dir_all(&dir);
}
