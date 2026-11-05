//! O que cada área opcional sabe sobre si mesma (ADR 017): evidência de uso,
//! consequências de desligar e bloqueios.
//!
//! Moram aqui só enquanto as áreas não têm crate próprio; cada política vai
//! junto com a sua área quando ela for extraída (a de Servidores já está no
//! crate `servers`; Rede e VPN seguem nas Fases 7 e 9). Quem as junta num
//! registro é o ponto de montagem (`crate::registries`).

mod network;
mod vpn;

pub use network::NetworkPolicy;
pub use vpn::VpnPolicy;
