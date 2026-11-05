//! Regras de domínio do cadastro de dispositivos.

pub mod access;
pub mod capabilities;
pub mod identify;

/// Plataformas e dispositivo do sistema moram no núcleo (a VPN também os usa).
pub use netmonitor_core::devices::{adapters, kinds, system_device, systems};
