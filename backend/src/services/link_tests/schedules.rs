//! Agendamentos de teste de enlace (ADR 015 §5): intervalo em minutos (≥ 15),
//! janela de horário opcional (hora local do servidor, pode cruzar a
//! meia-noite), ativo/inativo, último e próximo. O agendador do processo
//! único (ADR 007) chama [`run_due`] a cada minuto.

use std::sync::Arc;

use chrono::{DateTime, Local, NaiveDateTime, NaiveTime, TimeZone, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde_json::json;

use super::{
    env::LinkTestEnv,
    events,
    model::{
        as_db, LinkTestOrigin, LinkTestRequest, LinkTestSchedule, LinkTestScheduleInput,
        LinkTestStatus, MIN_INTERVAL_MINUTES,
    },
    plan, repository,
    repository::Requester,
    service, view,
};
use crate::{
    models::link_test_schedules,
    services::shared::errors::{AppError, AppResult},
};

/// Maior intervalo aceito: uma semana.
pub const MAX_INTERVAL_MINUTES: i32 = 7 * 24 * 60;
/// Nova tentativa quando o agendamento vence com o dispositivo ocupado.
const BUSY_RETRY_MINUTES: i64 = 5;

/// Janela de horário: `(início, fim)`.
pub type Window = (NaiveTime, NaiveTime);

/// `"HH:MM"` → horário.
///
/// # Errors
///
/// `Validation` fora do formato.
pub fn parse_hhmm(value: &str) -> AppResult<NaiveTime> {
    NaiveTime::parse_from_str(value.trim(), "%H:%M")
        .map_err(|_| AppError::validation(format!("Horário inválido: \"{value}\" (use HH:MM)")))
}

/// O horário cai na janela. Início depois do fim = cruza a meia-noite
/// (22:00–06:00); início igual ao fim = o dia todo.
#[must_use]
pub fn in_window(time: NaiveTime, (start, end): Window) -> bool {
    match start.cmp(&end) {
        std::cmp::Ordering::Less => start <= time && time < end,
        std::cmp::Ordering::Greater => time >= start || time < end,
        std::cmp::Ordering::Equal => true,
    }
}

/// O primeiro instante a partir de `at` que cai na janela.
#[must_use]
pub fn next_in_window(at: NaiveDateTime, window: Option<Window>) -> NaiveDateTime {
    let Some(window) = window.filter(|window| !in_window(at.time(), *window)) else {
        return at;
    };
    let today = at.date().and_time(window.0);
    if today > at {
        today
    } else {
        today + chrono::Duration::days(1)
    }
}

/// Próxima execução depois de `now` (hora local), respeitando a janela.
#[must_use]
pub fn next_run(
    now: NaiveDateTime,
    interval_minutes: i32,
    window: Option<Window>,
) -> NaiveDateTime {
    next_in_window(
        now + chrono::Duration::minutes(i64::from(interval_minutes)),
        window,
    )
}

fn local_to_utc(at: NaiveDateTime) -> DateTime<Utc> {
    Local.from_local_datetime(&at).earliest().map_or_else(
        || Utc.from_utc_datetime(&at),
        |local| local.with_timezone(&Utc),
    )
}

fn window_of(row: &link_test_schedules::Model) -> Option<Window> {
    match (row.window_start.as_deref(), row.window_end.as_deref()) {
        (Some(start), Some(end)) => Some((parse_hhmm(start).ok()?, parse_hhmm(end).ok()?)),
        _ => None,
    }
}

/// Próxima execução de um agendamento, em UTC.
fn next_run_utc(
    now: DateTime<Utc>,
    interval_minutes: i32,
    window: Option<Window>,
) -> DateTime<Utc> {
    local_to_utc(next_run(
        now.with_timezone(&Local).naive_local(),
        interval_minutes,
        window,
    ))
}

/// O pedido de teste que o agendamento repete.
#[must_use]
pub fn request_of(row: &link_test_schedules::Model) -> LinkTestRequest {
    let spec: super::model::LinkTestSpec =
        serde_json::from_value(row.spec.clone()).unwrap_or_default();
    LinkTestRequest {
        kind: super::model::from_db(&row.kind).unwrap_or_default(),
        link_id: row.link_id,
        source_device_id: row.source_device_id,
        target_device_id: row.target_device_id,
        spec: Some(super::model::LinkTestSpecInput {
            duration_s: Some(spec.duration_s),
            direction: Some(spec.direction),
            streams: Some(spec.streams),
            limit_mbps: spec.limit_mbps,
        }),
        confirm: Some(true),
    }
}

/// Confere o pedido e devolve a janela.
async fn validate(env: &LinkTestEnv, input: &LinkTestScheduleInput) -> AppResult<Option<Window>> {
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 120 {
        return Err(AppError::validation(
            "Dê um nome ao agendamento (até 120 caracteres)",
        ));
    }
    if !(MIN_INTERVAL_MINUTES..=MAX_INTERVAL_MINUTES).contains(&input.interval_minutes) {
        return Err(AppError::validation(format!(
            "O intervalo deve ficar entre {MIN_INTERVAL_MINUTES} minutos e uma semana"
        )));
    }
    let window = match (input.window_start.as_deref(), input.window_end.as_deref()) {
        (None, None) => None,
        (Some(start), Some(end)) => Some((parse_hhmm(start)?, parse_hhmm(end)?)),
        _ => {
            return Err(AppError::validation(
                "Informe o início e o fim da janela de horário, ou nenhum dos dois",
            ))
        }
    };
    // O mesmo plano da tela: confere especificação, dispositivos e enlace.
    // Bloqueio de agora (plugin faltando) não impede agendar.
    plan::build(env, &input_request(input)).await?;
    Ok(window)
}

fn input_request(input: &LinkTestScheduleInput) -> LinkTestRequest {
    LinkTestRequest {
        kind: input.kind,
        link_id: input.link_id,
        source_device_id: input.source_device_id,
        target_device_id: input.target_device_id,
        spec: input.spec.clone(),
        confirm: Some(true),
    }
}

fn hhmm(time: Option<NaiveTime>) -> Option<String> {
    time.map(|time| time.format("%H:%M").to_string())
}

/// # Errors
///
/// Erro do banco.
pub async fn list(env: &LinkTestEnv) -> AppResult<Vec<LinkTestSchedule>> {
    Ok(link_test_schedules::Entity::find()
        .order_by_asc(link_test_schedules::Column::Name)
        .all(&env.db)
        .await?
        .iter()
        .map(view::schedule)
        .collect())
}

/// # Errors
///
/// `NotFound` ou erro do banco.
pub async fn find(env: &LinkTestEnv, id: i64) -> AppResult<link_test_schedules::Model> {
    link_test_schedules::Entity::find_by_id(id)
        .one(&env.db)
        .await?
        .ok_or_else(|| AppError::not_found("Agendamento não encontrado"))
}

/// # Errors
///
/// `Validation`/`NotFound` do pedido ou erro do banco.
pub async fn create(
    env: &LinkTestEnv,
    input: &LinkTestScheduleInput,
    user_id: Option<i64>,
) -> AppResult<LinkTestSchedule> {
    let window = validate(env, input).await?;
    let spec = input.spec.clone().unwrap_or_default().resolve()?;
    let now = Utc::now();
    let row = link_test_schedules::ActiveModel {
        name: Set(input.name.trim().to_owned()),
        kind: Set(as_db(&input.kind)),
        link_id: Set(input.link_id),
        source_device_id: Set(input.source_device_id),
        target_device_id: Set(input.target_device_id),
        spec: Set(json!(spec)),
        interval_minutes: Set(input.interval_minutes),
        window_start: Set(hhmm(window.map(|window| window.0))),
        window_end: Set(hhmm(window.map(|window| window.1))),
        enabled: Set(input.enabled),
        next_run_at: Set(input
            .enabled
            .then(|| next_run_utc(now, input.interval_minutes, window).into())),
        user_id: Set(user_id),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
        ..Default::default()
    }
    .insert(&env.db)
    .await?;
    events::publish_schedule(env.bus.as_ref(), row.id);
    Ok(view::schedule(&row))
}

/// # Errors
///
/// `Validation`/`NotFound` ou erro do banco.
pub async fn update(
    env: &LinkTestEnv,
    id: i64,
    input: &LinkTestScheduleInput,
) -> AppResult<LinkTestSchedule> {
    let current = find(env, id).await?;
    let window = validate(env, input).await?;
    let spec = input.spec.clone().unwrap_or_default().resolve()?;
    let mut row: link_test_schedules::ActiveModel = current.into();
    row.name = Set(input.name.trim().to_owned());
    row.kind = Set(as_db(&input.kind));
    row.link_id = Set(input.link_id);
    row.source_device_id = Set(input.source_device_id);
    row.target_device_id = Set(input.target_device_id);
    row.spec = Set(json!(spec));
    row.interval_minutes = Set(input.interval_minutes);
    row.window_start = Set(hhmm(window.map(|window| window.0)));
    row.window_end = Set(hhmm(window.map(|window| window.1)));
    row.enabled = Set(input.enabled);
    row.next_run_at = Set(input
        .enabled
        .then(|| next_run_utc(Utc::now(), input.interval_minutes, window).into()));
    let row = row.update(&env.db).await?;
    events::publish_schedule(env.bus.as_ref(), row.id);
    Ok(view::schedule(&row))
}

/// # Errors
///
/// `NotFound` ou erro do banco.
pub async fn delete(env: &LinkTestEnv, id: i64) -> AppResult<link_test_schedules::Model> {
    let row = find(env, id).await?;
    link_test_schedules::Entity::delete_by_id(id)
        .exec(&env.db)
        .await?;
    events::publish_schedule(env.bus.as_ref(), id);
    Ok(row)
}

/// Roda o agendamento agora (pedido do operador). Devolve o id do teste.
///
/// # Errors
///
/// Os de [`service::start`].
pub async fn run_now(env: Arc<LinkTestEnv>, id: i64, user_id: Option<i64>) -> AppResult<i64> {
    let row = find(&env, id).await?;
    let test = service::start(
        Arc::clone(&env),
        &request_of(&row),
        Requester {
            origin: LinkTestOrigin::Schedule,
            schedule_id: Some(row.id),
            user_id,
        },
    )
    .await?;
    // `lastTestId`/`lastStatus` são do executor (`record_result`): ele os
    // grava ao começar e ao terminar, sem corrida com esta escrita.
    let mut active: link_test_schedules::ActiveModel = row.into();
    active.last_run_at = Set(Some(Utc::now().into()));
    active.update(&env.db).await?;
    events::publish_schedule(env.bus.as_ref(), id);
    Ok(test.id)
}

/// Grava o desfecho do teste no agendamento.
///
/// # Errors
///
/// Erro do banco.
pub async fn record_result(
    env: &LinkTestEnv,
    schedule_id: i64,
    test_id: i64,
    status: LinkTestStatus,
) -> AppResult<()> {
    let Some(row) = link_test_schedules::Entity::find_by_id(schedule_id)
        .one(&env.db)
        .await?
    else {
        return Ok(());
    };
    if row.last_test_id.is_some_and(|last| last > test_id) {
        // Um teste mais novo já ocupa o "último".
        return Ok(());
    }
    let mut active: link_test_schedules::ActiveModel = row.into();
    active.last_test_id = Set(Some(test_id));
    active.last_status = Set(Some(as_db(&status)));
    active.update(&env.db).await?;
    events::publish_schedule(env.bus.as_ref(), schedule_id);
    Ok(())
}

/// Dispara os agendamentos vencidos. Devolve quantos testes começaram.
///
/// # Errors
///
/// Erro do banco ao listar; falhas de cada agendamento ficam no próprio
/// histórico (um teste `failed` com o motivo) ou no log.
pub async fn run_due(env: Arc<LinkTestEnv>, now: DateTime<Utc>) -> AppResult<usize> {
    let due = link_test_schedules::Entity::find()
        .filter(link_test_schedules::Column::Enabled.eq(true))
        .filter(
            Condition::any()
                .add(link_test_schedules::Column::NextRunAt.is_null())
                .add(link_test_schedules::Column::NextRunAt.lte(now)),
        )
        .all(&env.db)
        .await?;
    let mut started = 0;
    for row in due {
        let window = window_of(&row);
        let local = now.with_timezone(&Local).naive_local();
        let mut active: link_test_schedules::ActiveModel = row.clone().into();
        if window.is_some_and(|window| !in_window(local.time(), window)) {
            active.next_run_at = Set(Some(local_to_utc(next_in_window(local, window)).into()));
            active.update(&env.db).await?;
            continue;
        }
        let request = request_of(&row);
        let requester = Requester {
            origin: LinkTestOrigin::Schedule,
            schedule_id: Some(row.id),
            user_id: None,
        };
        match service::start(Arc::clone(&env), &request, requester).await {
            Ok(_) => {
                started += 1;
                active.last_run_at = Set(Some(now.into()));
                active.next_run_at =
                    Set(Some(next_run_utc(now, row.interval_minutes, window).into()));
            }
            // Ocupado agora: tenta de novo em alguns minutos.
            Err(AppError::Conflict(reason)) => {
                tracing::info!(schedule_id = row.id, %reason, "agendamento de teste de enlace adiado");
                active.next_run_at = Set(Some(
                    (now + chrono::Duration::minutes(BUSY_RETRY_MINUTES)).into(),
                ));
            }
            Err(AppError::Internal(error)) => {
                tracing::warn!(%error, schedule_id = row.id, "falha ao disparar agendamento de teste de enlace");
                active.next_run_at =
                    Set(Some(next_run_utc(now, row.interval_minutes, window).into()));
            }
            // O plano não roda (bloqueio, enlace apagado): o motivo vai ao
            // histórico, como um teste que falhou.
            Err(error) => {
                let spec = request
                    .spec
                    .clone()
                    .unwrap_or_default()
                    .resolve()
                    .unwrap_or_default();
                let failed = repository::insert_failed(
                    &env.db,
                    &request,
                    &spec,
                    requester,
                    error.to_string(),
                )
                .await?;
                active.last_run_at = Set(Some(now.into()));
                active.last_test_id = Set(Some(failed.id));
                active.last_status = Set(Some(as_db(&LinkTestStatus::Failed)));
                active.next_run_at =
                    Set(Some(next_run_utc(now, row.interval_minutes, window).into()));
            }
        }
        active.update(&env.db).await?;
        events::publish_schedule(env.bus.as_ref(), row.id);
    }
    Ok(started)
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;

    fn t(h: u32, m: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, 0).unwrap()
    }

    fn at(day: u32, h: u32, m: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, day)
            .unwrap()
            .and_time(t(h, m))
    }

    #[test]
    fn janela_comum_e_cruzando_a_meia_noite() {
        let comercial = (t(8, 0), t(18, 0));
        assert!(in_window(t(8, 0), comercial));
        assert!(in_window(t(17, 59), comercial));
        assert!(!in_window(t(18, 0), comercial));
        assert!(!in_window(t(3, 0), comercial));

        let madrugada = (t(22, 0), t(6, 0));
        assert!(in_window(t(23, 30), madrugada));
        assert!(in_window(t(2, 0), madrugada));
        assert!(!in_window(t(6, 0), madrugada));
        assert!(!in_window(t(12, 0), madrugada));

        assert!(in_window(t(12, 0), (t(5, 0), t(5, 0))), "dia todo");
    }

    #[test]
    fn proxima_execucao_pelo_intervalo() {
        assert_eq!(next_run(at(16, 10, 0), 60, None), at(16, 11, 0));
        assert_eq!(next_run(at(16, 23, 50), 30, None), at(17, 0, 20));
    }

    #[test]
    fn proxima_execucao_respeita_a_janela() {
        let madrugada = Some((t(22, 0), t(6, 0)));
        // Dentro da janela: só soma o intervalo.
        assert_eq!(next_run(at(16, 23, 0), 60, madrugada), at(17, 0, 0));
        // Passou do fim: vai para o próximo início, no mesmo dia.
        assert_eq!(next_run(at(17, 5, 30), 60, madrugada), at(17, 22, 0));
        // Antes do início, à tarde: hoje às 22h.
        assert_eq!(next_run(at(16, 14, 0), 15, madrugada), at(16, 22, 0));

        let comercial = Some((t(8, 0), t(18, 0)));
        // Passou do fim: amanhã às 8h.
        assert_eq!(next_run(at(16, 17, 30), 60, comercial), at(17, 8, 0));
        // De madrugada: hoje às 8h.
        assert_eq!(next_in_window(at(16, 3, 0), comercial), at(16, 8, 0));
    }

    #[test]
    fn horario_no_formato_hh_mm() {
        assert_eq!(parse_hhmm("06:30").unwrap(), t(6, 30));
        assert!(parse_hhmm("6h30").is_err());
        assert!(parse_hhmm("25:00").is_err());
    }
}
