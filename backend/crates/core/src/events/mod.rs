//! Publicacao local de eventos de dominio e a entrega aos outros processos da
//! central (ADR 024).

pub mod audience;
pub mod bus;
mod counter;
pub mod fanout;
pub mod interest;
pub mod notice;
pub mod presence;

pub use audience::EventAudience;
pub use bus::{DomainEvent, EncodedEvent, EventBus};
pub use interest::Interests;
pub use presence::Presence;
