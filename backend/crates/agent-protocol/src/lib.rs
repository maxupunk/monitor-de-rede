//! Contrato de fio entre a central e o agente remoto (ADR 011).
//!
//! O que atravessa o canal — envelopes, comandos, eventos, erros, quadros de
//! ponte — e os tipos simples que eles carregam. Os dois lados (o binário
//! `netmonitor-agent` e a central) usam este crate, e ele não depende de
//! nenhum outro do workspace: nem do núcleo, nem de área (ADR 017,
//! `core ──▶ agent-protocol`). Conversões que dependem de regra de uma área
//! moram do lado dela.
//!
//! * [`protocol`] (reexportado na raiz) — envelope, comandos e erros;
//! * [`policy`] — o que cada host aceita, decidido **no host**;
//! * [`tunnel`] — a bomba de bytes das pontes (ADR 013), igual nas duas pontas;
//! * [`docker`], [`monitoring`], [`throughput`], [`device_io`], [`probes`] e
//!   [`telemetry`] — os tipos que os comandos e eventos carregam.

pub mod device_io;
pub mod docker;
pub mod monitoring;
pub mod policy;
pub mod probes;
pub mod protocol;
pub mod telemetry;
pub mod throughput;
pub mod tunnel;

pub use protocol::*;
