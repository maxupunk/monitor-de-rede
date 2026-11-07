//! Publicacao local e relay persistente de eventos de dominio.

pub mod audience;
pub mod bus;
mod counter;
pub mod interest;
pub mod presence;
pub mod relay;

pub use audience::EventAudience;
pub use bus::{DomainEvent, EventBus};
pub use interest::Interests;
pub use presence::Presence;
