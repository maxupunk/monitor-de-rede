//! Regressão dos dois defeitos que a ADR 007 corrigiu.
//!
//! Os dois viveram meses sem serem notados porque **nenhum teste bootava fora
//! do caminho do servidor** — e o caminho do servidor é o único onde os
//! `Initializer` do Loco rodam. Estes testes fecham exatamente essa brecha.

use backend::{app::App, tasks::scheduler_run::run_cycle};

use netmonitor_network::discovery::service::ScanSessionService;

use chrono::Utc;
use loco_rs::{
    app::Hooks, boot::create_context, environment::Environment, testing::prelude::*, Result,
};
use netmonitor_nucleo::{
    services::events::EventBus, services::monitoring::checkers::ping::PingClientCell,
};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, Set};
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
    // Instalada mesmo quando o socket não abre: é ela que tenta de novo
    // depois que o `ping_group_range` é liberado, sem reiniciar o processo.
    assert!(
        ctx.shared_store.contains::<PingClientCell>(),
        "célula do cliente ICMP ausente num contexto de tarefa"
    );
    // O `task scheduler_loop`/`scheduler_run` disputa a liderança do ciclo
    // (ADR 024): sem ela no contexto de tarefa, o ciclo dedicado rodaria ao
    // lado do servidor.
    assert!(
        ctx.shared_store
            .contains::<netmonitor_core::cluster::leadership::Leadership>(),
        "liderança ausente num contexto de tarefa"
    );
}

/// O ciclo do scheduler **não** entrega o que está no `event_outbox`.
///
/// O evento de outro processo chega pelo aviso do commit dele (ADR 024), não
/// por quem lê a tabela. Quando o relay era chamado de dentro do `run_cycle`,
/// o evento gerado pelo ciclo ficava parado na tabela e nunca chegava à tela.
/// Este teste falha se alguém voltar a varrer o outbox no ciclo.
#[tokio::test]
#[serial]
async fn o_ciclo_do_scheduler_nao_consome_o_outbox() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let bus = EventBus::from_context(&ctx).expect("barramento inicializado");

        netmonitor_core::models::event_outbox::ActiveModel {
            r#type: Set("monitor:result".into()),
            origin: Set("outro-processo".into()),
            payload: Set(serde_json::json!({
                "type": "monitor:result",
                "data": { "monitorId": 7 },
                "timestamp": Utc::now().to_rfc3339(),
            })),
            // Instante explícito: a purga do ciclo compara `created_at` no
            // formato do `sea-orm`, e a linha recém-gravada não pode vencer.
            created_at: Set(Utc::now().into()),
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

        // E a linha continua no outbox: o ciclo não a consome nem a apaga.
        let linhas = netmonitor_core::models::event_outbox::Entity::find()
            .filter(
                netmonitor_core::models::_entities::event_outbox::Column::Origin
                    .eq("outro-processo"),
            )
            .count(&ctx.db)
            .await
            .expect("contar o outbox");
        assert_eq!(linhas, 1, "o ciclo mexeu no outbox de outro processo");
    })
    .await;
}
