//! Serviços de manutenção de recursos.

pub mod data_pruner;

/// A remoção de agregados mora no núcleo (a VPN também a usa).
pub use netmonitor_core::maintenance::resource_cleanup;
