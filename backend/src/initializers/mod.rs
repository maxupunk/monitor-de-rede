//! Dois níveis de inicialização, e a diferença importa:
//!
//! * [`process_deps`] — roda em **todo** modo, via `Hooks::after_context`.
//!   Dependências que qualquer processo precisa (socket ICMP, barramento de
//!   eventos, sessão de scan).
//! * [`monitoring`] e [`syslog`] — rodam **só no servidor**, via `Initializer`.
//!   Conveniências de boot e laços que dependem do processo que atende HTTP —
//!   ou, no caso do syslog, que abre porta.

pub mod monitoring;
pub mod process_deps;
pub mod setup;
pub mod syslog;
