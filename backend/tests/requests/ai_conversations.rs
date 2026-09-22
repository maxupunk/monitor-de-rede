//! `/api/ai/conversations` — histórico do assistente guardado no servidor,
//! para o usuário encontrar as conversas em qualquer computador.
//!
//! O que se afirma aqui: o ciclo criar → listar → abrir → atualizar → apagar;
//! a conversa de um usuário é invisível para outro (responde 404, não 403, para
//! não revelar que o id existe); e o teto por usuário descarta as mais antigas.

use backend::{
    app::App,
    services::ai::conversations::{self, MAX_CONVERSATIONS_PER_USER},
};
use loco_rs::testing::prelude::*;
use serde_json::{json, Value};
use serial_test::serial;

use super::prepare_data;

fn corpo(title: &str, perguntas: &[&str]) -> Value {
    let messages: Vec<Value> = perguntas
        .iter()
        .enumerate()
        .map(|(i, texto)| json!({ "id": format!("m{i}"), "role": "user", "content": texto }))
        .collect();
    json!({ "title": title, "messages": messages })
}

#[tokio::test]
#[serial]
async fn ciclo_completo_da_conversa_salva() {
    request_with_config::<App, _, _>(RequestConfig::default(), |request, ctx| async move {
        let sessao = prepare_data::init_user_login(&request, &ctx).await;
        let (h, v) = prepare_data::auth_header(&sessao.token);

        let criada = request
            .post("/api/ai/conversations")
            .add_header(h.clone(), v.clone())
            .json(&corpo("  Ping   no gateway ", &["ping no gateway"]))
            .await;
        assert_eq!(criada.status_code(), 201, "{}", criada.text());
        let criada: Value = criada.json();
        let id = criada["id"].as_i64().unwrap();
        assert_eq!(criada["title"], "Ping no gateway");
        assert_eq!(criada["messageCount"], 1);

        let atualizada = request
            .put(&format!("/api/ai/conversations/{id}"))
            .add_header(h.clone(), v.clone())
            .json(&corpo(
                "Ping no gateway",
                &["ping no gateway", "e o traceroute?"],
            ))
            .await;
        assert_eq!(atualizada.status_code(), 200, "{}", atualizada.text());

        let lista: Value = request
            .get("/api/ai/conversations")
            .add_header(h.clone(), v.clone())
            .await
            .json();
        assert_eq!(lista.as_array().unwrap().len(), 1);
        assert_eq!(lista[0]["messageCount"], 2);
        assert!(
            lista[0].get("messages").is_none(),
            "a lista não carrega as mensagens"
        );

        let detalhe: Value = request
            .get(&format!("/api/ai/conversations/{id}"))
            .add_header(h.clone(), v.clone())
            .await
            .json();
        assert_eq!(detalhe["messages"][1]["content"], "e o traceroute?");

        let apagada = request
            .delete(&format!("/api/ai/conversations/{id}"))
            .add_header(h.clone(), v.clone())
            .await;
        assert_eq!(apagada.status_code(), 204);
        let depois = request
            .get(&format!("/api/ai/conversations/{id}"))
            .add_header(h, v)
            .await;
        assert_eq!(depois.status_code(), 404);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn conversa_de_um_usuario_nao_aparece_para_outro() {
    request_with_config::<App, _, _>(RequestConfig::default(), |request, ctx| async move {
        let dono = prepare_data::init_user_login(&request, &ctx).await;
        let outro = prepare_data::init_operator(&ctx).await;
        let (hd, vd) = prepare_data::auth_header(&dono.token);
        let (ho, vo) = prepare_data::auth_header(&outro.token);

        let criada: Value = request
            .post("/api/ai/conversations")
            .add_header(hd, vd)
            .json(&corpo("Segredo", &["qual a senha do roteador?"]))
            .await
            .json();
        let id = criada["id"].as_i64().unwrap();

        let lista: Value = request
            .get("/api/ai/conversations")
            .add_header(ho.clone(), vo.clone())
            .await
            .json();
        assert_eq!(lista.as_array().unwrap().len(), 0);

        for resposta in [
            request
                .get(&format!("/api/ai/conversations/{id}"))
                .add_header(ho.clone(), vo.clone())
                .await,
            request
                .put(&format!("/api/ai/conversations/{id}"))
                .add_header(ho.clone(), vo.clone())
                .json(&corpo("sequestrada", &[]))
                .await,
            request
                .delete(&format!("/api/ai/conversations/{id}"))
                .add_header(ho.clone(), vo.clone())
                .await,
        ] {
            assert_eq!(resposta.status_code(), 404, "{}", resposta.text());
        }
        assert_eq!(
            conversations::count(&ctx.db, dono.user.id).await.unwrap(),
            1
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn entrada_invalida_e_recusada_e_o_teto_descarta_as_mais_antigas() {
    request_with_config::<App, _, _>(RequestConfig::default(), |request, ctx| async move {
        let sessao = prepare_data::init_user_login(&request, &ctx).await;
        let (h, v) = prepare_data::auth_header(&sessao.token);

        let invalida = request
            .post("/api/ai/conversations")
            .add_header(h, v)
            .json(&json!({ "title": "x", "messages": { "role": "user" } }))
            .await;
        assert_eq!(invalida.status_code(), 422, "{}", invalida.text());

        let user_id = sessao.user.id;
        let mut primeira = None;
        for i in 0..=MAX_CONVERSATIONS_PER_USER {
            let criada = conversations::create(
                &ctx.db,
                user_id,
                serde_json::from_value(corpo(&format!("c{i}"), &["oi"])).unwrap(),
            )
            .await
            .unwrap();
            primeira.get_or_insert(criada.id);
        }
        assert_eq!(
            conversations::count(&ctx.db, user_id).await.unwrap(),
            MAX_CONVERSATIONS_PER_USER
        );
        assert!(
            conversations::get(&ctx.db, user_id, primeira.unwrap())
                .await
                .is_err(),
            "a mais antiga saiu"
        );
    })
    .await;
}
