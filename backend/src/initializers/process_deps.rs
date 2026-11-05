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
    events::{EventBus, Interests, Presence},
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
    // O que as abas estão olhando: decide se os agentes mandam o estado ao
    // vivo do Docker (ARQ-05).
    Interests::install(ctx);
    ApprovalDesk::install(ctx);
    // Os tipos de monitor das áreas (ADR 017, `MonitorChecker`). Na central
    // não há Docker Engine (ADR 014): o monitor de container responde que o
    // Docker daqui é do agente local quando a execução local de reserva o roda.
    ctx.shared_store
        .insert(crate::registries::monitor_checkers(None));
    // O que a área VPN pede ao resto da central (motor de alertas,
    // apresentação de dispositivo, endereços): o scheduler avalia os túneis.
    ctx.shared_store.insert(crate::registries::vpn_central());
    // O que a área Rede pede ao resto da central (motor de alertas): o
    // scheduler coleta o SNMP e avalia as interfaces.
    ctx.shared_store
        .insert(crate::registries::network_central());
    // O que as áreas fazem no ciclo do scheduler e na purga (registro de
    // rotinas) e os monitores que elas executam em grupo (o SNMP).
    ctx.shared_store.insert(crate::registries::area_routines());
    ctx.shared_store.insert(crate::registries::monitor_groups());
    // As políticas das áreas opcionais (o que desligar cada uma implica).
    ctx.shared_store
        .insert(netmonitor_core::areas::AreaPolicies::new(
            crate::registries::area_policies(),
        ));
    // O acesso ao equipamento que a Rede oferece ao núcleo (identidade SNMP,
    // Wi-Fi, credencial e shell da ativação do log).
    netmonitor_core::devices::access::install(ctx, crate::registries::device_access());
    // O lado central dos comandos das áreas aos agentes (a varredura).
    ctx.shared_store
        .insert(crate::registries::agent_command_dispatchers());
    // A IA (ADR 017, Fase 8): o classificador do núcleo pelo Decisor, o que o
    // assistente pede ao resto da central e o catálogo de ferramentas que as
    // áreas contribuem. Aqui, e não num initializer: o scheduler classifica
    // logs e roda tarefas agendadas com essas ferramentas.
    netmonitor_core::classifier::install(ctx, crate::registries::classifier());
    ctx.shared_store
        .insert(crate::registries::assistant_central());
    crate::registries::install_ai_tools();

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
