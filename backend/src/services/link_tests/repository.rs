//! Acesso a `link_tests`, `link_test_legs` e `link_test_schedules`.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sea_orm::{
    sea_query::Query, ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, EntityTrait,
    IntoActiveModel, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};

use super::{
    model::{
        as_db, LinkTest, LinkTestOrigin, LinkTestPage, LinkTestQuery, LinkTestRequest,
        LinkTestSpec, LinkTestStage, LinkTestStatus, LinkTestStepStatus, LinkTestUserRef,
    },
    plan::BuiltPlan,
    view,
};
use crate::{
    models::{_entities::users, link_test_legs, link_tests},
    services::shared::{
        errors::{AppError, AppResult},
        transaction::begin_write,
    },
};

/// Itens por página quando o cliente não manda `perPage`.
pub const DEFAULT_PER_PAGE: u64 = 20;
/// Teto de itens por página.
pub const MAX_PER_PAGE: u64 = 100;
/// Lote da purga.
const PRUNE_BATCH: u64 = 500;

fn json<T: serde::Serialize>(value: &T) -> AppResult<serde_json::Value> {
    serde_json::to_value(value).map_err(|error| AppError::Internal(anyhow::Error::new(error)))
}

/// Quem pediu e por onde: a tela (com usuário) ou um agendamento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Requester {
    pub origin: LinkTestOrigin,
    pub schedule_id: Option<i64>,
    pub user_id: Option<i64>,
}

fn new_test(
    request: &LinkTestRequest,
    spec: &LinkTestSpec,
    requester: Requester,
    device_ids: String,
    status: LinkTestStatus,
) -> AppResult<link_tests::ActiveModel> {
    let now = Utc::now().into();
    Ok(link_tests::ActiveModel {
        kind: Set(as_db(&request.kind)),
        status: Set(as_db(&status)),
        origin: Set(as_db(&requester.origin)),
        schedule_id: Set(requester.schedule_id),
        link_id: Set(request.link_id),
        source_device_id: Set(request.source_device_id),
        target_device_id: Set(request.target_device_id),
        device_ids: Set(device_ids),
        spec: Set(json(spec)?),
        user_id: Set(requester.user_id),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    })
}

/// Grava o teste (`queued`) e as pernas do plano, numa transação.
///
/// # Errors
///
/// Erro do banco.
pub async fn insert(
    db: &sea_orm::DatabaseConnection,
    plan: &BuiltPlan,
    requester: Requester,
) -> AppResult<(link_tests::Model, Vec<link_test_legs::Model>)> {
    let devices = view::device_ids_column(
        plan.legs
            .iter()
            .flat_map(|leg| [leg.view.a.device_id, leg.view.b.device_id])
            .flatten(),
    );
    let txn = begin_write(db).await?;
    let test = new_test(
        &plan.request,
        &plan.spec,
        requester,
        devices,
        LinkTestStatus::Queued,
    )?
    .insert(&txn)
    .await?;
    let now = Utc::now().into();
    let mut legs = Vec::with_capacity(plan.legs.len());
    for leg in &plan.legs {
        let view = &leg.view;
        legs.push(
            link_test_legs::ActiveModel {
                test_id: Set(test.id),
                seq: Set(i32::try_from(view.seq).unwrap_or(i32::MAX)),
                kind: Set(as_db(&view.kind)),
                a: Set(json(&view.a)?),
                b: Set(json(&view.b)?),
                edge_id: Set(view.edge_id),
                engine: Set(view.engine.map(|engine| engine.as_str().to_owned())),
                server_side: Set(view.server_side.map(|side| side.as_str().to_owned())),
                duration_s: Set(i32::from(view.duration_s)),
                runnable: Set(view.runnable),
                issues: Set(json(&view.issues)?),
                status: Set(as_db(&LinkTestStepStatus::Pending)),
                stages: Set(json(&LinkTestStage::all_pending())?),
                created_at: Set(now),
                updated_at: Set(now),
                ..Default::default()
            }
            .insert(&txn)
            .await?,
        );
    }
    txn.commit().await?;
    Ok((test, legs))
}

/// Grava um teste que nem começou — o agendamento que venceu com o plano
/// bloqueado — para o motivo aparecer no histórico.
///
/// # Errors
///
/// Erro do banco.
pub async fn insert_failed<C: ConnectionTrait>(
    db: &C,
    request: &LinkTestRequest,
    spec: &LinkTestSpec,
    requester: Requester,
    error: String,
) -> AppResult<link_tests::Model> {
    let now = Utc::now().into();
    let mut row = new_test(
        request,
        spec,
        requester,
        String::new(),
        LinkTestStatus::Failed,
    )?;
    row.error = Set(Some(error));
    row.finished_at = Set(Some(now));
    Ok(row.insert(db).await?)
}

/// Regrava todas as colunas do teste.
///
/// # Errors
///
/// Erro do banco.
pub async fn save_test<C: ConnectionTrait>(
    db: &C,
    row: &link_tests::Model,
) -> AppResult<link_tests::Model> {
    let mut active = row.clone().into_active_model().reset_all();
    active.updated_at = Set(Utc::now().into());
    Ok(active.update(db).await?)
}

/// Regrava todas as colunas da perna.
///
/// # Errors
///
/// Erro do banco.
pub async fn save_leg<C: ConnectionTrait>(
    db: &C,
    row: &link_test_legs::Model,
) -> AppResult<link_test_legs::Model> {
    let mut active = row.clone().into_active_model().reset_all();
    active.updated_at = Set(Utc::now().into());
    Ok(active.update(db).await?)
}

/// # Errors
///
/// `NotFound` ou erro do banco.
pub async fn find<C: ConnectionTrait>(db: &C, id: i64) -> AppResult<link_tests::Model> {
    link_tests::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::not_found("Teste de enlace não encontrado"))
}

/// As pernas dos testes, em ordem.
///
/// # Errors
///
/// Erro do banco.
pub async fn legs_of<C: ConnectionTrait>(
    db: &C,
    test_ids: &[i64],
) -> AppResult<Vec<link_test_legs::Model>> {
    if test_ids.is_empty() {
        return Ok(Vec::new());
    }
    Ok(link_test_legs::Entity::find()
        .filter(link_test_legs::Column::TestId.is_in(test_ids.to_vec()))
        .order_by_asc(link_test_legs::Column::TestId)
        .order_by_asc(link_test_legs::Column::Seq)
        .all(db)
        .await?)
}

/// `{ id, name }` dos usuários.
///
/// # Errors
///
/// Erro do banco.
pub async fn user_refs<C: ConnectionTrait>(
    db: &C,
    ids: Vec<i64>,
) -> AppResult<HashMap<i64, LinkTestUserRef>> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    Ok(users::Entity::find()
        .filter(users::Column::Id.is_in(ids))
        .all(db)
        .await?
        .into_iter()
        .map(|user| {
            (
                user.id,
                LinkTestUserRef {
                    id: user.id,
                    name: user.name,
                },
            )
        })
        .collect())
}

/// O teste completo, com as amostras de cada perna.
///
/// # Errors
///
/// `NotFound` ou erro do banco.
pub async fn detail<C: ConnectionTrait>(db: &C, id: i64) -> AppResult<LinkTest> {
    let row = find(db, id).await?;
    let legs = legs_of(db, &[id]).await?;
    let users = user_refs(db, row.user_id.into_iter().collect()).await?;
    let requested_by = row.user_id.and_then(|id| users.get(&id).cloned());
    Ok(view::test(&row, &legs, requested_by, true))
}

/// Uma página da lista, do mais novo ao mais antigo.
///
/// # Errors
///
/// Erro do banco.
pub async fn page<C: ConnectionTrait>(db: &C, query: &LinkTestQuery) -> AppResult<LinkTestPage> {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query
        .per_page
        .unwrap_or(DEFAULT_PER_PAGE)
        .clamp(1, MAX_PER_PAGE);
    let mut filter = Condition::all();
    if let Some(device_id) = query.device_id {
        filter = filter.add(link_tests::Column::DeviceIds.contains(format!(",{device_id},")));
    }
    if let Some(link_id) = query.link_id {
        filter = filter.add(
            Condition::any()
                .add(link_tests::Column::LinkId.eq(link_id))
                .add(
                    link_tests::Column::Id.in_subquery(
                        Query::select()
                            .column(link_test_legs::Column::TestId)
                            .from(link_test_legs::Entity)
                            .and_where(link_test_legs::Column::EdgeId.eq(link_id))
                            .to_owned(),
                    ),
                ),
        );
    }
    if let Some(status) = query.status {
        filter = filter.add(link_tests::Column::Status.eq(as_db(&status)));
    }
    if let Some(kind) = query.kind {
        filter = filter.add(link_tests::Column::Kind.eq(as_db(&kind)));
    }
    if let Some(since) = query
        .since
        .as_deref()
        .filter(|since| !since.trim().is_empty())
    {
        let since = DateTime::parse_from_rfc3339(since.trim()).map_err(|_| {
            AppError::validation(format!("`since` precisa ser uma data RFC 3339: {since}"))
        })?;
        filter = filter.add(link_tests::Column::CreatedAt.gte(since));
    }
    let select = link_tests::Entity::find()
        .filter(filter)
        .order_by_desc(link_tests::Column::Id);
    let total = select.clone().count(db).await?;
    let rows = select
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all(db)
        .await?;
    let ids: Vec<i64> = rows.iter().map(|row| row.id).collect();
    let legs = legs_of(db, &ids).await?;
    let user_ids: Vec<i64> = rows.iter().filter_map(|row| row.user_id).collect();
    let users = user_refs(db, user_ids).await?;
    Ok(LinkTestPage {
        items: rows
            .iter()
            .map(|row| {
                let requested_by = row.user_id.and_then(|id| users.get(&id).cloned());
                view::test(row, &legs, requested_by, false)
            })
            .collect(),
        total,
        page,
        per_page,
    })
}

/// Testes que ainda constam como em curso.
///
/// # Errors
///
/// Erro do banco.
pub async fn active<C: ConnectionTrait>(db: &C) -> AppResult<Vec<link_tests::Model>> {
    Ok(link_tests::Entity::find()
        .filter(link_tests::Column::Status.is_in([
            as_db(&LinkTestStatus::Queued),
            as_db(&LinkTestStatus::Running),
        ]))
        .all(db)
        .await?)
}

/// Pernas com servidor ainda de pé num equipamento.
///
/// # Errors
///
/// Erro do banco.
pub async fn pending_cleanups<C: ConnectionTrait>(db: &C) -> AppResult<Vec<link_test_legs::Model>> {
    Ok(link_test_legs::Entity::find()
        .filter(link_test_legs::Column::Cleanup.is_not_null())
        .all(db)
        .await?)
}

/// Apaga os testes terminados antes de `cutoff` (com as pernas). Teste em
/// curso nunca envelhece.
///
/// # Errors
///
/// Erro do banco.
pub async fn prune<C: ConnectionTrait>(db: &C, cutoff: DateTime<Utc>) -> AppResult<u64> {
    let mut deleted = 0;
    loop {
        let ids: Vec<i64> = link_tests::Entity::find()
            .select_only()
            .column(link_tests::Column::Id)
            .filter(link_tests::Column::FinishedAt.lt(cutoff))
            .order_by_asc(link_tests::Column::Id)
            .limit(PRUNE_BATCH)
            .into_tuple::<i64>()
            .all(db)
            .await?;
        if ids.is_empty() {
            return Ok(deleted);
        }
        // Pernas antes: o SQLite só aplica o `CASCADE` com `foreign_keys=ON`.
        link_test_legs::Entity::delete_many()
            .filter(link_test_legs::Column::TestId.is_in(ids.clone()))
            .exec(db)
            .await?;
        deleted += link_tests::Entity::delete_many()
            .filter(link_tests::Column::Id.is_in(ids))
            .exec(db)
            .await?
            .rows_affected;
    }
}
