//! Configuração do agente, lida do ambiente do host remoto.
//!
//! | Variável              | Padrão                          |
//! |-----------------------|---------------------------------|
//! | `AGENT_SERVER_URL`    | obrigatória                     |
//! | `AGENT_ENROLL_CODE`   | usada só se ainda não há token  |
//! | `AGENT_TOKEN`         | lido de `AGENT_TOKEN_FILE`      |
//! | `AGENT_TOKEN_FILE`    | `<state>/token`; o agente local lê o que a central grava (ADR 014) |
//! | `AGENT_STATE_DIR`     | `/var/lib/netmonitor-agent`     |
//! | `AGENT_ALLOW`         | `read,lifecycle,monitor,discovery` (`device_io` libera plugins de dispositivo) |
//! | `AGENT_DEVICE_CIDRS`  | vazio = qualquer rede privada; restringe o alvo dos plugins |
//! | `AGENT_DATABASE_TARGETS` | vazio = nenhum; `IP:porta` ou `faixa:porta` liberados para a ponte de banco (exige `database` em `AGENT_ALLOW`) |
//! | `AGENT_OUTBOX_MAX`    | 10 000 eventos                  |
//! | `HOST_PROC`/`HOST_ROOT` | `/proc` e `/`                 |

use std::path::PathBuf;

use crate::services::agents::policy::Policy;

use super::tunnel::{parse_targets, TunnelTarget};

pub const DEFAULT_STATE_DIR: &str = "/var/lib/netmonitor-agent";
const DEFAULT_OUTBOX_MAX: usize = 10_000;

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub server_url: String,
    pub enroll_code: Option<String>,
    pub token: Option<String>,
    /// Onde o token fica (ou de onde é lido, no agente local).
    pub token_file: Option<PathBuf>,
    pub state_dir: PathBuf,
    pub policy: Policy,
    pub outbox_max: usize,
    /// Bancos que a ponte pode alcançar (ADR 013).
    pub database_targets: Vec<TunnelTarget>,
}

fn env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

impl AgentConfig {
    /// # Errors
    ///
    /// URL da central ausente ou política com permissão desconhecida.
    pub fn from_env() -> Result<Self, String> {
        let server_url = env("AGENT_SERVER_URL")
            .ok_or("AGENT_SERVER_URL é obrigatória (ex.: http://10.8.0.1:3333)")?;
        let policy = Policy::parse(&env("AGENT_ALLOW").unwrap_or_default())?;
        let database_targets = parse_targets(&env("AGENT_DATABASE_TARGETS").unwrap_or_default())?;
        Ok(Self {
            server_url: normalize_server_url(&server_url)?,
            enroll_code: env("AGENT_ENROLL_CODE"),
            token: env("AGENT_TOKEN"),
            token_file: env("AGENT_TOKEN_FILE").map(PathBuf::from),
            state_dir: env("AGENT_STATE_DIR")
                .map_or_else(|| DEFAULT_STATE_DIR.into(), PathBuf::from),
            policy,
            outbox_max: env("AGENT_OUTBOX_MAX")
                .and_then(|value| value.parse().ok())
                .filter(|value| *value > 0)
                .unwrap_or(DEFAULT_OUTBOX_MAX),
            database_targets,
        })
    }

    #[must_use]
    pub fn token_path(&self) -> PathBuf {
        self.token_file
            .clone()
            .unwrap_or_else(|| self.state_dir.join("token"))
    }

    #[must_use]
    pub fn outbox_path(&self) -> PathBuf {
        self.state_dir.join("outbox.json")
    }

    /// Endereço do canal: `http` vira `ws`, `https` vira `wss`.
    #[must_use]
    pub fn websocket_url(&self) -> String {
        websocket_url(&self.server_url)
    }
}

/// # Errors
///
/// Esquema diferente de http/https.
pub fn normalize_server_url(raw: &str) -> Result<String, String> {
    let url = raw.trim().trim_end_matches('/').to_string();
    if url.starts_with("http://") || url.starts_with("https://") {
        Ok(url)
    } else {
        Err(format!(
            "AGENT_SERVER_URL precisa começar com http:// ou https://: {raw}"
        ))
    }
}

#[must_use]
pub fn websocket_url(server_url: &str) -> String {
    let base = server_url
        .strip_prefix("https://")
        .map(|rest| format!("wss://{rest}"))
        .or_else(|| {
            server_url
                .strip_prefix("http://")
                .map(|rest| format!("ws://{rest}"))
        })
        .unwrap_or_else(|| server_url.to_string());
    format!("{base}/api/agents/connect")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_do_canal_segue_o_esquema_da_central() {
        assert_eq!(
            websocket_url("http://10.8.0.1:3333"),
            "ws://10.8.0.1:3333/api/agents/connect"
        );
        assert_eq!(
            websocket_url("https://monitor.exemplo.com"),
            "wss://monitor.exemplo.com/api/agents/connect"
        );
        assert_eq!(
            normalize_server_url("http://10.8.0.1:3333/").expect("url"),
            "http://10.8.0.1:3333"
        );
        assert!(normalize_server_url("10.8.0.1:3333").is_err());
    }
}
