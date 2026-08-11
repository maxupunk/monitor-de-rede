//! WireGuard: chaves, IPAM, configuração, telemetria e artefatos (§8.10).
//!
//! A fronteira que organiza tudo aqui: o servidor **nunca** executa `wg` nem
//! `docker exec`. Ele escreve `<iface>.conf` num volume compartilhado e lê
//! `<iface>.status` do mesmo volume; quem aplica e quem publica é o container
//! do WireGuard. É isso que permite o processo da API rodar sem `NET_ADMIN`.

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
