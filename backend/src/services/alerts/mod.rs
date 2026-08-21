//! Motor de alertas (§8.7).
//!
//! A divisão é em três: quem observa a rede publica *fatos* (`datasets`), o
//! `evaluator` decide se um fato satisfaz uma regra, e o `manager` transforma
//! isso em `alert_events`, notificação e evento SSE.

pub mod baseline;
pub mod catalog;
pub mod contracts;
pub mod correlation;
pub mod datasets;
pub mod episode;
pub mod evaluator;
pub mod feed;
pub mod fields;
pub mod hysteresis;
pub mod inhibition;
pub mod instability;
pub mod manager;
pub mod problem_kind;
pub mod recovery;
pub mod repository;
pub mod silence;
pub mod state_machine;

pub use contracts::{AlertEvaluationContext, AlertEvaluationScope, AlertScopeKey};
