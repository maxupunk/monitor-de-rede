//! Notificações de alerta: contrato, formatação e os quatro canais (§8.9).
//!
//! Desde a Fase 4 do roadmap de alertas inteligentes o caminho tem três etapas
//! em vez de uma: o motor **enfileira** ([`outbox::enqueue`]), a [`policy`]
//! pura decide se a mensagem sai, espera o agrupamento ou é engolida, e o ciclo
//! do scheduler **despacha** ([`outbox::dispatch_pending`]). O
//! [`NotificationService`] continua sendo o que fala com os canais — o que
//! mudou é quem o chama e quando.

pub mod channels;
pub mod contracts;
pub mod formatter;
pub mod http_channel;
pub mod outbox;
pub mod policy;
pub mod service;

pub use contracts::{NotificationChannel, NotificationMessage, Severity};
pub use policy::{NotificationKind, NotificationPolicy};
pub use service::NotificationService;
