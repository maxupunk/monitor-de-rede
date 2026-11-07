//! O contrato dos canais de notificação (§8.9). Os canais, a política e a
//! fila continuam fora do núcleo; o contrato mora aqui para que quem só
//! **pede** um envio (o resumo da IA) não dependa deles.

pub mod contracts;

pub use contracts::{NotificationChannel, NotificationMessage, Severity};
