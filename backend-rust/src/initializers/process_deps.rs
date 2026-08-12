//! Dependências que existem **uma vez por processo**, em qualquer modo.
//!
//! Chamado de [`crate::app::App::after_context`], que é o único gancho do Loco
//! executado tanto no `start` quanto no `task`, no `scheduler` e no
//! `db migrate` — `loco_rs::boot::create_context` o invoca antes de tudo.
//!
//! **Por que não um `Initializer`.** O `run_task` do Loco não roda initializers
//! (`loco_rs::boot::run_task` só registra e executa a tarefa; quem chama
//! `H::before_run` e `initializer.before_run` é o `run_app`). Enquanto estas
//! dependências viveram num initializer, os processos `scheduler` e `probe`
//! subiam com o `shared_store` vazio: todo monitor de ping gravava `unknown`
//! com "Cliente ICMP não inicializado", e o relay de eventos reclamava a cada
//! ciclo. Aqui, não há modo que escape.

use loco_rs::app::AppContext;

use crate::services::{
    discovery::service::ScanSessionService, events::EventBus,
    monitoring::checkers::ping::PingClient,
};

/// Popula o `shared_store` do contexto.
///
/// Não devolve `Result` de propósito: um erro aqui abortaria **todo** comando,
/// inclusive `db migrate` e `doctor`. Ver a nota sobre o ICMP abaixo.
pub fn install(ctx: &AppContext) {
    // Puramente em memória, não falham.
    ctx.shared_store.insert(EventBus::create());
    ctx.shared_store.insert(ScanSessionService::create());

    // O socket ICMP é a única peça que pode falhar, e falha por configuração do
    // ambiente, não por bug: sem `net.ipv4.ping_group_range` liberado, o
    // `SOCK_DGRAM` volta `EACCES` (ADR 003). O container `migration` não recebe
    // esse sysctl — e não precisa, porque não pinga. Derrubar o `db migrate`
    // por causa disso travaria a stack inteira, então aqui o erro vira aviso.
    //
    // Quem realmente precisa do cliente descobre no lugar certo: o
    // `PingChecker::from_context` devolve erro nomeado, que vira a mensagem do
    // `monitor_results`.
    match PingClient::create() {
        Ok(client) => {
            ctx.shared_store.insert(client);
        }
        Err(error) => {
            tracing::warn!(
                %error,
                "cliente ICMP indisponível neste processo; monitores de ping locais vão falhar. \
                 Confira o sysctl net.ipv4.ping_group_range (ADR 003)."
            );
        }
    }
}
