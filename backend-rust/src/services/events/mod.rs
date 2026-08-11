//! Publicacao local e relay persistente de eventos de dominio.

pub mod bus;
pub mod relay;

pub use bus::{DomainEvent, EventBus};
