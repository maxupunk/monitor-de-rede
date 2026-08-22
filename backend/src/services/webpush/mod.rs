//! Módulo de notificações Web Push (VAPID, RFC 8030, RFC 8291, RFC 8292).

pub mod client;
pub mod crypto;
pub mod keys;

pub use client::{send_push, PushOutcome};
pub use crypto::{SubscriptionKeys, VapidKeyPair};
pub use keys::{get_or_create_vapid_keys, get_public_key};
