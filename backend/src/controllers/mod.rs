pub mod agents;
pub mod alerts;
pub mod areas;
pub mod audit;
pub mod auth;
pub mod auth_guard;
pub mod backup;
pub mod dashboard;
pub mod devices;
pub mod diagnostics;
pub mod dns;
pub mod dns_servers;
pub mod events;
pub mod logs;
pub mod maintenance_windows;
pub mod monitors;
pub mod networks;
pub mod probes;
pub mod push;
pub mod root;
pub mod scheduled_tasks;
pub mod server_addresses;
pub mod settings;
pub mod sites;
pub mod storages;
pub mod users;

/// As rotas do assistente moram no crate dele (`netmonitor-assistant`).
pub use netmonitor_assistant::controllers::{ai, ai_conversations};
