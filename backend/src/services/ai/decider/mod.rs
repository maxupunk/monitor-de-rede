//! Decisor: decisões tipadas (sim/não, escolha, escala) servidas pelo Ollama
//! (`POST /v1/systemone`) — o mesmo container que atende o chat.
//!
//! Não é um LLM — é um classificador que responde várias perguntas numa
//! passada só, em dezenas de milissegundos. O NetMonitor o usa para decidir
//! antes do LLM do chat o que mandar para ele, deixando o chat focado no
//! conteúdo. Cada decisão implementa [`decision::Decision`].
//!
//! É opcional e nunca obrigatório: desligado ou fora do ar, o chat recebe
//! todas as ferramentas e as outras frentes seguem sem sugestão.

pub mod action_review;
pub mod benchmark;
pub mod bundled;
pub mod catalog;
pub mod client;
pub mod config;
pub mod decision;
pub mod decisions;
pub mod decisions_benchmark;
pub mod diagnostics;
pub mod locate;
pub mod protocols;
pub mod providers;
pub mod runtime;
pub mod schema;
pub mod suggestion;
pub mod tool_routing;
