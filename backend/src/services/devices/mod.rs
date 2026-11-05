//! Regras de domínio do cadastro de dispositivos.

pub mod access;
pub mod capabilities;
pub mod identify;
pub mod kinds;
pub mod systems;

/// Plataformas e dispositivo do sistema moram no núcleo (a VPN também os usa).
pub use netmonitor_core::devices::{adapters, system_device};
