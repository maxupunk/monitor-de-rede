//! Notificações de alerta: contrato, formatação e os quatro canais (§8.9).

pub mod channels;
pub mod contracts;
pub mod formatter;
pub mod http_channel;
pub mod service;

pub use contracts::{NotificationChannel, NotificationMessage, Severity};
pub use service::NotificationService;
