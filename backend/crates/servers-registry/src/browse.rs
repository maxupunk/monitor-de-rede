//! O que a tela mostra, montado a partir do catálogo (sem HTTP aqui).

use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde_json::Value;

use netmonitor_core::{
    models::{
        deploy_applications, registry_blobs, registry_exceptions, registry_feeds,
        registry_findings, registry_manifests, registry_repositories, registry_repository_blobs,
        registry_tags, users,
    },
    shared::{
        errors::{AppError, AppResult},
        transaction::begin_write,
    },
};

use netmonitor_servers::deploys::usage::images_in_use;

use crate::scan::advisory::Severity;
use crate::{
    catalog, manifest, names, settings, tokens,
    views::{
        RegistryExceptionView, RegistryFeedView, RegistryFindingView, RegistryOverview,
        RegistryRepositoryDetail, RegistryRepositoryView, RegistryScanSummary, RegistryTokenView,
        RegistryVersionView,
    },
};

fn summary(row: &registry_manifests::Model) -> Option<RegistryScanSummary> {
    let status = row.scan_status.clone()?;
    let scan = row.scan.clone().unwrap_or(Value::Null);
    let count = |key: &str| {
        scan.get(key)
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(0)
    };
    Some(RegistryScanSummary {
        status,
        critical: count("critical"),
        high: count("high"),
        medium: count("medium"),
        low: count("low"),
        unknown: count("unknown"),
        packages: row.sbom_packages,
        source: scan
            .get("source")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        scanned_at: row.scanned_at.map(|at| at.to_rfc3339()),
    })
}

async fn applications(
    db: &DatabaseConnection,
) -> AppResult<HashMap<i64, deploy_applications::Model>> {
    Ok(deploy_applications::Entity::find()
        .all(db)
        .await?
        .into_iter()
        .map(|app| (app.id, app))
        .collect())
}

async fn repository_view(
    db: &DatabaseConnection,
    repo: &registry_repositories::Model,
    apps: &HashMap<i64, deploy_applications::Model>,
    sizes: &HashMap<String, i64>,
) -> AppResult<RegistryRepositoryView> {
    let tags = registry_tags::Entity::find()
        .filter(registry_tags::Column::RepositoryId.eq(repo.id))
        .order_by_desc(registry_tags::Column::UpdatedAt)
        .all(db)
        .await?;
    let versions: HashSet<&str> = tags
        .iter()
        .map(|tag| tag.manifest_digest.as_str())
        .collect();
    let bytes = registry_repository_blobs::Entity::find()
        .filter(registry_repository_blobs::Column::RepositoryId.eq(repo.id))
        .all(db)
        .await?
        .iter()
        .filter_map(|link| sizes.get(&link.digest))
        .sum();
    let latest = match tags.first() {
        Some(tag) => catalog::manifest_by_digest(db, repo.id, &tag.manifest_digest).await?,
        None => None,
    };
    Ok(RegistryRepositoryView {
        id: repo.id,
        name: repo.name.clone(),
        application_id: repo.application_id,
        application: repo
            .application_id
            .and_then(|id| apps.get(&id))
            .map(|app| app.name.clone()),
        versions: u32::try_from(versions.len()).unwrap_or(u32::MAX),
        bytes,
        latest_tag: tags.first().map(|tag| tag.name.clone()),
        last_push: tags.first().map(|tag| tag.updated_at.to_rfc3339()),
        latest_scan: latest.as_ref().and_then(summary),
    })
}

async fn blob_sizes(db: &DatabaseConnection) -> AppResult<HashMap<String, i64>> {
    Ok(registry_blobs::Entity::find()
        .all(db)
        .await?
        .into_iter()
        .map(|blob| (blob.digest, blob.size))
        .collect())
}

/// A visão geral.
///
/// # Errors
///
/// Erro do banco.
pub async fn overview(db: &DatabaseConnection) -> AppResult<RegistryOverview> {
    let settings = settings::load(db).await?;
    let apps = applications(db).await?;
    let sizes = blob_sizes(db).await?;
    let mut repositories = Vec::new();
    for repo in registry_repositories::Entity::find()
        .order_by_asc(registry_repositories::Column::Name)
        .all(db)
        .await?
    {
        repositories.push(repository_view(db, &repo, &apps, &sizes).await?);
    }
    let feeds = registry_feeds::Entity::find()
        .order_by_asc(registry_feeds::Column::Ecosystem)
        .all(db)
        .await?
        .into_iter()
        .map(|feed| RegistryFeedView {
            ecosystem: feed.ecosystem,
            synced_at: feed.synced_at.map(|at| at.to_rfc3339()),
            entries: feed.entries,
            error: feed.error,
        })
        .collect();
    Ok(RegistryOverview {
        quota_bytes: settings.quota_bytes(),
        used_bytes: settings::usage(db).await?,
        settings,
        repositories,
        feeds,
    })
}

/// As versões de um repositório.
///
/// # Errors
///
/// `NotFound`; erro do banco.
pub async fn detail(db: &DatabaseConnection, id: i64) -> AppResult<RegistryRepositoryDetail> {
    let repo = registry_repositories::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::not_found("Repositório não encontrado"))?;
    let apps = applications(db).await?;
    let sizes = blob_sizes(db).await?;
    let usage = images_in_use(db).await?;
    let tags = registry_tags::Entity::find()
        .filter(registry_tags::Column::RepositoryId.eq(repo.id))
        .all(db)
        .await?;
    let mut by_digest: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for tag in tags {
        by_digest
            .entry(tag.manifest_digest)
            .or_default()
            .push(tag.name);
    }
    let mut versions = Vec::new();
    for row in registry_manifests::Entity::find()
        .filter(registry_manifests::Column::RepositoryId.eq(repo.id))
        .order_by_desc(registry_manifests::Column::CreatedAt)
        .all(db)
        .await?
    {
        let Some(mut row_tags) = by_digest.remove(&row.digest) else {
            continue;
        };
        row_tags.sort();
        let platforms = manifest::parse(row.content.as_bytes(), Some(&row.media_type))
            .map(|parsed| match &parsed.kind {
                manifest::Kind::Index { .. } => parsed
                    .images()
                    .iter()
                    .filter_map(|image| image.platform.as_ref().map(manifest::Platform::label))
                    .collect(),
                manifest::Kind::Image { .. } => row.platform.clone().into_iter().collect(),
            })
            .unwrap_or_default();
        let used_by: Vec<String> = row_tags
            .iter()
            .flat_map(|tag| usage.live_in(&repo.name, tag).iter().cloned())
            .collect();
        let protected = row_tags
            .iter()
            .any(|tag| usage.is_protected(&repo.name, tag));
        versions.push(RegistryVersionView {
            id: row.id,
            digest: row.digest.clone(),
            immutable: row_tags.iter().any(|tag| names::is_commit_tag(tag)),
            tags: row_tags,
            media_type: row.media_type.clone(),
            platforms,
            bytes: row.total_size,
            pushed_by: row.pushed_by.clone(),
            provenance: row.provenance.clone(),
            pinned: row.pinned,
            created_at: row.created_at.to_rfc3339(),
            scan: summary(&row),
            used_by,
            protected,
        });
    }
    Ok(RegistryRepositoryDetail {
        repository: repository_view(db, &repo, &apps, &sizes).await?,
        versions,
    })
}

/// Uma versão pelo id.
///
/// # Errors
///
/// `NotFound`.
pub async fn version(db: &DatabaseConnection, id: i64) -> AppResult<registry_manifests::Model> {
    registry_manifests::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::not_found("Versão não encontrada"))
}

/// Os achados de uma versão, marcando os cobertos por exceção.
///
/// # Errors
///
/// Erro do banco.
pub async fn findings(db: &DatabaseConnection, id: i64) -> AppResult<Vec<RegistryFindingView>> {
    let manifest = version(db, id).await?;
    let repo = registry_repositories::Entity::find_by_id(manifest.repository_id)
        .one(db)
        .await?;
    let application_id = repo.and_then(|repo| repo.application_id);
    let exceptions = crate::scan::Exceptions::active(db, application_id).await?;
    let mut rows = registry_findings::Entity::find()
        .filter(registry_findings::Column::ManifestId.eq(manifest.id))
        .all(db)
        .await?;
    // Mais grave primeiro (a ordem é a do próprio `Severity`).
    rows.sort_by(|a, b| {
        Severity::parse(&b.severity)
            .cmp(&Severity::parse(&a.severity))
            .then(a.package.cmp(&b.package))
    });
    Ok(rows
        .iter()
        .map(|row| RegistryFindingView::new(row, exceptions.covers(row)))
        .collect())
}

/// Fixa ou solta uma versão.
///
/// # Errors
///
/// `NotFound`.
pub async fn pin(
    db: &DatabaseConnection,
    id: i64,
    pinned: bool,
) -> AppResult<registry_manifests::Model> {
    let row = version(db, id).await?;
    let mut active: registry_manifests::ActiveModel = row.into();
    active.pinned = Set(pinned);
    Ok(active.update(db).await?)
}

/// Apaga um manifest ou uma tag de um repositório, com as proteções de
/// sempre: o que um alvo usa (ou guarda para o rollback), o que está fixado e
/// a tag de commit (imutável) não saem. Vale para a tela e para o `DELETE`
/// da API `/v2`.
///
/// # Errors
///
/// `Conflict` com o motivo; erro do banco.
pub async fn remove_reference(
    db: &DatabaseConnection,
    repo: &registry_repositories::Model,
    reference: &str,
) -> AppResult<bool> {
    let usage = images_in_use(db).await?;
    if reference.parse::<crate::digest::Digest>().is_err() {
        if names::is_commit_tag(reference) {
            return Err(AppError::conflict(
                "Tag de commit é imutável: apague a versão inteira na tela do Registry",
            ));
        }
        if usage.is_protected(&repo.name, reference) {
            return Err(AppError::conflict(IN_USE));
        }
        return catalog::delete_tag(db, repo.id, reference).await;
    }
    let Some(row) = catalog::manifest_by_digest(db, repo.id, reference).await? else {
        return Ok(false);
    };
    if row.pinned {
        return Err(AppError::conflict(
            "Esta versão está fixada: solte-a antes de apagar",
        ));
    }
    let tags = catalog::tags_of(db, repo.id, &row.digest).await?;
    if tags.iter().any(|tag| usage.is_protected(&repo.name, tag)) {
        return Err(AppError::conflict(IN_USE));
    }
    // As tags e o manifest saem juntos.
    let txn = begin_write(db).await?;
    let removed = catalog::delete_manifest(&txn, repo.id, &row.digest).await?;
    txn.commit().await?;
    Ok(removed)
}

const IN_USE: &str =
    "Esta versão está no ar (ou guardada para o rollback) em algum alvo: não pode ser apagada";

/// Apaga uma versão pela tela.
///
/// # Errors
///
/// `Conflict` quando protegida; `NotFound`.
pub async fn remove(db: &DatabaseConnection, id: i64) -> AppResult<registry_manifests::Model> {
    let row = version(db, id).await?;
    let repo = registry_repositories::Entity::find_by_id(row.repository_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::not_found("Repositório não encontrado"))?;
    remove_reference(db, &repo, &row.digest).await?;
    Ok(row)
}

/// Os tokens (de quem pede, ou de todos para o administrador).
///
/// # Errors
///
/// Erro do banco.
pub async fn token_views(
    db: &DatabaseConnection,
    user_id: Option<i64>,
) -> AppResult<Vec<RegistryTokenView>> {
    let owners: HashMap<i64, String> = users::Entity::find()
        .all(db)
        .await?
        .into_iter()
        .map(|user| (user.id, user.name))
        .collect();
    Ok(tokens::list(db, user_id)
        .await?
        .iter()
        .map(|row| {
            RegistryTokenView::new(row, owners.get(&row.user_id).cloned().unwrap_or_default())
        })
        .collect())
}

/// As exceções.
///
/// # Errors
///
/// Erro do banco.
pub async fn exception_views(db: &DatabaseConnection) -> AppResult<Vec<RegistryExceptionView>> {
    let apps = applications(db).await?;
    Ok(registry_exceptions::Entity::find()
        .order_by_desc(registry_exceptions::Column::Id)
        .all(db)
        .await?
        .iter()
        .map(|row| {
            RegistryExceptionView::new(
                row,
                row.application_id
                    .and_then(|id| apps.get(&id))
                    .map(|app| app.name.clone()),
            )
        })
        .collect())
}

/// Registra uma exceção.
///
/// # Errors
///
/// `Validation`.
pub async fn add_exception(
    db: &DatabaseConnection,
    application_id: Option<i64>,
    advisory: &str,
    reason: &str,
    days: i64,
    by: &str,
) -> AppResult<registry_exceptions::Model> {
    let advisory = advisory.trim();
    if advisory.is_empty() || advisory.len() > 64 {
        return Err(AppError::validation(
            "Informe o identificador (CVE, GHSA, DSA…)",
        ));
    }
    if reason.trim().len() < 10 {
        return Err(AppError::validation(
            "Explique o motivo da exceção (ao menos 10 caracteres): ele fica na auditoria",
        ));
    }
    if !(1..=365).contains(&days) {
        return Err(AppError::validation("A exceção vale de 1 a 365 dias"));
    }
    let now = Utc::now();
    Ok(registry_exceptions::ActiveModel {
        application_id: Set(application_id),
        advisory: Set(advisory.to_string()),
        reason: Set(reason.trim().to_string()),
        expires_at: Set((now + chrono::Duration::days(days)).into()),
        created_by: Set(by.to_string()),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
        ..Default::default()
    }
    .insert(db)
    .await?)
}

/// Remove uma exceção.
///
/// # Errors
///
/// `NotFound`.
pub async fn remove_exception(db: &DatabaseConnection, id: i64) -> AppResult<()> {
    let removed = registry_exceptions::Entity::delete_by_id(id)
        .exec(db)
        .await?;
    if removed.rows_affected == 0 {
        return Err(AppError::not_found("Exceção não encontrada"));
    }
    Ok(())
}
