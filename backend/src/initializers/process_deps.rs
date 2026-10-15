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
    agents::hub::AgentHub,
    ai::approvals::ApprovalDesk,
    discovery::service::ScanSessionService,
    docker::log_stream::LogStreams,
    events::{EventBus, Presence},
    monitoring::checkers::ping::PingClientCell,
};

/// Popula o `shared_store` do contexto.
///
/// Não devolve `Result` de propósito: um erro aqui abortaria **todo** comando,
/// inclusive `db migrate` e `doctor`. Ver a nota sobre o ICMP abaixo.
pub fn install(ctx: &AppContext) {
    // Puramente em memória, não falham.
    let bus = EventBus::create();
    ctx.shared_store.insert(bus.clone());
    ctx.shared_store.insert(ScanSessionService::create(bus));
    // Registro das conexões de agentes e dos logs em follow. Só o processo da
    // API os povoa, mas existir vazio em todo processo evita um "não
    // inicializado" em quem apenas consulta (ADR 011).
    AgentHub::install(ctx);
    LogStreams::install(ctx);
    // Quem está com o NetMonitor aberto (stream SSE): decide se um pedido de
    // aprovação da IA vira notificação no celular.
    Presence::install(ctx);
    ApprovalDesk::install(ctx);

    // O socket ICMP é a única peça que pode falhar, e falha por configuração do
    // ambiente, não por bug: sem `net.ipv4.ping_group_range` liberado, o
    // `SOCK_DGRAM` volta `EPERM` (ADR 003). Derrubar o `db migrate` por causa
    // disso travaria a stack inteira, então aqui o erro vira aviso — e a célula
    // tenta de novo a cada uso, para o ping voltar sozinho quando o sysctl for
    // liberado, sem reiniciar o processo.
    //
    // Quem realmente precisa do cliente descobre no lugar certo: o
    // `PingClient::from_context` devolve erro nomeado, que vira a mensagem do
    // `monitor_results` e da descoberta.
    ctx.shared_store.insert(PingClientCell::open());
}
