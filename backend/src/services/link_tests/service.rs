//! Os casos de uso do teste de enlace: planejar, iniciar, cancelar, listar e
//! detalhar. Os controllers só extraem, validam, delegam e serializam.

use std::sync::Arc;

use chrono::Utc;

use super::{
    env::LinkTestEnv,
    events, guard,
    model::{
        as_db, from_db, LinkTest, LinkTestFix, LinkTestPage, LinkTestPlan, LinkTestQuery,
        LinkTestRequest, LinkTestStatus, LinkTestStepStatus,
    },
    plan,
    repository::{self, Requester},
    runner::{Job, CANCELLED_TEXT},
    view,
};
use crate::services::shared::errors::{AppError, AppResult};

/// Mensagem quando o pedido chega sem a confirmação explícita.
pub const CONFIRM_REQUIRED: &str =
    "Confirme o teste: medir satura o enlace durante a medida (envie confirm: true)";

/// O plano do pedido, sem rodar nada.
///
/// # Errors
///
/// Ver [`plan::build`].
pub async fn plan(env: &LinkTestEnv, request: &LinkTestRequest) -> AppResult<LinkTestPlan> {
    Ok(plan::build(env, request).await?.view)
}

/// Refaz o plano, reserva a vaga, grava o teste e o executa em segundo
/// plano. Devolve o id.
///
/// # Errors
///
/// `Validation` sem `confirm: true` (pedido da tela) ou com o pedido
/// incompleto; `BusinessRule` quando o plano tem bloqueios; `Conflict` (409)
/// quando um dispositivo já está em teste ou o sistema já roda o máximo.
pub async fn start(
    env: Arc<LinkTestEnv>,
    request: &LinkTestRequest,
    requester: Requester,
) -> AppResult<LinkTest> {
    let built = plan::build(&env, request).await?;
    // Ocupado vem antes dos demais bloqueios: é 409, não 400.
    let reservation = guard::reserve(&built.device_ids(), &|id| built.device_name(id))?;
    if !built.view.blocking.is_empty() {
        return Err(AppError::business_rule(built.view.blocking.join("; ")));
    }
    // Só faltam passos (instalar, cadastrar o acesso), mas nenhuma perna
    // roda sem eles.
    if !built.legs.iter().any(|leg| leg.view.runnable) {
        let steps: Vec<&str> = built.view.fixes.iter().map(LinkTestFix::message).collect();
        return Err(AppError::business_rule(format!(
            "Antes de testar, falta: {}",
            steps.join("; ")
        )));
    }
    let (test, legs) = repository::insert(&env.db, &built, requester).await?;
    reservation.bind(test.id);
    let users = repository::user_refs(&env.db, test.user_id.into_iter().collect()).await?;
    let requested_by = test.user_id.and_then(|id| users.get(&id).cloned());
    let snapshot = view::test(&test, &legs, requested_by.clone(), false);
    events::publish_test(env.bus.as_ref(), &snapshot);
    let job = Job::new(
        Arc::clone(&env),
        test,
        legs,
        built.legs,
        built.spec,
        requested_by,
        reservation,
    );
    tokio::spawn(job.run());
    Ok(snapshot)
}

/// Pedido da tela: exige `confirm: true`.
///
/// # Errors
///
/// Ver [`start`].
pub async fn start_confirmed(
    env: Arc<LinkTestEnv>,
    request: &LinkTestRequest,
    user_id: Option<i64>,
) -> AppResult<LinkTest> {
    if request.confirm != Some(true) {
        return Err(AppError::validation(CONFIRM_REQUIRED));
    }
    start(
        env,
        request,
        Requester {
            origin: super::model::LinkTestOrigin::User,
            schedule_id: None,
            user_id,
        },
    )
    .await
}

/// Cancela o teste. Um teste que consta como em curso mas não roda neste
/// processo (sobra de uma queda) é encerrado direto.
///
/// # Errors
///
/// `NotFound`; `Conflict` quando o teste já terminou.
pub async fn cancel(env: &LinkTestEnv, id: i64) -> AppResult<LinkTest> {
    let mut test = repository::find(&env.db, id).await?;
    let status = from_db::<LinkTestStatus>(&test.status).unwrap_or(LinkTestStatus::Failed);
    if !status.is_active() {
        return Err(AppError::conflict("O teste já terminou."));
    }
    if !guard::cancel(id) {
        test.status = as_db(&LinkTestStatus::Cancelled);
        test.error = Some(CANCELLED_TEXT.to_owned());
        test.finished_at = Some(Utc::now().into());
        repository::save_test(&env.db, &test).await?;
        for mut leg in repository::legs_of(&env.db, &[id]).await? {
            let leg_status =
                from_db::<LinkTestStepStatus>(&leg.status).unwrap_or(LinkTestStepStatus::Pending);
            if !leg_status.is_final() {
                leg.status = as_db(&LinkTestStepStatus::Cancelled);
                leg.error = Some(CANCELLED_TEXT.to_owned());
                repository::save_leg(&env.db, &leg).await?;
            }
        }
    }
    let detail = repository::detail(&env.db, id).await?;
    if !guard::is_running(id) {
        events::publish_test(env.bus.as_ref(), &detail);
    }
    Ok(detail)
}

/// Uma página da lista.
///
/// # Errors
///
/// Erro do banco.
pub async fn list(env: &LinkTestEnv, query: &LinkTestQuery) -> AppResult<LinkTestPage> {
    repository::page(&env.db, query).await
}

/// O teste com as amostras.
///
/// # Errors
///
/// `NotFound` ou erro do banco.
pub async fn detail(env: &LinkTestEnv, id: i64) -> AppResult<LinkTest> {
    repository::detail(&env.db, id).await
}
