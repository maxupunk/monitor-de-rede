//! Plugins de dispositivo — "drivers" que agem sobre o equipamento.
//!
//! Um plugin é um pacote (`.nmplugin`) com manifesto, script Rhai, documentação
//! de uso, lista de compatibilidade e testes. O script roda num sandbox
//! ([`runtime`]) que não enxerga disco, processo nem rede: toda E/S passa pelo
//! [`transport::DeviceTransport`], que só alcança o equipamento ao qual o
//! plugin está sendo aplicado. É esse ponto único que permite auditar,
//! aprovar ([`gate`]) e simular ([`transport::fake`]) cada acesso.
//!
//! ```text
//! pacote ─► manifest::validate ─► review (quarentena de importação)
//!        ─► testing (fixtures) ─► runs (execução real, com gate) ─► compatibilidade
//! ```
//!
//! O Rhai roda **sempre na central**. Quando o equipamento só é alcançado por
//! um agente remoto, o agente recebe apenas operações unitárias de E/S
//! (`Command::DeviceIo`), nunca o script — o enum do protocolo continua
//! fechado, sem interpretador genérico do outro lado.

pub mod actions;
pub mod auto_accept;
pub mod builtin;
pub mod compat;
pub mod credentials;
pub mod effect;
pub mod fingerprint;
pub mod fleet;
pub mod gate;
pub mod manifest;
pub mod package;
pub mod params;
pub mod review;
pub mod runs;
pub mod runtime;
pub mod service;
pub mod settings;
pub mod testing;
pub mod transport;
pub mod version;
