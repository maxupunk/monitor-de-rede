//! Contratos independentes de HTTP para as verificações de monitoramento.
//!
//! O resultado de uma medição atravessa o canal do agente (`Command::Monitor`)
//! e por isso mora no contrato de fio (`agent-protocol`); o trait dos checkers
//! é do núcleo, que as áreas também implementam.

pub use agent_protocol::monitoring::{CheckMetric, CheckResult, MonitorStatus};
pub use netmonitor_core::monitoring::Checker;
