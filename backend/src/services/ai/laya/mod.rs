//! Laya: decisões tipadas (sim/não, escolha, escala) servidas pelo Ollaya.
//!
//! Não é um LLM — é um classificador que responde várias perguntas numa
//! passada só, em dezenas de milissegundos. O NetMonitor o usa para decidir
//! antes do LLM do chat o que mandar para ele, deixando o chat focado no
//! conteúdo. Cada decisão implementa [`decision::Decision`].
//!
//! É opcional e nunca obrigatório: desligado ou fora do ar, o chat segue com
//! as heurísticas de sempre.

pub mod catalog;
pub mod client;
pub mod config;
pub mod decision;
pub mod decisions;
pub mod diagnostics;
pub mod runtime;
pub mod schema;
pub mod suggestion;
pub mod tool_routing;
