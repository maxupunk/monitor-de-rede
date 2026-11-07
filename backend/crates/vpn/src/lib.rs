//! Área VPN (ADR 017): WireGuard — chaves, IPAM, configuração, telemetria,
//! artefatos dos clientes (§8.10) e as rotas `/api/vpn`.
//!
//! A fronteira que organiza tudo aqui: o processo da API **nunca** executa `wg`
//! nem `docker exec`. Ele escreve `<iface>.conf` num diretório combinado e lê
//! `<iface>.status` do mesmo lugar; quem aplica e quem publica é o watcher, um
//! processo à parte. É isso que permite o processo da API rodar sem `NET_ADMIN`
//! mesmo quando o túnel sobe ao lado dele, no mesmo container.
//!
//! Depende só do núcleo (`netmonitor-core`) — nunca de outra área nem do crate
//! de montagem. O que esta área pede ao resto da central (motor de alertas,
//! apresentação de dispositivo, endereços da central) chega pelo
//! [`central::CentralServices`]; o que o núcleo precisa dela, pela
//! [`VpnPolicy`] (`AreaPolicy`). Os dois são montados pelo ponto de montagem.

pub mod access_control;
pub mod ai_tools;
pub mod alerts;
pub mod area_policy;
pub mod central;
pub mod cidr;
pub mod config_builder;
pub mod config_writer;
pub mod controllers;
pub mod ip_allocator;
pub mod key_generator;
pub mod migrations;
pub mod monitor_provisioner;
pub mod peer_hints;
pub mod peer_name;
pub mod peer_service;
pub mod peer_status;
pub mod preflight;
pub mod profiles;
pub mod routines;
pub mod server_service;
pub mod shell_escape;
pub mod traffic_recorder;
pub mod views;

/// O cofre efêmero das chaves privadas mora no núcleo (o enrollment dos
/// agentes e as senhas de sessão dos plugins usam o mesmo); o caminho da área
/// continua valendo.
pub use netmonitor_core::shared::secret_store;
/// A topologia do `vpn-probe` é do núcleo (registro no boot, troca de banco).
pub use netmonitor_core::vpn_probe::probe_is_external;

pub use area_policy::VpnPolicy;
pub use profiles::{GeneratedArtifact, PERSISTENT_KEEPALIVE_SECONDS, PRIVATE_KEY_UNAVAILABLE};
pub use secret_store::client_key_store;
