//! O que as áreas precisam para contribuir com a IA (ADR 017, Fase 8).
//!
//! O assistente (`netmonitor-assistant`) é quem conversa com o modelo; as
//! ferramentas que ele chama são de cada área. Por isso o contrato mora aqui:
//! [`tools::AiToolHandler`] e a política que libera cada ferramenta
//! ([`tools::ToolPolicy`]), os argumentos ([`args::ToolArgs`]), o risco
//! ([`risk`]), o gráfico que a ferramenta devolve ([`chart`]), a fonte do
//! `grep` ([`grep::GrepSource`]) e a seção do estado da plataforma
//! ([`platform::PlatformSection`]). [`lookup`] e [`series`] são as utilidades
//! que toda ferramenta usa para achar dispositivo e resumir série; a
//! documentação que a IA consulta fica em [`knowledge`].

pub mod args;
pub mod automation;
pub mod chart;
pub mod function;
pub mod grep;
pub mod knowledge;
pub mod lookup;
pub mod platform;
pub mod risk;
pub mod series;
pub mod tools;
