//! Compilação no servidor (ADR 023): o push chega, o agente compila, a imagem
//! vai ao registry da central e o deploy segue a fila do ADR 021.
//!
//! * [`push`] — o aviso do GitHub App (assinado) vira compilações;
//! * [`runner`] — uma compilação do começo ao fim, com console ao vivo;
//! * [`secrets`] — os segredos de build da aplicação, cifrados.
//!
//! Estados: `queued` → `running` → `succeeded`/`failed`/`cancelled`; uma
//! compilação na fila vira `superseded` quando chega outra mais nova da mesma
//! aplicação e ref (a mesma regra de substituição do deploy).

pub mod push;
pub mod runner;
pub mod secrets;

use chrono::Utc;
use loco_rs::app::AppContext;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use ts_rs::TS;

use netmonitor_core::{
    events::EventBus,
    models::{deploy_applications, deploy_builds, probes},
    shared::{
        errors::{AppError, AppResult},
        transaction::begin_write,
    },
};

use super::images::StoredImage;

pub use agent_protocol::docker::images::BuildStage;

pub const BUILD_EVENT: &str = "deploys:build";
pub const BUILD_LOG_EVENT: &str = "deploys:build-log";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(
    export,
    export_to = "../../frontend/src/bindings/",
    rename = "DeployBuildStatus"
)]
pub enum BuildStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    /// Uma compilação mais nova da mesma aplicação e ref chegou antes desta
    /// começar.
    Superseded,
}

impl BuildStatus {
    pub const ALL: [Self; 6] = [
        Self::Queued,
        Self::Running,
        Self::Succeeded,
        Self::Failed,
        Self::Cancelled,
        Self::Superseded,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Superseded => "superseded",
        }
    }

    #[must_use]
    pub fn parse(raw: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|status| status.as_str() == raw)
            .unwrap_or(Self::Failed)
    }

    #[must_use]
    pub const fn is_terminal(self) -> bool {
        !matches!(self, Self::Queued | Self::Running)
    }
}

/// Quem pediu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(
    export,
    export_to = "../../frontend/src/bindings/",
    rename = "DeployBuildTrigger"
)]
pub enum BuildTrigger {
    /// O GitHub avisou um push.
    Push,
    /// "Compilar agora".
    Manual,
}

impl BuildTrigger {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Push => "push",
            Self::Manual => "manual",
        }
    }
}

/// Uma compilação, como a tela a vê.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../frontend/src/bindings/",
    rename = "DeployServerBuildView"
)]
pub struct BuildView {
    #[ts(type = "number")]
    pub id: i64,
    #[ts(type = "number")]
    pub application_id: i64,
    pub application: String,
    pub reference: Option<String>,
    pub commit_sha: String,
    /// O commit abreviado, como as outras visões de deploy.
    pub short_sha: String,
    pub commit_message: Option<String>,
    pub trigger: String,
    pub triggered_by: Option<String>,
    pub agent: Option<String>,
    pub status: BuildStatus,
    pub stage: Option<BuildStage>,
    pub current_image: Option<String>,
    pub no_cache: bool,
    pub images: Vec<StoredImage>,
    pub error: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    #[ts(type = "number | null")]
    pub duration_ms: Option<i64>,
    pub created_at: String,
}

fn stage_of(raw: Option<&str>) -> Option<BuildStage> {
    raw.and_then(|raw| serde_json::from_value(json!(raw)).ok())
}

/// # Errors
///
/// Banco.
pub async fn view<C: ConnectionTrait>(
    db: &C,
    build: &deploy_builds::Model,
) -> AppResult<BuildView> {
    let application = deploy_applications::Entity::find_by_id(build.application_id)
        .one(db)
        .await?
        .map(|app| app.name)
        .unwrap_or_default();
    let agent = match build.probe_id {
        Some(id) => probes::Entity::find_by_id(id)
            .one(db)
            .await?
            .map(|probe| probe.name),
        None => None,
    };
    Ok(BuildView {
        id: build.id,
        application_id: build.application_id,
        application,
        reference: build.reference.clone(),
        commit_sha: build.commit_sha.clone(),
        short_sha: super::short_sha(&build.commit_sha).to_string(),
        commit_message: build.commit_message.clone(),
        trigger: build.trigger.clone(),
        triggered_by: build.triggered_by.clone(),
        agent,
        status: BuildStatus::parse(&build.status),
        stage: stage_of(build.stage.as_deref()),
        current_image: build.current_image.clone(),
        no_cache: build.no_cache,
        images: serde_json::from_value(build.images.clone()).unwrap_or_default(),
        error: build.error.clone(),
        started_at: build.started_at.map(|at| at.to_rfc3339()),
        finished_at: build.finished_at.map(|at| at.to_rfc3339()),
        duration_ms: build.duration_ms,
        created_at: build.created_at.to_rfc3339(),
    })
}

fn snapshot_key(id: i64) -> String {
    format!("deploy-build:{id}")
}

/// Publica o estado (snapshot enquanto em curso; quem abre a tela no meio
/// recebe as compilações em andamento ao conectar).
pub async fn publish(ctx: &AppContext, id: i64) {
    let Ok(bus) = EventBus::from_context(ctx) else {
        return;
    };
    let Ok(Some(build)) = deploy_builds::Entity::find_by_id(id).one(&ctx.db).await else {
        return;
    };
    let Ok(view) = view(&ctx.db, &build).await else {
        return;
    };
    let key = snapshot_key(id);
    bus.publish_snapshot(BUILD_EVENT, key.clone(), json!(view));
    if view.status.is_terminal() {
        bus.forget_snapshots(&key);
    }
}

pub fn publish_lines(ctx: &AppContext, id: i64, lines: Vec<String>) {
    if lines.is_empty() {
        return;
    }
    if let Ok(bus) = EventBus::from_context(ctx) {
        bus.publish_ephemeral(BUILD_LOG_EVENT, json!({ "buildId": id, "lines": lines }));
    }
}

/// # Errors
///
/// `404`.
pub async fn find<C: ConnectionTrait>(db: &C, id: i64) -> AppResult<deploy_builds::Model> {
    deploy_builds::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::not_found("Compilação não encontrada"))
}

/// As mais recentes (de uma aplicação ou de todas).
///
/// # Errors
///
/// Banco.
pub async fn recent<C: ConnectionTrait>(
    db: &C,
    application_id: Option<i64>,
    limit: u64,
) -> AppResult<Vec<deploy_builds::Model>> {
    let mut query = deploy_builds::Entity::find().order_by_desc(deploy_builds::Column::Id);
    if let Some(id) = application_id {
        query = query.filter(deploy_builds::Column::ApplicationId.eq(id));
    }
    Ok(query.limit(limit).all(db).await?)
}

/// O pedido de uma compilação.
pub struct Ask {
    pub application_id: i64,
    pub reference: Option<String>,
    pub commit_sha: String,
    pub commit_message: Option<String>,
    pub trigger: BuildTrigger,
    pub triggered_by: Option<String>,
    pub delivery_id: Option<String>,
    pub no_cache: bool,
}

/// Enfileira e dispara. O mesmo aviso (`delivery_id`) não compila duas vezes,
/// e o mesmo commit já compilado (ou em curso) também não — salvo pedido
/// manual.
///
/// # Errors
///
/// Banco.
pub async fn request(ctx: &AppContext, ask: Ask) -> AppResult<Option<deploy_builds::Model>> {
    let commit = ask.commit_sha.to_lowercase();
    // Conferir, substituir e inserir numa transação só: dois avisos (ou dois
    // cliques) ao mesmo tempo não viram duas compilações.
    let txn = begin_write(&ctx.db).await?;
    if let Some(delivery) = &ask.delivery_id {
        let seen = deploy_builds::Entity::find()
            .filter(deploy_builds::Column::DeliveryId.eq(delivery.as_str()))
            .one(&txn)
            .await?;
        if seen.is_some() {
            return Ok(None);
        }
    }
    // O mesmo commit já na fila ou compilando não compila de novo; pelo push,
    // nem o já compilado (o pedido manual pode recompilar o que terminou).
    let busy: &[&str] = if ask.trigger == BuildTrigger::Push {
        &["queued", "running", "succeeded"]
    } else {
        &["queued", "running"]
    };
    let same = deploy_builds::Entity::find()
        .filter(deploy_builds::Column::ApplicationId.eq(ask.application_id))
        .filter(deploy_builds::Column::CommitSha.eq(commit.as_str()))
        .filter(deploy_builds::Column::Status.is_in(busy.iter().copied()))
        .one(&txn)
        .await?;
    if same.is_some() {
        return Ok(None);
    }
    // A que ainda espera, da mesma aplicação e ref, sai da fila.
    let mut waiting = deploy_builds::Entity::find()
        .filter(deploy_builds::Column::ApplicationId.eq(ask.application_id))
        .filter(deploy_builds::Column::Status.eq("queued"));
    waiting = match &ask.reference {
        Some(reference) => waiting.filter(deploy_builds::Column::Reference.eq(reference.as_str())),
        None => waiting.filter(deploy_builds::Column::Reference.is_null()),
    };
    let mut superseded = Vec::new();
    for old in waiting.all(&txn).await? {
        superseded.push(old.id);
        let mut active: deploy_builds::ActiveModel = old.into();
        active.status = Set(BuildStatus::Superseded.as_str().to_string());
        active.finished_at = Set(Some(Utc::now().into()));
        active.update(&txn).await?;
    }
    let now = Utc::now().into();
    let build = deploy_builds::ActiveModel {
        application_id: Set(ask.application_id),
        reference: Set(ask
            .reference
            .map(|reference| reference.chars().take(120).collect())),
        commit_sha: Set(commit),
        commit_message: Set(ask
            .commit_message
            .map(|message| message.chars().take(2000).collect())),
        trigger: Set(ask.trigger.as_str().to_string()),
        triggered_by: Set(ask.triggered_by.map(|by| by.chars().take(120).collect())),
        probe_id: Set(None),
        status: Set(BuildStatus::Queued.as_str().to_string()),
        stage: Set(None),
        current_image: Set(None),
        no_cache: Set(ask.no_cache),
        delivery_id: Set(ask.delivery_id),
        images: Set(json!([])),
        error: Set(None),
        log: Set(None),
        started_at: Set(None),
        finished_at: Set(None),
        duration_ms: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(&txn)
    .await?;
    txn.commit().await?;
    for id in superseded {
        publish(ctx, id).await;
    }
    publish(ctx, build.id).await;
    runner::spawn(ctx, build.id);
    Ok(Some(build))
}

#[derive(Debug, Deserialize)]
struct Head {
    sha: String,
    commit: HeadCommit,
}

#[derive(Debug, Deserialize)]
struct HeadCommit {
    #[serde(default)]
    message: Option<String>,
}

/// O commit mais novo de uma branch (e a mensagem), pelo acesso ao GitHub da
/// aplicação — o "Compilar agora" sem commit escolhido.
///
/// # Errors
///
/// GitHub inacessível, branch inexistente.
pub async fn head_of<C: ConnectionTrait>(
    db: &C,
    application: &deploy_applications::Model,
    branch: &str,
) -> AppResult<(String, Option<String>)> {
    let client = super::access::repository_client(
        db,
        super::access::Source::of(application),
        &application.repository,
        super::github::app::Scope::Read,
    )
    .await?;
    let head: Head = client
        .get(&format!(
            "/repos/{}/commits/{}",
            application.repository,
            super::github::encode(branch)
        ))
        .await
        .map_err(AppError::from)?;
    Ok((head.sha.to_lowercase(), head.commit.message))
}

/// O que a tela manda no "Compilar agora".
pub struct ManualBuild<'a> {
    /// Branch; vazia, a primeira que um alvo acompanha.
    pub reference: Option<&'a str>,
    /// Commit exato; vazio, o mais novo da branch.
    pub commit: Option<&'a str>,
    pub no_cache: bool,
}

/// "Compilar agora": resolve a branch e o commit e enfileira.
///
/// # Errors
///
/// `Validation` sem branch ou com commit inválido; `Conflict` quando esse
/// commit já está compilando; GitHub inacessível.
pub async fn request_manual(
    ctx: &AppContext,
    application: &deploy_applications::Model,
    input: &ManualBuild<'_>,
    by: &str,
) -> AppResult<deploy_builds::Model> {
    let reference = match input.reference.map(str::trim).filter(|r| !r.is_empty()) {
        Some(reference) => reference.to_string(),
        None => super::applications::targets_of(&ctx.db, application.id)
            .await?
            .into_iter()
            .map(|target| target.reference)
            .find(|reference| !reference.contains('*'))
            .ok_or_else(|| AppError::validation("Informe a branch a compilar"))?,
    };
    let (commit, message) = match input.commit.map(str::trim).filter(|c| !c.is_empty()) {
        Some(commit) => (commit.to_lowercase(), None),
        None => head_of(&ctx.db, application, &reference).await?,
    };
    agent_protocol::docker::stacks::validate_commit(&commit)
        .map_err(|error| AppError::validation(error.to_string()))?;
    request(
        ctx,
        Ask {
            application_id: application.id,
            reference: Some(format!("refs/heads/{reference}")),
            commit_sha: commit,
            commit_message: message,
            trigger: BuildTrigger::Manual,
            triggered_by: Some(by.to_string()),
            delivery_id: None,
            no_cache: input.no_cache,
        },
    )
    .await?
    .ok_or_else(|| AppError::conflict("Esta versão já está na fila ou compilando"))
}

/// "Recompilar sem cache": o mesmo commit e ref de uma compilação anterior.
///
/// # Errors
///
/// `404`; `Conflict` quando esse commit já está compilando.
pub async fn rebuild(ctx: &AppContext, id: i64, by: &str) -> AppResult<deploy_builds::Model> {
    let previous = find(&ctx.db, id).await?;
    request(
        ctx,
        Ask {
            application_id: previous.application_id,
            reference: previous.reference,
            commit_sha: previous.commit_sha,
            commit_message: previous.commit_message,
            trigger: BuildTrigger::Manual,
            triggered_by: Some(by.to_string()),
            delivery_id: None,
            no_cache: true,
        },
    )
    .await?
    .ok_or_else(|| AppError::conflict("Esta versão já está na fila ou compilando"))
}

/// A branch (ou tag) de uma compilação, como a tela mostra.
#[must_use]
pub fn branch_of(build: &deploy_builds::Model) -> String {
    build
        .reference
        .as_deref()
        .and_then(runner::parse_ref)
        .map(|reference| reference.name().to_string())
        .unwrap_or_default()
}

/// Cancela uma compilação na fila ou em curso.
///
/// # Errors
///
/// `404`; `Conflict` quando já terminou.
pub async fn cancel(ctx: &AppContext, id: i64) -> AppResult<deploy_builds::Model> {
    let build = find(&ctx.db, id).await?;
    if BuildStatus::parse(&build.status).is_terminal() {
        return Err(AppError::conflict("Esta compilação já terminou"));
    }
    runner::cancel(id);
    let mut active: deploy_builds::ActiveModel = build.into();
    active.status = Set(BuildStatus::Cancelled.as_str().to_string());
    active.finished_at = Set(Some(Utc::now().into()));
    let saved = active.update(&ctx.db).await?;
    publish(ctx, id).await;
    Ok(saved)
}

/// Na subida da central: o que estava em curso morreu com ela.
///
/// # Errors
///
/// Banco.
pub async fn recover(ctx: &AppContext) -> AppResult<()> {
    let stuck = deploy_builds::Entity::find()
        .filter(deploy_builds::Column::Status.is_in(["queued", "running"]))
        .all(&ctx.db)
        .await?;
    for build in stuck {
        if build.status == "queued" {
            runner::spawn(ctx, build.id);
            continue;
        }
        let mut active: deploy_builds::ActiveModel = build.into();
        active.status = Set(BuildStatus::Failed.as_str().to_string());
        active.error = Set(Some(
            "A central reiniciou durante a compilação: peça de novo".to_string(),
        ));
        active.finished_at = Set(Some(Utc::now().into()));
        active.update(&ctx.db).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estados_ida_e_volta() {
        for status in BuildStatus::ALL {
            assert_eq!(BuildStatus::parse(status.as_str()), status);
        }
        assert!(BuildStatus::Superseded.is_terminal());
        assert!(!BuildStatus::Queued.is_terminal());
    }

    #[test]
    fn etapa_gravada_volta() {
        assert_eq!(stage_of(Some("store")), Some(BuildStage::Store));
        assert_eq!(stage_of(Some("??")), None);
    }
}
