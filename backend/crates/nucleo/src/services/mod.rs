//! Módulos de domínio.
//!
//! A regra da casa: o controller extrai, valida, delega e serializa. Toda regra
//! de negócio vive aqui e é testável sem HTTP.

/// Barramento de eventos e utilidades transversais moram no núcleo
/// (`crates/core`); os caminhos `services::events` e `services::shared` continuam.
pub use netmonitor_core::{events, preferences, shared};

pub mod agent_runtime;
pub mod agents;
pub mod ai_tools;
pub mod alerts;
pub mod audit;
pub mod auth;
pub mod backup;
pub mod dashboard;
pub mod devices;
pub mod maintenance;
pub mod maintenance_windows;
pub mod menu_prefs;
pub mod monitoring;
pub mod network_tools;
pub mod notifications;
pub mod onboarding;
pub mod probes;
pub mod scheduled_tasks;
pub mod server_addresses;
pub mod settings;
pub mod storage;
pub mod syslog;
pub mod telemetry;
pub mod users;
pub mod webpush;
