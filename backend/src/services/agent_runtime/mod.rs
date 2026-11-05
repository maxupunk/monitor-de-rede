//! O lado do agente: o que roda no servidor remoto (binário
//! `netmonitor-agent`, ADR 011).
//!
//! Não sobe o Loco nem abre banco. Reaproveita da central o protocolo
//! ([`crate::services::agents::protocol`]), os checkers (`run_monitor_with`)
//! e a telemetria. O que é de área — o Docker do host (Servidores), a
//! descoberta, o equipamento e a vazão (Rede) — chega montado pelo binário
//! ([`client::AgentHost`]), sem este runtime conhecer área nenhuma.

pub mod client;
pub mod config;
pub mod executor;
pub mod identity;
pub mod outbox;
pub mod sampler;
pub mod session;
pub mod tunnel;
