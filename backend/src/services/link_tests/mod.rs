//! Orquestrador do teste de enlace (ADR 015 §5).
//!
//! Mede **quanto passa** num enlace (`link`), ao longo de um caminho, enlace
//! por enlace (`path`), ou do servidor até um equipamento (`server`). Cada
//! teste tem pernas; cada perna, etapas (`prepare_a`, `prepare_b`, `serve`,
//! `forward`, `reverse`, `stop`).
//!
//! | módulo | responsabilidade |
//! |---|---|
//! | [`model`] | contrato §5.1 (DTOs, `ts-rs`) |
//! | [`negotiate`] | escolha pura de motor e papéis; tradução de sentido |
//! | [`endpoint`] | as pontas: equipamento (plugin), central, agente |
//! | [`plan`] | o plano antes de rodar |
//! | [`transparent`] | equipamento sem IP: o teste passa por ele, não para nele |
//! | [`fixes`] | os passos que o plano oferece (instalar o plugin, cadastrar o acesso) |
//! | [`guard`] | um teste por dispositivo, no máximo dois no sistema |
//! | [`runner`] | a máquina de estados (pernas e etapas, cancelamento, limpeza) |
//! | [`outcome`] | desfecho de perna e teste, gargalo |
//! | [`repository`] / [`view`] | banco e linhas → contrato |
//! | [`events`] | SSE (`link_test:updated`, `link_test:sample`) |
//! | [`schedules`] | agendamentos e o disparo pelo agendador |
//! | [`janitor`] | limpeza no boot |
//! | [`env`] | os contratos que o orquestrador usa (plugins, motores, btest) |
//! | [`service`] | os casos de uso que os controllers chamam |

pub mod endpoint;
pub mod env;
pub mod events;
pub mod fixes;
pub mod guard;
pub mod janitor;
pub mod model;
pub mod negotiate;
pub mod outcome;
pub mod plan;
pub mod repository;
pub mod runner;
pub mod schedules;
pub mod service;
pub mod transparent;
pub mod view;

#[cfg(test)]
mod tests;

pub use env::LinkTestEnv;
