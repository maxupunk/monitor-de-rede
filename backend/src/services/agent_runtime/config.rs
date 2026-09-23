//! Configuração do agente, lida do ambiente do host remoto.
//!
//! | Variável              | Padrão                          |
//! |-----------------------|---------------------------------|
//! | `AGENT_SERVER_URL`    | obrigatória                     |
//! | `AGENT_ENROLL_CODE`   | usada só se ainda não há token  |
//! | `AGENT_TOKEN`         | lido de `<state>/token`         |
//! | `AGENT_STATE_DIR`     | `/var/lib/netmonitor-agent`     |
//! | `AGENT_ALLOW`         | `read,lifecycle,monitor,discovery` |
//! | `AGENT_OUTBOX_MAX`    | 10 000 eventos                  |
//! | `HOST_PROC`/`HOST_ROOT` | `/proc` e `/`                 |

use std::path::{Path, PathBuf};

use crate::services::agents::policy::Policy;

pub const DEFAULT_STATE_DIR: &str = "/var/lib/netmonitor-agent";
const DEFAULT_OUTBOX_MAX: usize = 10_000;

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub server_url: String,
    pub enroll_code: Option<String>,
    pub token: Option<String>,
    pub state_dir: PathBuf,
    pub policy: Policy,
    pub outbox_max: usize,
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
        Ok(Self {
            server_url: normalize_server_url(&server_url)?,
            enroll_code: env("AGENT_ENROLL_CODE"),
            token: env("AGENT_TOKEN"),
            state_dir: env("AGENT_STATE_DIR")
                .map_or_else(|| DEFAULT_STATE_DIR.into(), PathBuf::from),
            policy,
            outbox_max: env("AGENT_OUTBOX_MAX")
                .and_then(|value| value.parse().ok())
                .filter(|value| *value > 0)
                .unwrap_or(DEFAULT_OUTBOX_MAX),
        })
    }

    #[must_use]
    pub fn token_path(&self) -> PathBuf {
        self.state_dir.join("token")
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

/// Lê o token salvo por um enrollment anterior.
#[must_use]
pub fn read_token(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Grava o token com permissão 0600: é a credencial do host.
///
/// # Errors
///
/// Falha de escrita.
pub fn write_token(path: &Path, token: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, token)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(temporary, path)
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

    #[test]
    fn token_e_gravado_e_lido_de_volta() {
        let dir = std::env::temp_dir().join(format!("nm-agent-{}", uuid::Uuid::new_v4()));
        let path = dir.join("token");
        assert_eq!(read_token(&path), None);
        write_token(&path, "segredo\n").expect("grava");
        assert_eq!(read_token(&path).as_deref(), Some("segredo"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
