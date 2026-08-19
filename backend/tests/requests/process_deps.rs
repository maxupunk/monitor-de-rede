//! Regressão dos dois defeitos que a ADR 007 corrigiu.
//!
//! Os dois viveram meses sem serem notados porque **nenhum teste bootava fora
//! do caminho do servidor** — e o caminho do servidor é o único onde os
//! `Initializer` do Loco rodam. Estes testes fecham exatamente essa brecha.

use backend::{
    app::App,
    services::{
        discovery::service::ScanSessionService,
        events::{relay::relay_pending, EventBus},
    },
    tasks::scheduler_run::run_cycle,
};
use chrono::Utc;
use loco_rs::{
    app::Hooks, boot::create_context, environment::Environment, testing::prelude::*, Result,
};
use sea_orm::{ActiveModelTrait, Set};
use serial_test::serial;

/// Monta o contexto **do mesmo jeito que `run_task` monta**.
///
/// `boot_test`/`request_with_config` passam por `create_app` → `run_app`, que
/// executa os initializers. Um `task` (o `scheduler` e o `probe`) não passa:
/// `loco_rs::boot::run_task` só registra e roda. Reproduzir esse caminho é o
/// ponto inteiro deste arquivo.
async fn contexto_de_tarefa() -> Result<loco_rs::app::AppContext> {
    let config = App::load_config(&Environment::Test).await?;
    create_context::<App>(&Environment::Test, config).await
}

/// Um processo de tarefa precisa das dependências de processo.
///
/// Enquanto elas viveram num `Initializer`, este contexto nascia vazio e o
/// `scheduler` gravava `unknown` com "Cliente ICMP não inicializado" em todo
/// monitor de ping. Se alguém mover a instalação de volta para um
/// `Initializer`, este teste quebra.
#[tokio::test]
#[serial]
async fn contexto_de_tarefa_tem_as_dependencias_de_processo() {
    let ctx = contexto_de_tarefa()
        .await
        .expect("criar contexto de tarefa");

    assert!(
        EventBus::from_context(&ctx).is_ok(),
        "barramento de eventos ausente num contexto de tarefa — as dependências \
         de processo voltaram para um Initializer?"
    );
    assert!(
        ScanSessionService::from_context(&ctx).is_ok(),
        "sessão de scan ausente num contexto de tarefa"
    );
}

/// O ciclo do scheduler **não** pode drenar o `event_outbox`.
///
/// O barramento é in-process: quem tem conexão SSE aberta é o servidor. Quando
/// o relay era chamado de dentro do `run_cycle`, o evento gerado pelo ciclo
/// ficava parado na tabela e nunca chegava à tela. Este teste falha se o relay
/// voltar para o ciclo.
#[tokio::test]
#[serial]
async fn o_ciclo_do_scheduler_nao_consome_o_outbox() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let bus = EventBus::from_context(&ctx).expect("barramento inicializado");

        backend::models::event_outbox::ActiveModel {
            r#type: Set("monitor:result".into()),
            origin: Set("outro-processo".into()),
            payload: Set(serde_json::json!({
                "type": "monitor:result",
                "data": { "monitorId": 7 },
                "timestamp": Utc::now().to_rfc3339(),
            })),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("gravar no outbox");

        // Alguém escutando: se o ciclo relaysse, ele consumiria a linha aqui.
        let mut inscrito = bus.subscribe();
        run_cycle(&ctx).await.expect("ciclo do scheduler");
        // O ciclo publica os eventos dos monitores que ele mesmo executou —
        // inclusive a coleta de saúde do sistema, provisionada no boot. O que
        // ele não pode fazer é entregar o que estava no `event_outbox`, que é
        // de outro processo. Por isso a asserção é sobre a **origem** do
        // evento, e não sobre haver evento algum: um `try_recv` vazio deixou
        // de distinguir as duas coisas no dia em que o servidor ganhou um
        // monitor próprio.
        while let Ok(evento) = inscrito.try_recv() {
            let texto = serde_json::to_string(&evento).unwrap_or_default();
            assert!(
                !texto.contains("\"monitorId\":7"),
                "o ciclo do scheduler entregou o evento do outbox — o relay voltou para o ciclo: {texto}"
            );
        }

        // E a prova de que o evento continua disponível para quem deve
        // entregá-lo: o servidor.
        assert_eq!(
            relay_pending(&ctx).await.expect("relay do servidor"),
            1,
            "o evento sumiu do outbox sem ter sido entregue por ninguém"
        );
    })
    .await;
}
