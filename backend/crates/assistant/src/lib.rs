//! O assistente do NetMonitor (ADR 017, Fase 8): o chat com ferramentas
//! (`harness`), os provedores de LLM (`drivers`, `ollama`, `openrouter`,
//! `opencode`, `local_server`, `local_models`), o Decisor (`decider`), as
//! rotinas automáticas (`proactive`), as marcações do `@` (`mentions`), as
//! conversas salvas (`conversations`) e a exportação delas (`data_export`), a pesquisa na web (`web_search`) e as
//! rotas `/api/ai`.
//!
//! Depende **só** do núcleo (`netmonitor-core`) — nunca de uma área nem do
//! crate de montagem. As ferramentas que a IA chama são contribuídas pelas
//! áreas pelo contrato do núcleo (`netmonitor_core::ai::tools::AiToolHandler`);
//! o catálogo é montado no ponto de montagem. O que o núcleo pede ao
//! assistente chega por ponto de extensão: o [`DeciderClassifier`]
//! (`netmonitor_core::classifier::Classifier`). O que o assistente pede ao
//! resto da central, pelo [`central::CentralServices`].

pub mod approvals;
pub mod central;
pub mod chat_benchmark;
pub mod classifier;
pub mod context_window;
pub mod controllers;
pub mod conversations;
pub mod data_export;
pub mod decider;
pub mod drivers;
pub mod dtos;
pub mod harness;
pub mod local_models;
pub mod local_server;
pub mod mentions;
pub mod migrations;
pub mod ollama;
pub mod opencode;
pub mod openrouter;
pub mod proactive;
pub mod response_style;
pub mod settings;
pub mod tools;
pub mod web_search;

/// A documentação que a IA consulta mora no núcleo (as ferramentas das áreas
/// também a citam).
pub use netmonitor_core::ai::knowledge;

pub use classifier::DeciderClassifier;
