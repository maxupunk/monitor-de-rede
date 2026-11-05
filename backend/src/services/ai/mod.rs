//! O assistente é um crate (`netmonitor-assistant`, ADR 017, Fase 8); o
//! caminho antigo continua valendo para os controllers, as views e os testes.
//! As ferramentas que a IA chama não moram aqui: o contrato é do núcleo e
//! cada área contribui as suas (`services::ai_tools`, `netmonitor_servers::
//! ai_tools`, `netmonitor_vpn::ai_tools`…), montadas em `crate::registries`.

pub use netmonitor_assistant::*;
