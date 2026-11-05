//! Núcleo do NetMonitor (ADR 017): os serviços, as rotas e as telas da área
//! que está sempre ligada — monitoramento e o ciclo do scheduler, alertas,
//! notificações (canais e Web Push), dispositivos, logs (syslog), sondas e
//! agentes (cadastro, eventos e o runtime do host do agente), usuários e
//! autenticação, configurações, dashboard, tarefas agendadas, backup, as
//! ferramentas da IA do núcleo e o assistente inicial.
//!
//! Depende só do `core` (contratos, modelo de dados e o que as áreas também
//! usam) e do contrato de fio (`agent-protocol`). Nenhuma área depende dele: o
//! que uma área pede à central chega pelo `CentralServices` dela, implementado
//! no ponto de montagem com os serviços daqui. Por isso mexer aqui recompila
//! só este crate e o ponto de montagem.
//!
//! * [`services`] — o domínio, testável sem HTTP;
//! * [`controllers`] — extraem, validam, delegam e serializam;
//! * [`dtos`], [`views`] — entrada e saída;
//! * [`mailers`] — os e-mails da autenticação.

pub mod controllers;
pub mod dtos;
pub mod mailers;
pub mod services;
pub mod views;

/// O modelo de dados mora no `core`; o caminho `crate::models` continua valendo
/// dentro deste crate.
pub use netmonitor_core::models;

/// A versão que a central declara (`Hooks::app_version`), gravada nos
/// backups: a do pacote e o commit do build, quando houver.
#[must_use]
pub fn app_version() -> String {
    format!(
        "{} ({})",
        env!("CARGO_PKG_VERSION"),
        option_env!("BUILD_SHA")
            .or(option_env!("GITHUB_SHA"))
            .unwrap_or("dev")
    )
}
