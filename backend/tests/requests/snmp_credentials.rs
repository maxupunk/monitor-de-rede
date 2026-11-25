//! Credenciais SNMP (`/api/snmp/credentials`): o cadastro, a máscara dos
//! segredos, a recusa de apagar credencial em uso, o papel e a credencial
//! padrão que o dispositivo recebe.
//!
//! A autenticação contra um agente de verdade (v2c, v3 authNoPriv e authPriv,
//! senha errada) está em `snmp_credentials_real_snmpd` (`#[ignore]`), porque
//! precisa de um `snmpd` no ar.

use backend::app::App;
use loco_rs::testing::prelude::*;
use netmonitor_core::{
    audit::{AuditAction, ResourceType},
    devices::snmp_credentials::{self, SnmpVersion, FALLBACK_PROFILE_NAME},
    models::{_entities::audit_logs, devices, snmp_profiles},
};
use netmonitor_nucleo::services::users::Role;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::{json, Value};
use serial_test::serial;

use super::prepare_data;

const MASK: &str = "********";

fn corpo_de(texto: &str) -> Value {
    serde_json::from_str(texto).unwrap_or(Value::Null)
}

async fn cria(request: &loco_rs::TestServer, payload: Value) -> Value {
    let resposta = request.post("/api/snmp/credentials").json(&payload).await;
    assert_eq!(resposta.status_code(), 201, "{}", resposta.text());
    corpo_de(&resposta.text())
}

fn v3_auth_priv(nome: &str) -> Value {
    json!({
        "name": nome, "version": "v3", "username": "monitor",
        "authProtocol": "sha", "authKey": "senha-auth-1",
        "privProtocol": "aes128", "privKey": "senha-priv-1",
    })
}

async fn gravada(ctx: &loco_rs::app::AppContext, id: i64) -> snmp_credentials::SnmpCredentials {
    snmp_credentials::for_profile(&ctx.db, id)
        .await
        .expect("credencial gravada")
}

#[tokio::test]
#[serial]
async fn cadastro_mascara_segredos_e_mascara_mantem_o_gravado() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        prepare_data::session_with_role(&mut request, &ctx, Role::Operator).await;

        let v2c = cria(
            &request,
            json!({ "name": "Comunidade da casa", "version": "v2c", "community": "segredo-casa" }),
        )
        .await;
        assert_eq!(v2c["community"], MASK);
        assert_eq!(v2c["securityLevel"], "community");

        let v3 = cria(&request, v3_auth_priv("Core v3")).await;
        let id = v3["id"].as_i64().unwrap();
        assert_eq!(v3["securityLevel"], "authPriv");
        assert_eq!(v3["authKey"], MASK);
        assert_eq!(v3["privKey"], MASK);
        assert!(v3["community"].is_null());

        let lista = request.get("/api/snmp/credentials").await;
        assert_eq!(lista.status_code(), 200);
        let texto = lista.text();
        for segredo in ["segredo-casa", "senha-auth-1", "senha-priv-1"] {
            assert!(
                !texto.contains(segredo),
                "segredo vazou na lista: {segredo}"
            );
        }
        assert_eq!(corpo_de(&lista.text()).as_array().unwrap().len(), 2);

        // A linha no banco também não guarda o segredo em claro.
        let linha = snmp_profiles::Entity::find_by_id(id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_ne!(linha.auth_key_encrypted.as_deref(), Some("senha-auth-1"));

        // Reenviar a máscara (ou deixar em branco) mantém os segredos.
        let mut edicao = v3_auth_priv("Core v3 renomeada");
        edicao["authKey"] = json!(MASK);
        edicao["privKey"] = json!("");
        let editada = request
            .put(&format!("/api/snmp/credentials/{id}"))
            .json(&edicao)
            .await;
        assert_eq!(editada.status_code(), 200, "{}", editada.text());
        assert_eq!(corpo_de(&editada.text())["name"], "Core v3 renomeada");
        let mantida = gravada(&ctx, id).await;
        assert_eq!(mantida.auth_key.as_deref(), Some("senha-auth-1"));
        assert_eq!(mantida.priv_key.as_deref(), Some("senha-priv-1"));

        // Senha nova substitui; tirar a privacidade vira authNoPriv.
        let trocada = request
            .put(&format!("/api/snmp/credentials/{id}"))
            .json(&json!({
                "name": "Core v3 renomeada", "version": "v3", "username": "monitor",
                "authProtocol": "sha256", "authKey": "senha-nova-22",
            }))
            .await;
        assert_eq!(trocada.status_code(), 200, "{}", trocada.text());
        assert_eq!(corpo_de(&trocada.text())["securityLevel"], "authNoPriv");
        let nova = gravada(&ctx, id).await;
        assert_eq!(nova.version, SnmpVersion::V3);
        assert_eq!(nova.auth_protocol.as_deref(), Some("sha256"));
        assert_eq!(nova.auth_key.as_deref(), Some("senha-nova-22"));
        assert!(nova.priv_key.is_none());

        // A auditoria registra criação e alteração, sem segredo.
        let trilha = audit_logs::Entity::find()
            .filter(audit_logs::Column::ResourceType.eq(ResourceType::SnmpCredential.as_str()))
            .all(&ctx.db)
            .await
            .unwrap();
        assert!(trilha
            .iter()
            .any(|row| row.action == AuditAction::Create.as_str()));
        assert!(trilha
            .iter()
            .any(|row| row.action == AuditAction::Update.as_str()));
        let trilha_texto = serde_json::to_string(
            &trilha
                .iter()
                .map(|row| row.changes.clone())
                .collect::<Vec<_>>(),
        )
        .unwrap();
        assert!(!trilha_texto.contains("senha-auth-1") && !trilha_texto.contains("senha-nova-22"));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn credencial_incompleta_ou_nome_repetido_e_recusada() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        prepare_data::session_with_role(&mut request, &ctx, Role::Admin).await;
        cria(&request, v3_auth_priv("Única")).await;

        let repetida = request
            .post("/api/snmp/credentials")
            .json(&v3_auth_priv("Única"))
            .await;
        assert_eq!(repetida.status_code(), 409, "{}", repetida.text());

        for invalida in [
            json!({ "name": "sem comunidade", "version": "v2c" }),
            json!({ "name": "sem usuário", "version": "v3", "authProtocol": "sha", "authKey": "senha-longa" }),
            json!({ "name": "senha curta", "version": "v3", "username": "u", "authProtocol": "sha", "authKey": "curta" }),
            json!({ "name": "priv sem auth", "version": "v3", "username": "u", "privProtocol": "aes128", "privKey": "senha-longa" }),
            json!({ "name": "versão", "version": "v4", "community": "x" }),
            json!({ "name": "  ", "version": "v2c", "community": "x" }),
        ] {
            let resposta = request.post("/api/snmp/credentials").json(&invalida).await;
            assert_eq!(resposta.status_code(), 422, "{invalida}: {}", resposta.text());
        }
    })
    .await;
}

#[tokio::test]
#[serial]
async fn credencial_em_uso_nao_e_apagada() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        prepare_data::session_with_role(&mut request, &ctx, Role::Operator).await;
        let usada = cria(&request, v3_auth_priv("Usada")).await;
        let id = usada["id"].as_i64().unwrap();
        let dispositivo = request
            .post("/api/devices")
            .json(&json!({
                "name": "core-sw", "type": "switch", "ipAddress": "10.20.0.1",
                "snmpEnabled": true, "snmpProfileId": id,
            }))
            .await;
        assert_eq!(dispositivo.status_code(), 201, "{}", dispositivo.text());
        assert_eq!(corpo_de(&dispositivo.text())["snmpProfileId"], id);

        let lista = corpo_de(&request.get("/api/snmp/credentials").await.text());
        assert_eq!(lista[0]["deviceCount"], 1);

        let recusada = request.delete(&format!("/api/snmp/credentials/{id}")).await;
        assert_eq!(recusada.status_code(), 409, "{}", recusada.text());
        assert!(recusada.text().contains("core-sw"), "{}", recusada.text());

        let livre = cria(
            &request,
            json!({ "name": "Livre", "version": "v1", "community": "x" }),
        )
        .await;
        let apagada = request
            .delete(&format!("/api/snmp/credentials/{}", livre["id"]))
            .await;
        assert_eq!(apagada.status_code(), 204, "{}", apagada.text());
        assert_eq!(
            request
                .get(&format!("/api/snmp/credentials/{}", livre["id"]))
                .await
                .status_code(),
            404
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn leitor_le_mas_nao_muda_credencial() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        prepare_data::session_with_role(&mut request, &ctx, Role::Viewer).await;
        assert_eq!(
            request.get("/api/snmp/credentials").await.status_code(),
            200
        );
        let criar = request
            .post("/api/snmp/credentials")
            .json(&v3_auth_priv("Leitor"))
            .await;
        assert_eq!(criar.status_code(), 403, "{}", criar.text());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn dispositivo_com_snmp_recebe_a_credencial_padrao() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        prepare_data::session_with_role(&mut request, &ctx, Role::Operator).await;

        // Sem nenhuma credencial: nasce a padrão de fábrica (public, v2c).
        let primeiro = request
            .post("/api/devices")
            .json(&json!({ "name": "sw-1", "type": "switch", "snmpEnabled": true }))
            .await;
        assert_eq!(primeiro.status_code(), 201, "{}", primeiro.text());
        let fabrica = corpo_de(&primeiro.text())["snmpProfileId"]
            .as_i64()
            .expect("padrão");
        let perfil = snmp_profiles::Entity::find_by_id(fabrica)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(perfil.name, FALLBACK_PROFILE_NAME);
        assert!(perfil.is_default);
        assert_eq!(
            gravada(&ctx, fabrica).await,
            snmp_credentials::SnmpCredentials::fallback()
        );

        // Sem SNMP, nada é inventado.
        let sem_snmp = request
            .post("/api/devices")
            .json(&json!({ "name": "host", "type": "server" }))
            .await;
        assert!(corpo_de(&sem_snmp.text())["snmpProfileId"].is_null());

        // Marcar outra como padrão desmarca a anterior; o dispositivo novo
        // recebe a nova, e o que já existia continua com a dele.
        let casa = cria(
            &request,
            json!({ "name": "Casa", "version": "v2c", "community": "casa", "isDefault": true }),
        )
        .await;
        let lista = corpo_de(&request.get("/api/snmp/credentials").await.text());
        let padroes: Vec<&Value> = lista
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| item["isDefault"] == true)
            .collect();
        assert_eq!(padroes.len(), 1);
        assert_eq!(padroes[0]["id"], casa["id"]);

        let segundo = request
            .post("/api/devices")
            .json(&json!({ "name": "sw-2", "type": "switch", "snmpEnabled": true }))
            .await;
        assert_eq!(corpo_de(&segundo.text())["snmpProfileId"], casa["id"]);
        let sw1 = devices::Entity::find()
            .filter(devices::Column::Name.eq("sw-1"))
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(sw1.snmp_profile_id, Some(fabrica));

        // Credencial inexistente é recusada no cadastro.
        let inexistente = request
            .post("/api/devices")
            .json(&json!({
                "name": "sw-3", "type": "switch", "snmpEnabled": true, "snmpProfileId": 9999
            }))
            .await;
        assert_eq!(inexistente.status_code(), 422, "{}", inexistente.text());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn teste_de_credencial_contra_alvo_sem_agente_falha_com_mensagem() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        prepare_data::session_with_role(&mut request, &ctx, Role::Operator).await;
        let perfil = cria(
            &request,
            json!({ "name": "Teste", "version": "v2c", "community": "public" }),
        )
        .await;
        // Porta sem agente no loopback: falha, com mensagem — nunca 200 vazio.
        let resposta = request
            .post(&format!("/api/snmp/credentials/{}/test", perfil["id"]))
            .json(&json!({ "host": "127.0.0.1", "port": 9 }))
            .await;
        assert_ne!(resposta.status_code(), 200, "{}", resposta.text());
        assert!(
            corpo_de(&resposta.text())["message"].is_string(),
            "{}",
            resposta.text()
        );
    })
    .await;
}
