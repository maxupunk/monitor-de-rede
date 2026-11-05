//! Núcleo do monitoramento: execução, persistência e apresentação.

pub mod bandwidth_latency;
pub mod checkers;
pub mod contracts;
pub mod creation;
pub mod health;
pub mod heatmap;
pub mod ip_reconciliation;
pub mod managed;
pub mod ping_diagnostics;
pub mod presenter;
pub mod result_processor;
pub mod rollup;
pub mod runner;
pub mod saas;
pub mod scheduler;
pub mod timeseries;
pub mod uptime;

/// Quem pode medir o quê mora no núcleo (a VPN também consulta).
pub use netmonitor_core::monitoring::{
    device_status, execution_guard, link_speed, metrics_repository, reachability,
};
