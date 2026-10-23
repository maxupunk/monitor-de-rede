//! Tarefas agendadas: uma ferramenta do catálogo da IA, com os argumentos,
//! rodada numa expressão cron.
//!
//! Humanos (tela `/scheduled-tasks`) e IA (grupo `schedules` do chat) usam o
//! mesmo cadastro e o mesmo catálogo — o das ferramentas que rodam sem
//! ninguém do outro lado ([`UnattendedTools`]): consultas, testes ativos,
//! ações e containers; nunca acesso a equipamento nem pergunta ao usuário.
//!
//! - [`schedule`]: a expressão cron, as próximas execuções e a frase.
//! - [`arguments`]: os argumentos conferidos contra o JSON Schema.
//! - [`service`]: catálogo, conferência, cadastro e histórico.
//! - [`runner`]: o laço que roda o que venceu e o "rodar agora".
//!
//! [`UnattendedTools`]: crate::services::ai::harness::tools::UnattendedTools

pub mod arguments;
pub mod events;
pub mod model;
pub mod runner;
pub mod schedule;
pub mod service;
pub mod view;

pub use service::Author;
