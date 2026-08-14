//! Dois níveis de inicialização, e a diferença importa:
//!
//! * [`process_deps`] — roda em **todo** modo, via `Hooks::after_context`.
//!   Dependências que qualquer processo precisa (socket ICMP, barramento de
//!   eventos, sessão de scan).
//! * [`monitoring`] — roda **só no servidor**, via `Initializer`. Conveniências
//!   de boot que dependem do banco e do processo que atende HTTP.

pub mod monitoring;
pub mod process_deps;
pub mod setup;
