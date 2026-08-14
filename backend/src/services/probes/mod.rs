//! Probes: fila de tarefas, vida do agente e o agente em si (§8.11).

pub mod agent;
pub mod buffer;
pub mod dispatcher;
pub mod liveness;
pub mod receiver;

pub use dispatcher::{ProbeTask, TASK_TTL_SECONDS};
pub use liveness::{is_probe_alive, PROBE_OFFLINE_AFTER_SECONDS};

/// Token compartilhado do agente que roda no namespace do WireGuard.
///
/// ⚠️ É **compartilhado de propósito**: o container `vpn-probe` sobe sem
/// configuração e precisa se autenticar. É também a razão de
/// `probes.token_hash` não ter índice único (§6 #03) — mais de um agente pode
/// responder pelo mesmo hash. O registrador da Fase 8 (`vpn_probe_registrar`)
/// reutiliza esta constante em vez de redeclará-la.
pub const DEFAULT_VPN_PROBE_TOKEN: &str = "default_vpn_probe_token";
