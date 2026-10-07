//! Trava de "um de cada vez" por recurso, dentro do processo.
//!
//! O clique em "Fazer backup agora" pode coincidir com o agendador; dois
//! backups do mesmo alvo no mesmo instante gravariam o mesmo arquivo, e a
//! retenção de um apagaria o que o outro ainda está enviando. A trava é por
//! `(escopo, id)`: o destino 3 e a conexão de banco 3 não se bloqueiam.

use std::collections::HashSet;
use std::sync::{LazyLock, Mutex};

use crate::services::shared::errors::{AppError, AppResult};

static RUNNING: LazyLock<Mutex<HashSet<(&'static str, i64)>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

/// Segura a vez enquanto viver; solta no `Drop`, inclusive em pânico ou erro.
#[derive(Debug)]
pub struct RunGuard {
    key: (&'static str, i64),
}

impl RunGuard {
    /// Pega a vez de `(scope, id)`.
    ///
    /// # Errors
    ///
    /// `409` com `busy_message` quando já há uma execução em andamento.
    pub fn acquire(scope: &'static str, id: i64, busy_message: &str) -> AppResult<Self> {
        let mut running = RUNNING
            .lock()
            .map_err(|_| AppError::Internal(anyhow::anyhow!("trava de execução envenenada")))?;
        if !running.insert((scope, id)) {
            return Err(AppError::conflict(busy_message));
        }
        Ok(Self { key: (scope, id) })
    }

    /// Já há execução em andamento para `(scope, id)`?
    #[must_use]
    pub fn is_running(scope: &'static str, id: i64) -> bool {
        RUNNING
            .lock()
            .is_ok_and(|running| running.contains(&(scope, id)))
    }
}

impl Drop for RunGuard {
    fn drop(&mut self) {
        if let Ok(mut running) = RUNNING.lock() {
            running.remove(&self.key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dois_do_mesmo_recurso_nao_correm_juntos() {
        let first = RunGuard::acquire("teste", -42, "ocupado").unwrap();
        assert!(RunGuard::is_running("teste", -42));
        assert!(RunGuard::acquire("teste", -42, "ocupado").is_err());
        drop(first);
        assert!(!RunGuard::is_running("teste", -42));
        assert!(RunGuard::acquire("teste", -42, "ocupado").is_ok());
    }

    #[test]
    fn escopos_diferentes_nao_se_bloqueiam() {
        let _storage = RunGuard::acquire("teste-a", -7, "ocupado").unwrap();
        assert!(RunGuard::acquire("teste-b", -7, "ocupado").is_ok());
    }
}
