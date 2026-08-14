//! Geradores de configuração por perfil de equipamento (§8.10.5).
//!
//! O conteúdo dos scripts reproduz fielmente os perfis validados em hardware
//! real; qualquer "melhoria" aqui sem um equipamento na mão é aposta.

pub mod contract;
pub mod mikrotik;
pub mod openwrt;
pub mod registry;
pub mod variants;
pub mod wg_conf;

pub use contract::{
    GeneratedArtifact, PeerConfigContext, VpnProfileGenerator, PERSISTENT_KEEPALIVE_SECONDS,
    PRIVATE_KEY_UNAVAILABLE,
};
