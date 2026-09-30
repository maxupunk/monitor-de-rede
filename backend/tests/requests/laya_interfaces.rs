//! O Laya indicando o uplink e as interfaces que valem monitorar.
//!
//! O que se afirma aqui, contra o Ollaya falso: o uplink sugerido é a
//! interface cujo texto diz WAN, devolvido pelo ifIndex; as que valem
//! monitorar vêm com confiança; e com a frente desligada a resposta é
//! `available: false`, sem consulta.

use backend::{
    app::App,
    dtos::snmp::InterfaceSuggestions,
    services::ai::{laya::decisions::interfaces::InterfaceCandidate, settings},
};
use loco_rs::testing::prelude::*;
use serde_json::json;
use serial_test::serial;

use super::{
    fake_ollaya::{self, by_keyword, with_key},
    prepare_data,
};

fn interfaces() -> Vec<InterfaceCandidate> {
    let port = |if_index: i32, name: &str, alias: Option<&str>| InterfaceCandidate {
        if_index,
        name: name.into(),
        alias: alias.map(str::to_string),
        if_type: Some(6),
        speed: Some(1_000_000_000),
        oper_up: Some(true),
        in_octets: Some(1_000),
        out_octets: Some(1_000),
        ..InterfaceCandidate::default()
    };
    vec![
        port(1, "ether1", Some("LAN-escritorio")),
        port(5, "ether5", Some("WAN-Vivo")),
        port(8, "ether8", None),
    ]
}

#[tokio::test]
#[serial]
async fn sugere_o_uplink_pelo_texto_e_as_que_valem_monitorar() {
    request_with_config::<App, _, _>(RequestConfig::default(), |request, ctx| async move {
        with_key();
        // Escolha: o critério que menciona WAN. Sim/não: só a pergunta da ether5.
        let fake = fake_ollaya::start(by_keyword(&[("wan", "*"), ("", "mon_if5")])).await;
        settings::save_laya(&ctx.db, fake.settings()).await.unwrap();
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (h, v) = prepare_data::auth_header(&session.token);

        let response = request
            .post("/api/snmp/interfaces/suggestions")
            .add_header(h, v)
            .json(&json!({ "deviceName": "Borda", "interfaces": interfaces() }))
            .await;
        response.assert_status_ok();
        let suggestions: InterfaceSuggestions = response.json();

        assert!(suggestions.available);
        let uplink = suggestions.uplink.expect("uplink sugerido");
        assert_eq!(uplink.value, "5");
        assert_eq!(uplink.model, "laya:multilingual");
        assert_eq!(suggestions.monitor.len(), 1);
        assert_eq!(suggestions.monitor[0].if_index, 5);
        assert_eq!(fake.calls(), 1, "uma chamada para tudo");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn frente_desligada_nao_consulta() {
    request_with_config::<App, _, _>(RequestConfig::default(), |request, ctx| async move {
        with_key();
        let fake = fake_ollaya::start(by_keyword(&[("wan", "*")])).await;
        let mut laya = fake.settings();
        laya.features.interfaces = false;
        settings::save_laya(&ctx.db, laya).await.unwrap();
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (h, v) = prepare_data::auth_header(&session.token);

        let response = request
            .post("/api/snmp/interfaces/suggestions")
            .add_header(h, v)
            .json(&json!({ "interfaces": interfaces() }))
            .await;
        response.assert_status_ok();
        let suggestions: InterfaceSuggestions = response.json();

        assert!(!suggestions.available);
        assert!(suggestions.uplink.is_none());
        assert_eq!(fake.calls(), 0);
    })
    .await;
}
