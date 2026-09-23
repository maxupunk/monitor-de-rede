//! Telemetria de host e containers: amostragem, agregação em minutos e
//! histórico. Compartilhado pela central (host `local`) e pelo agente remoto.

pub mod host_metrics;
pub mod rollup;
pub mod sampler;
pub mod store;
