//! Agenda dos backups automáticos (configurações e bancos de dados).
//!
//! Não há fila nem estado em memória: o instante e o resultado da última
//! execução, gravados no cadastro, **são** a agenda. Um restart retoma de onde
//! parou, e os dois tipos de backup vencem pela mesma regra.

use chrono::{DateTime, Utc};

use crate::services::shared::errors::{AppError, AppResult};

/// Intervalo máximo entre backups automáticos: 30 dias.
pub const MAX_INTERVAL_HOURS: i32 = 24 * 30;

/// Quantas cópias um cadastro pode guardar no máximo.
pub const MAX_RETENTION: i32 = 365;

/// Confere a frequência e a retenção que o formulário mandou.
///
/// # Errors
///
/// Valor fora de `1..=MAX`.
pub fn validate_policy(interval_hours: i32, retention: i32) -> AppResult<()> {
    if !(1..=MAX_INTERVAL_HOURS).contains(&interval_hours) {
        return Err(AppError::validation(format!(
            "A frequência do backup deve ficar entre 1 hora e {} dias",
            MAX_INTERVAL_HOURS / 24
        )));
    }
    if !(1..=MAX_RETENTION).contains(&retention) {
        return Err(AppError::validation(format!(
            "Mantenha entre 1 e {MAX_RETENTION} cópias"
        )));
    }
    Ok(())
}

/// Depois de uma falha, a nova tentativa não espera o intervalo inteiro: um
/// NAS que voltou em meia hora não pode custar o backup do dia.
pub const RETRY_AFTER_FAILURE_HOURS: i64 = 1;

/// O que a agenda precisa saber de um cadastro com backup automático.
#[derive(Debug, Clone, Copy)]
pub struct BackupPolicy {
    pub enabled: bool,
    pub interval_hours: i32,
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_run_failed: bool,
}

impl BackupPolicy {
    /// Instante em que vence; `None` com o backup automático desligado.
    ///
    /// Quem nunca fez backup vence desde sempre. Depois de uma falha, a espera
    /// é a menor entre o intervalo e [`RETRY_AFTER_FAILURE_HOURS`].
    #[must_use]
    pub fn due_at(&self) -> Option<DateTime<Utc>> {
        if !self.enabled {
            return None;
        }
        let Some(last) = self.last_run_at else {
            return Some(DateTime::<Utc>::MIN_UTC);
        };
        let interval = i64::from(self.interval_hours.max(1));
        let wait = if self.last_run_failed {
            interval.min(RETRY_AFTER_FAILURE_HOURS)
        } else {
            interval
        };
        Some(last + chrono::Duration::hours(wait))
    }

    /// Deve rodar agora?
    #[must_use]
    pub fn is_due(&self, now: DateTime<Utc>) -> bool {
        self.due_at().is_some_and(|due| due <= now)
    }

    /// Próxima execução, para a tela. Vencido conta como "agora": o próximo
    /// ciclo o pega.
    #[must_use]
    pub fn next_run(&self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        self.due_at().map(|due| due.max(now))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(hour: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 6, hour, 0, 0).unwrap()
    }

    fn policy(enabled: bool, last: Option<DateTime<Utc>>, failed: bool) -> BackupPolicy {
        BackupPolicy {
            enabled,
            interval_hours: 24,
            last_run_at: last,
            last_run_failed: failed,
        }
    }

    #[test]
    fn frequencia_e_retencao_fora_da_faixa_sao_recusadas() {
        for (interval, retention) in [(0, 14), (MAX_INTERVAL_HOURS + 1, 14), (24, 0), (24, 366)] {
            assert!(
                validate_policy(interval, retention).is_err(),
                "{interval}h / {retention}"
            );
        }
        assert!(validate_policy(24, 14).is_ok());
    }

    #[test]
    fn desligado_nunca_vence() {
        assert!(!policy(false, None, false).is_due(at(12)));
        assert_eq!(policy(false, None, false).next_run(at(0)), None);
    }

    #[test]
    fn quem_nunca_fez_backup_vence_na_hora() {
        assert!(policy(true, None, false).is_due(at(0)));
    }

    #[test]
    fn sucesso_espera_o_intervalo_inteiro() {
        let done = policy(true, Some(at(0)), false);
        assert!(!done.is_due(at(23)));
        assert!(done.is_due(Utc.with_ymd_and_hms(2026, 10, 7, 0, 0, 0).unwrap()));
    }

    #[test]
    fn falha_tenta_de_novo_em_uma_hora() {
        let failed = policy(true, Some(at(0)), true);
        assert!(!failed.is_due(Utc.with_ymd_and_hms(2026, 10, 6, 0, 59, 0).unwrap()));
        assert!(failed.is_due(at(1)));
        assert_eq!(failed.next_run(at(0)), Some(at(1)));
        // Vencido aparece como "agora", e não num passado.
        assert_eq!(failed.next_run(at(5)), Some(at(5)));
    }
}
