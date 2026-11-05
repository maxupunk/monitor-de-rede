//! Motor de alertas (§8.7).
//!
//! A divisão é em três: quem observa a rede publica *fatos* (`datasets`), o
//! `evaluator` decide se um fato satisfaz uma regra, e o `manager` transforma
//! isso em `alert_events`, notificação e evento SSE.

pub mod actions;
pub mod adaptive_latency;
pub mod baseline;
pub mod catalog;
pub mod correlation;
pub mod datasets;
pub mod episode;
pub mod evaluator;
pub mod fields;
pub mod hysteresis;
pub mod inhibition;
pub mod instability;
pub mod manager;
pub mod problem_kind;
pub mod recovery;
pub mod repository;
pub mod rules;
pub mod silence;
pub mod state_machine;
pub mod vpn_peer_watcher;

/// Contratos e feed SSE dos alertas moram no núcleo (a IA também os usa).
pub use netmonitor_core::alerts::{contracts, feed};

pub use contracts::{AlertEvaluationContext, AlertEvaluationScope, AlertScopeKey};
