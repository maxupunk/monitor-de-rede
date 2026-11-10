//! Blocos compartilhados por todos os módulos de domínio (§8.1).

pub mod backup_schedule;
pub mod body;
pub mod cidr;
pub mod client_addr;
pub mod crypto;
pub mod errors;
pub mod incident;
pub mod pagination;
pub mod rate_limit;
pub mod run_guard;
pub mod runtime;
pub mod secret_file;
pub mod secret_store;
pub mod sql_like;
pub mod ssh;
pub mod text;
pub mod transaction;
pub mod units;
