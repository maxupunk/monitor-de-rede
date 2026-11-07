//! A chave de um host Docker: o agente que o atende (ADR 014).

use std::{fmt, str::FromStr};

use crate::shared::errors::{AppError, AppResult};

const AGENT_PREFIX: &str = "agent-";

/// Chave de um host Docker: o id do agente que o atende.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HostKey(i64);

impl HostKey {
    #[must_use]
    pub const fn agent(probe_id: i64) -> Self {
        Self(probe_id)
    }

    #[must_use]
    pub const fn probe_id(self) -> i64 {
        self.0
    }
}

impl fmt::Display for HostKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{AGENT_PREFIX}{}", self.0)
    }
}

impl FromStr for HostKey {
    type Err = AppError;

    fn from_str(value: &str) -> AppResult<Self> {
        value
            .trim()
            .strip_prefix(AGENT_PREFIX)
            .and_then(|id| id.parse::<i64>().ok())
            .filter(|id| *id > 0)
            .map(Self)
            .ok_or_else(|| AppError::validation("Host Docker inválido"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chave_de_host_faz_ida_e_volta() {
        assert_eq!(
            "agent-12".parse::<HostKey>().expect("agente"),
            HostKey::agent(12)
        );
        assert_eq!(HostKey::agent(12).to_string(), "agent-12");
        assert_eq!(HostKey::agent(12).probe_id(), 12);
    }

    #[test]
    fn chave_invalida_e_recusada() {
        for value in [
            "", "local", "agent-", "agent-0", "agent--1", "agent-x", "remoto", "agent:3",
        ] {
            assert!(value.parse::<HostKey>().is_err(), "{value}");
        }
    }
}
