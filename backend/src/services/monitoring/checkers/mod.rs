//! Checkers de rede. Cada um degrada o resultado em vez de propagar falhas de rede.

pub mod dns;
pub mod host_resources;
pub mod http;
pub mod ping;
pub mod system_health;
pub mod tcp;
