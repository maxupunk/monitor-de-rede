//! Dois níveis de inicialização, e a diferença importa:
//!
//! * [`process_deps`] — roda em **todo** modo, via `Hooks::after_context`.
//!   Dependências que qualquer processo precisa (socket ICMP, barramento de
//!   eventos, sessão de scan).
//! * [`ai`], [`monitoring`], [`syslog`] e [`btest`] — rodam **só no servidor**, via
//!   `Initializer`. Conveniências de boot e laços que dependem do processo que
//!   atende HTTP — ou, no caso do syslog e do btest, que abre porta.

pub mod ai;
pub mod btest;
pub mod monitoring;
pub mod process_deps;
pub mod setup;
pub mod syslog;
pub mod system_device;
