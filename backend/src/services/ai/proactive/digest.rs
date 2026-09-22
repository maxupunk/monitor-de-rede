//! Resumo periódico da rede (diário ou semanal), entregue pelos canais de
//! notificação configurados e guardado para consulta na tela.

use std::{sync::Arc, time::Duration as StdDuration};

use chrono::{DateTime, Local, Utc};
use loco_rs::prelude::AppContext;
use sea_orm::ConnectionTrait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use ts_rs::TS;

use super::{
    config::AiDigestSchedule,
    runner::{ask, DriverFactory},
    schedule::{is_digest_due, period_hours},
};
use crate::{
    models::system_settings,
    services::{
        ai::{
            harness::prompt::ChatContext,
            settings::{self, AiSettings},
        },
        notifications::{NotificationMessage, NotificationService, Severity},
        shared::errors::{AppError, AppResult},
    },
};

/// Último resumo gerado (automático ou pedido na tela).
const LATEST_KEY: &str = "ai.digest.latest";
/// Instante do último envio automático — só o agendado conta.
const LAST_SENT_KEY: &str = "ai.digest.last_sent";
/// De quanto em quanto tempo o agendador confere se o resumo venceu.
const CHECK_INTERVAL: StdDuration = StdDuration::from_secs(300);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AiDigest {
    pub text: String,
    pub generated_at: String,
    #[ts(type = "number")]
    pub period_hours: i64,
    #[ts(type = "number")]
    pub prompt_tokens: u64,
    #[ts(type = "number")]
    pub completion_tokens: u64,
}

fn prompt(hours: i64) -> String {
    format!(
        "Gere o resumo da rede das últimas {hours} horas para o responsável pela infraestrutura. \
Use as ferramentas: get_system_summary, get_alerts (status 'all', hours {hours}), analyze_root_cause, \
get_logs_overview (hours {hours}, severity error) e get_device_interfaces com problems_only nos dispositivos com alerta. \
Formato, em tópicos curtos: 1) Disponibilidade geral; 2) Incidentes e causas; 3) Pontos de atenção \
(interfaces, logs de erro recorrentes); 4) Até 3 recomendações. Sem introdução."
    )
}

/// # Errors
///
/// Erro do banco.
pub async fn latest<C: ConnectionTrait>(db: &C) -> AppResult<Option<AiDigest>> {
    Ok(system_settings::Model::get(db, LATEST_KEY)
        .await?
        .and_then(|row| row.value)
        .and_then(|text| serde_json::from_str(&text).ok()))
}

async fn last_sent<C: ConnectionTrait>(db: &C) -> AppResult<Option<DateTime<Utc>>> {
    Ok(system_settings::Model::get(db, LAST_SENT_KEY)
        .await?
        .and_then(|row| row.value)
        .and_then(|text| DateTime::parse_from_rfc3339(&text).ok())
        .map(|at| at.with_timezone(&Utc)))
}

/// Gera o resumo cobrindo `hours` horas e o guarda como o mais recente.
///
/// # Errors
///
/// Assistente desativado, provedor indisponível ou erro do banco.
pub async fn generate(
    ctx: &AppContext,
    settings: &AiSettings,
    drivers: &dyn DriverFactory,
    hours: i64,
) -> AppResult<AiDigest> {
    if !settings.enabled {
        return Err(AppError::validation(
            "O Assistente IA está desativado nas configurações do sistema.",
        ));
    }
    let answer = ask(
        ctx,
        settings,
        drivers,
        prompt(hours),
        ChatContext::default(),
    )
    .await?;
    let digest = AiDigest {
        text: answer.text,
        generated_at: Utc::now().to_rfc3339(),
        period_hours: hours,
        prompt_tokens: answer.usage.prompt_tokens,
        completion_tokens: answer.usage.completion_tokens,
    };
    let text = serde_json::to_string(&digest)
        .map_err(|error| AppError::Internal(anyhow::Error::new(error)))?;
    system_settings::Model::set(&ctx.db, LATEST_KEY, Some(text)).await?;
    Ok(digest)
}

fn notification(digest: &AiDigest, schedule: AiDigestSchedule) -> NotificationMessage {
    let title = match schedule {
        AiDigestSchedule::Weekly => "Resumo semanal da rede (IA)",
        AiDigestSchedule::Off | AiDigestSchedule::Daily => "Resumo diário da rede (IA)",
    };
    NotificationMessage {
        title: title.to_string(),
        body: digest.text.clone(),
        severity: Severity::Info,
        metadata: json!({ "kind": "ai_digest", "periodHours": digest.period_hours }),
    }
}

/// Confere o agendamento e, vencido, gera, envia e marca o envio.
/// Devolve se enviou.
///
/// # Errors
///
/// Provedor indisponível ou erro do banco.
pub async fn run_if_due(
    ctx: &AppContext,
    drivers: &dyn DriverFactory,
    now: DateTime<Local>,
) -> AppResult<bool> {
    let settings = settings::load(&ctx.db).await?;
    let schedule = settings.proactive.digest;
    if !settings.enabled
        || !is_digest_due(
            &now,
            last_sent(&ctx.db).await?,
            schedule,
            settings.proactive.digest_hour,
        )
    {
        return Ok(false);
    }
    // Marca antes de gerar: se o provedor falhar, a próxima tentativa fica
    // para o próximo período em vez de insistir a cada 5 minutos.
    system_settings::Model::set(
        &ctx.db,
        LAST_SENT_KEY,
        Some(now.with_timezone(&Utc).to_rfc3339()),
    )
    .await?;
    let digest = generate(ctx, &settings, drivers, period_hours(schedule)).await?;
    NotificationService::with_default_channels()
        .notify(ctx, &notification(&digest, schedule))
        .await;
    Ok(true)
}

/// Confere o agendamento a cada 5 minutos.
pub fn spawn_scheduler(ctx: AppContext, drivers: Arc<dyn DriverFactory>) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(CHECK_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            if let Err(error) = run_if_due(&ctx, drivers.as_ref(), Local::now()).await {
                tracing::warn!(%error, "falha ao gerar o resumo periódico da IA");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titulo_da_notificacao_segue_a_frequencia() {
        let digest = AiDigest {
            text: "Tudo ok".into(),
            generated_at: String::new(),
            period_hours: 168,
            prompt_tokens: 0,
            completion_tokens: 0,
        };
        let semanal = notification(&digest, AiDigestSchedule::Weekly);
        assert_eq!(semanal.title, "Resumo semanal da rede (IA)");
        assert_eq!(semanal.body, "Tudo ok");
        assert_eq!(semanal.metadata["periodHours"], 168);
        assert_eq!(
            notification(&digest, AiDigestSchedule::Daily).title,
            "Resumo diário da rede (IA)"
        );
    }
}
