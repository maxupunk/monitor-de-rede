//! Área Servidores (ADR 017): Docker de todo host (pelo agente, ADR 014),
//! o monitor de container, o backup de bancos de terceiros (ADR 013) e as
//! rotas e telas que os servem.
//!
//! Depende só do núcleo (`netmonitor-core`) e do contrato de fio
//! (`agent-protocol`) — nunca de outra área nem do crate de montagem. O que o
//! núcleo precisa desta área chega por ponto de extensão: o
//! [`ContainerMonitor`] (`MonitorChecker`) e a [`ServersPolicy`]
//! (`AreaPolicy`), registrados pelo ponto de montagem.
//!
//! * [`docker`] — a Engine (só no agente) e o Docker de cada host pelo canal;
//! * [`agent_host`] — o lado do agente: os comandos Docker
//!   (`AgentCommandHandler`) e o Docker do host para o runtime dele;
//! * [`ai_tools`] — as ferramentas `docker_*` da IA e a fonte `docker` do
//!   `grep`, contribuídas ao catálogo pelo ponto de montagem;
//! * [`databases`] — dump e restauração nativos, pela ponte do agente;
//! * [`controllers`], [`dtos`], [`views`] — as rotas `/api/docker` e
//!   `/api/databases`.

pub mod agent_host;
pub mod ai_tools;
pub mod area_policy;
pub mod container_monitor;
pub mod controllers;
pub mod databases;
pub mod docker;
pub mod dtos;
pub mod views;
pub mod widgets;

use std::sync::Arc;

use netmonitor_core::monitoring::MonitorChecker;

pub use area_policy::ServersPolicy;
pub use container_monitor::ContainerMonitor;

/// Os tipos de monitor desta área, para o registro do núcleo. A Engine só
/// existe no agente (ADR 014): a central passa `None`.
#[must_use]
pub fn monitor_checkers(
    docker: Option<Arc<dyn docker::source::DockerEngine>>,
) -> Vec<Arc<dyn MonitorChecker>> {
    vec![Arc::new(ContainerMonitor::new(docker))]
}
