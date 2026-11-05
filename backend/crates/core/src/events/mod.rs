//! Publicacao local e relay persistente de eventos de dominio.

pub mod bus;
mod counter;
pub mod interest;
pub mod presence;
pub mod relay;

pub use bus::{DomainEvent, EventBus};
pub use interest::Interests;
pub use presence::Presence;
