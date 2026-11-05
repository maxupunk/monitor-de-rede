//! Telemetria de host e containers.
//!
//! A leitura do host (`host_metrics`) roda no agente; o balde de 1 minuto
//! (`rollup`) é contrato do canal (`agent-protocol`); o histórico gravado
//! (`store`) é do núcleo. O amostrador mora no lado do agente
//! (`agent_runtime::sampler`).

pub mod host_metrics;

pub use agent_protocol::telemetry as rollup;
pub use netmonitor_core::telemetry::store;
