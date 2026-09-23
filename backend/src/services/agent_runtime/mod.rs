//! O lado do agente: o que roda no servidor remoto (binário
//! `netmonitor-agent`, ADR 011).
//!
//! Não sobe o Loco nem abre banco. Reaproveita da central o protocolo
//! ([`crate::services::agents::protocol`]), a Engine local
//! ([`crate::services::docker::source::LocalEngine`]), os checkers
//! (`run_monitor_with`) e a telemetria.

pub mod client;
pub mod config;
pub mod executor;
pub mod identity;
pub mod outbox;
pub mod session;
