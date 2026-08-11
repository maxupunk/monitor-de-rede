//! Motor de alertas (§8.7).
//!
//! A divisão espelha a do backend AdonisJS: quem observa a rede publica
//! *fatos* (`datasets`), o `evaluator` decide se um fato satisfaz uma regra, e
//! o `manager` transforma isso em `alert_events`, notificação e evento SSE.

pub mod catalog;
pub mod contracts;
pub mod datasets;
pub mod evaluator;
pub mod fields;
pub mod manager;
pub mod recovery;
pub mod repository;
pub mod silence;

pub use contracts::{AlertEvaluationContext, AlertEvaluationScope, AlertScopeKey};
