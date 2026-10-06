//! Backup automático: o ciclo que manda a cópia para cada destino na hora.
//!
//! Um laço só, no processo do servidor, que a cada minuto pergunta ao banco
//! quem está vencido ([`is_due`]). Não há fila nem estado em memória: o
//! `last_backup_at` gravado é a agenda, e um restart retoma de onde parou.

use std::time::Duration;

use chrono::{DateTime, Utc};
use loco_rs::{app::AppContext, app::Hooks};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use super::{backups, service};
use crate::{app::App, models::storage_destinations, services::shared::errors::AppResult};

/// De quanto em quanto tempo o ciclo confere os vencimentos.
const TICK: Duration = Duration::from_secs(60);

/// Espera antes da primeira conferência: o boot já tem trabalho de sobra, e um
/// restart em sequência não deve disparar uma rajada de backups.
const FIRST_TICK_DELAY: Duration = Duration::from_secs(120);

/// Depois de uma falha, a nova tentativa não espera o intervalo inteiro: um
/// NAS que voltou em meia hora não pode custar o backup do dia.
const RETRY_AFTER_FAILURE_HOURS: i64 = 1;

/// Instante em que o destino vence; `None` com o backup automático desligado.
///
/// Destino que nunca fez backup vence desde sempre. Depois de uma falha, a
/// espera é a menor entre o intervalo e [`RETRY_AFTER_FAILURE_HOURS`].
fn due_at(row: &storage_destinations::Model) -> Option<DateTime<Utc>> {
    if !row.backup_enabled {
        return None;
    }
    let Some(last) = row.last_backup_at else {
        return Some(DateTime::<Utc>::MIN_UTC);
    };
    let interval = i64::from(row.backup_interval_hours.max(1));
    let wait = if row.last_backup_status.as_deref() == Some(backups::STATUS_FAILED) {
        interval.min(RETRY_AFTER_FAILURE_HOURS)
    } else {
        interval
    };
    Some(last.with_timezone(&Utc) + chrono::Duration::hours(wait))
}

/// O destino deve receber um backup agora?
#[must_use]
pub fn is_due(row: &storage_destinations::Model, now: DateTime<Utc>) -> bool {
    due_at(row).is_some_and(|due| due <= now)
}

/// Quando o próximo backup automático vai acontecer, para a tela. Vencido
/// conta como "agora": o próximo ciclo o pega.
#[must_use]
pub fn next_run(row: &storage_destinations::Model, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    due_at(row).map(|due| due.max(now))
}

/// Sobe o ciclo. Fora do ambiente de teste — lá, quem testa chama
/// [`run_due`] diretamente.
pub fn spawn(ctx: &AppContext) {
    if ctx.environment == loco_rs::environment::Environment::Test {
        return;
    }
    let ctx = ctx.clone();
    tokio::spawn(async move {
        tokio::time::sleep(FIRST_TICK_DELAY).await;
        loop {
            if let Err(error) = run_due(&ctx, Utc::now()).await {
                tracing::warn!(%error, "ciclo de backup automático falhou");
            }
            tokio::time::sleep(TICK).await;
        }
    });
}

/// Faz o backup de cada destino vencido, um de cada vez.
///
/// Um destino que falha não segura os outros: o erro fica gravado nele e o
/// ciclo segue.
///
/// # Errors
///
/// Só a falha da consulta inicial.
pub async fn run_due(ctx: &AppContext, now: DateTime<Utc>) -> AppResult<usize> {
    let rows = storage_destinations::Entity::find()
        .filter(storage_destinations::Column::BackupEnabled.eq(true))
        .all(&ctx.db)
        .await?;
    let mut ran = 0;
    for row in rows.into_iter().filter(|row| is_due(row, now)) {
        let id = row.id;
        let outcome = match service::open(row) {
            Ok(destination) => backups::run(&ctx.db, &destination, App::app_version())
                .await
                .map(|_| ()),
            Err(error) => Err(error),
        };
        if let Err(error) = outcome {
            tracing::warn!(destination = id, %error, "backup automático falhou");
        }
        ran += 1;
        service::publish_updated(ctx).await;
    }
    Ok(ran)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn row(
        enabled: bool,
        last: Option<DateTime<Utc>>,
        status: Option<&str>,
    ) -> storage_destinations::Model {
        let now = Utc::now().into();
        storage_destinations::Model {
            id: 1,
            name: "NAS".into(),
            provider: "local".into(),
            config_encrypted: String::new(),
            backup_enabled: enabled,
            backup_interval_hours: 24,
            backup_retention: 14,
            last_backup_at: last.map(Into::into),
            last_backup_status: status.map(Into::into),
            last_backup_error: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn at(hour: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 6, hour, 0, 0).unwrap()
    }

    #[test]
    fn desligado_nunca_vence() {
        assert!(!is_due(&row(false, None, None), at(12)));
        assert_eq!(next_run(&row(false, None, None), at(0)), None);
    }

    #[test]
    fn destino_que_nunca_fez_backup_vence_na_hora() {
        assert!(is_due(&row(true, None, None), at(0)));
    }

    #[test]
    fn sucesso_espera_o_intervalo_inteiro() {
        let done = row(true, Some(at(0)), Some(backups::STATUS_SUCCESS));
        assert!(!is_due(&done, at(23)));
        assert!(is_due(
            &done,
            Utc.with_ymd_and_hms(2026, 10, 7, 0, 0, 0).unwrap()
        ));
    }

    #[test]
    fn falha_tenta_de_novo_em_uma_hora() {
        let failed = row(true, Some(at(0)), Some(backups::STATUS_FAILED));
        assert!(!is_due(
            &failed,
            Utc.with_ymd_and_hms(2026, 10, 6, 0, 59, 0).unwrap()
        ));
        assert!(is_due(&failed, at(1)));
        assert_eq!(next_run(&failed, at(0)), Some(at(1)));
        // Vencido aparece como "agora", e não num passado.
        assert_eq!(next_run(&failed, at(5)), Some(at(5)));
    }
}
