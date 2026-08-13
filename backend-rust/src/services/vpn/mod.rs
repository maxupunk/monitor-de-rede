//! WireGuard: chaves, IPAM, configuração, telemetria e artefatos (§8.10).
//!
//! A fronteira que organiza tudo aqui: o processo da API **nunca** executa `wg`
//! nem `docker exec`. Ele escreve `<iface>.conf` num diretório combinado e lê
//! `<iface>.status` do mesmo lugar; quem aplica e quem publica é o watcher, um
//! processo à parte. É isso que permite o processo da API rodar sem `NET_ADMIN`
//! mesmo quando o túnel sobe ao lado dele, no mesmo container.

pub mod access_control;
pub mod cidr;
pub mod config_builder;
pub mod config_writer;
pub mod ip_allocator;
pub mod key_generator;
pub mod monitor_provisioner;
pub mod peer_hints;
pub mod peer_service;
pub mod peer_status;
pub mod preflight;
pub mod probe_registrar;
pub mod profiles;
pub mod secret_store;
pub mod server_service;
pub mod state_watcher;
pub mod traffic_recorder;

pub use profiles::{GeneratedArtifact, PERSISTENT_KEEPALIVE_SECONDS, PRIVATE_KEY_UNAVAILABLE};
pub use secret_store::client_key_store;

/// O túnel sobe em **outro** namespace de rede, alcançável só pelo agente
/// dedicado (`vpn-probe`)?
///
/// `false` — o padrão — descreve a topologia de um container só: o WireGuard
/// sobe ao lado da API e a `wg0` é do próprio processo, que portanto alcança a
/// faixa do túnel sem intermediário.
///
/// Duas decisões dependem desta resposta e precisam ser a mesma:
///
/// * registrar (ou não) o `vpn-probe` no boot — `initializers::monitoring`;
/// * como ler um ping que falha com o túnel de pé — [`peer_hints`].
#[must_use]
pub fn probe_is_external() -> bool {
    std::env::var("VPN_PROBE_EXTERNAL").is_ok_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "true" | "1" | "yes" | "on"
        )
    })
}
