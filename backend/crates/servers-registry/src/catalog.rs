//! O catálogo: repositórios, blobs vinculados, manifests e tags (RF-01..04).
//!
//! Um blob existe uma vez no storage, mas só é **legível** pelos repositórios
//! que o vinculam (RS-06): por envio, por montagem autorizada ou por ingestão.
//! Conhecer um digest não dá acesso a ele.

use chrono::Utc;
use sea_orm::{
    sea_query::OnConflict, ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use serde_json::Value;

use netmonitor_core::{
    models::{
        registry_blobs, registry_manifests, registry_repositories, registry_repository_blobs,
        registry_tags,
    },
    shared::errors::{AppError, AppResult},
};

use crate::{
    digest::Digest,
    manifest::{self, Manifest},
    names,
};

/// O repositório pelo nome.
///
/// # Errors
///
/// Erro do banco.
pub async fn repository<C: ConnectionTrait>(
    db: &C,
    name: &str,
) -> AppResult<Option<registry_repositories::Model>> {
    Ok(registry_repositories::Entity::find()
        .filter(registry_repositories::Column::Name.eq(name))
        .one(db)
        .await?)
}

/// O repositório, criado se ainda não existe.
///
/// # Errors
///
/// `Validation` com nome fora da regra; erro do banco.
pub async fn ensure_repository<C: ConnectionTrait>(
    db: &C,
    name: &str,
    application_id: Option<i64>,
) -> AppResult<registry_repositories::Model> {
    if !names::valid_name(name) {
        return Err(AppError::validation(format!(
            "Nome de repositório inválido: {name}"
        )));
    }
    if let Some(found) = repository(db, name).await? {
        if found.application_id.is_none() && application_id.is_some() {
            let mut active: registry_repositories::ActiveModel = found.into();
            active.application_id = Set(application_id);
            return Ok(active.update(db).await?);
        }
        return Ok(found);
    }
    let now = Utc::now().into();
    registry_repositories::Entity::insert(registry_repositories::ActiveModel {
        name: Set(name.to_string()),
        application_id: Set(application_id),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    })
    .on_conflict(
        OnConflict::column(registry_repositories::Column::Name)
            .do_nothing()
            .to_owned(),
    )
    .exec_without_returning(db)
    .await?;
    repository(db, name)
        .await?
        .ok_or_else(|| AppError::not_found("Repositório não encontrado"))
}

/// Registra o blob (se novo) e o vincula ao repositório.
///
/// # Errors
///
/// Erro do banco.
pub async fn link_blob<C: ConnectionTrait>(
    db: &C,
    repository_id: i64,
    digest: &Digest,
    size: u64,
) -> AppResult<()> {
    let now = Utc::now().into();
    registry_blobs::Entity::insert(registry_blobs::ActiveModel {
        digest: Set(digest.to_string()),
        size: Set(i64::try_from(size).unwrap_or(i64::MAX)),
        created_at: Set(now),
        ..Default::default()
    })
    .on_conflict(
        OnConflict::column(registry_blobs::Column::Digest)
            .do_nothing()
            .to_owned(),
    )
    .exec_without_returning(db)
    .await?;
    registry_repository_blobs::Entity::insert(registry_repository_blobs::ActiveModel {
        repository_id: Set(repository_id),
        digest: Set(digest.to_string()),
        created_at: Set(now),
        ..Default::default()
    })
    .on_conflict(
        OnConflict::columns([
            registry_repository_blobs::Column::RepositoryId,
            registry_repository_blobs::Column::Digest,
        ])
        .do_nothing()
        .to_owned(),
    )
    .exec_without_returning(db)
    .await?;
    Ok(())
}

/// O tamanho de um blob, se ele é legível neste repositório.
///
/// # Errors
///
/// Erro do banco.
pub async fn linked_blob<C: ConnectionTrait>(
    db: &C,
    repository_id: i64,
    digest: &Digest,
) -> AppResult<Option<u64>> {
    let linked = registry_repository_blobs::Entity::find()
        .filter(registry_repository_blobs::Column::RepositoryId.eq(repository_id))
        .filter(registry_repository_blobs::Column::Digest.eq(digest.as_str()))
        .one(db)
        .await?
        .is_some();
    if !linked {
        return Ok(None);
    }
    Ok(registry_blobs::Entity::find()
        .filter(registry_blobs::Column::Digest.eq(digest.as_str()))
        .one(db)
        .await?
        .map(|blob| u64::try_from(blob.size).unwrap_or(0)))
}

/// Tira o vínculo de um blob com o repositório (o arquivo fica para a coleta).
///
/// # Errors
///
/// Erro do banco.
pub async fn unlink_blob<C: ConnectionTrait>(
    db: &C,
    repository_id: i64,
    digest: &Digest,
) -> AppResult<bool> {
    let result = registry_repository_blobs::Entity::delete_many()
        .filter(registry_repository_blobs::Column::RepositoryId.eq(repository_id))
        .filter(registry_repository_blobs::Column::Digest.eq(digest.as_str()))
        .exec(db)
        .await?;
    Ok(result.rows_affected > 0)
}

/// O manifest, pelo digest, neste repositório.
///
/// # Errors
///
/// Erro do banco.
pub async fn manifest_by_digest<C: ConnectionTrait>(
    db: &C,
    repository_id: i64,
    digest: &str,
) -> AppResult<Option<registry_manifests::Model>> {
    Ok(registry_manifests::Entity::find()
        .filter(registry_manifests::Column::RepositoryId.eq(repository_id))
        .filter(registry_manifests::Column::Digest.eq(digest))
        .one(db)
        .await?)
}

/// O manifest de uma referência: digest (`sha256:…`) ou tag.
///
/// # Errors
///
/// Erro do banco.
pub async fn resolve<C: ConnectionTrait>(
    db: &C,
    repository_id: i64,
    reference: &str,
) -> AppResult<Option<registry_manifests::Model>> {
    if reference.parse::<Digest>().is_ok() {
        return manifest_by_digest(db, repository_id, reference).await;
    }
    let Some(tag) = tag(db, repository_id, reference).await? else {
        return Ok(None);
    };
    manifest_by_digest(db, repository_id, &tag.manifest_digest).await
}

/// Uma tag.
///
/// # Errors
///
/// Erro do banco.
pub async fn tag<C: ConnectionTrait>(
    db: &C,
    repository_id: i64,
    name: &str,
) -> AppResult<Option<registry_tags::Model>> {
    Ok(registry_tags::Entity::find()
        .filter(registry_tags::Column::RepositoryId.eq(repository_id))
        .filter(registry_tags::Column::Name.eq(name))
        .one(db)
        .await?)
}

/// Quem envia o manifest, para o registro.
#[derive(Debug, Clone, Default)]
pub struct Push {
    pub pushed_by: Option<String>,
    pub provenance: Option<Value>,
}

/// Grava um manifest já conferido: o texto exato, o tipo e o que ele soma.
/// O que ele referencia precisa estar no repositório (blobs vinculados,
/// manifests filhos).
///
/// # Errors
///
/// `Validation` (`MANIFEST_BLOB_UNKNOWN`) quando falta uma referência; erro
/// do banco.
pub async fn put_manifest<C: ConnectionTrait>(
    db: &C,
    repository_id: i64,
    content: &str,
    parsed: &Manifest,
    push: &Push,
) -> AppResult<registry_manifests::Model> {
    let digest = Digest::of(content.as_bytes());
    let mut total = i64::try_from(content.len()).unwrap_or(i64::MAX);
    for blob in parsed.blobs() {
        if linked_blob(db, repository_id, &blob.digest)
            .await?
            .is_none()
        {
            return Err(AppError::validation(format!(
                "MANIFEST_BLOB_UNKNOWN: o blob {} não foi enviado a este repositório",
                blob.digest
            )));
        }
        total = total.saturating_add(blob.size);
    }
    let mut platform = None;
    for child in parsed.children() {
        let Some(found) = manifest_by_digest(db, repository_id, child.digest.as_str()).await?
        else {
            return Err(AppError::validation(format!(
                "MANIFEST_BLOB_UNKNOWN: o manifest {} não foi enviado a este repositório",
                child.digest
            )));
        };
        if child.attests().is_none() {
            total = total.saturating_add(found.total_size);
        }
    }
    if let Some(first) = parsed.images().first() {
        platform = first.platform.as_ref().map(manifest::Platform::label);
    }

    if let Some(existing) = manifest_by_digest(db, repository_id, digest.as_str()).await? {
        // O mesmo conteúdo de novo: idempotente. A origem só é gravada uma vez.
        if existing.provenance.is_none() && push.provenance.is_some() {
            let mut active: registry_manifests::ActiveModel = existing.into();
            active.provenance = Set(push.provenance.clone());
            return Ok(active.update(db).await?);
        }
        return Ok(existing);
    }
    let now = Utc::now().into();
    Ok(registry_manifests::ActiveModel {
        repository_id: Set(repository_id),
        digest: Set(digest.to_string()),
        media_type: Set(parsed.media_type.clone()),
        artifact_type: Set(parsed.artifact_type.clone().or_else(|| match &parsed.kind {
            manifest::Kind::Image { config, .. }
                if !config.media_type.contains("image.config")
                    && !config.media_type.contains("container.image") =>
            {
                Some(config.media_type.clone())
            }
            _ => None,
        })),
        subject_digest: Set(parsed.subject.as_ref().map(ToString::to_string)),
        content: Set(content.to_string()),
        size: Set(i64::try_from(content.len()).unwrap_or(i64::MAX)),
        total_size: Set(total),
        platform: Set(platform),
        provenance: Set(push.provenance.clone()),
        pushed_by: Set(push.pushed_by.clone()),
        pinned: Set(false),
        sbom_packages: Set(None),
        scan_status: Set(None),
        scan: Set(None),
        scanned_at: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(db)
    .await?)
}

/// Aponta a tag para o manifest. Tag de commit é imutável (RF-04): mudar o
/// digest dela é recusado; repetir o mesmo é idempotente.
///
/// # Errors
///
/// `Conflict` ao tentar mover uma tag imutável; `Validation` com tag fora da
/// regra; erro do banco.
pub async fn set_tag<C: ConnectionTrait>(
    db: &C,
    repository_id: i64,
    name: &str,
    digest: &Digest,
) -> AppResult<()> {
    if !names::valid_tag(name) {
        return Err(AppError::validation(format!("Tag inválida: {name}")));
    }
    let now = Utc::now().into();
    match tag(db, repository_id, name).await? {
        Some(existing) if existing.manifest_digest == digest.as_str() => Ok(()),
        Some(_) if names::is_commit_tag(name) => Err(AppError::conflict(format!(
            "A tag {name} é de um commit e não muda de conteúdo (imutável)"
        ))),
        Some(existing) => {
            let mut active: registry_tags::ActiveModel = existing.into();
            active.manifest_digest = Set(digest.to_string());
            active.updated_at = Set(now);
            active.update(db).await?;
            Ok(())
        }
        None => {
            registry_tags::ActiveModel {
                repository_id: Set(repository_id),
                name: Set(name.to_string()),
                manifest_digest: Set(digest.to_string()),
                created_at: Set(now),
                updated_at: Set(now),
                ..Default::default()
            }
            .insert(db)
            .await?;
            Ok(())
        }
    }
}

/// As tags do repositório, em ordem, com a paginação da especificação
/// (`n` e `last`).
///
/// # Errors
///
/// Erro do banco.
pub async fn tags<C: ConnectionTrait>(
    db: &C,
    repository_id: i64,
    limit: Option<u64>,
    last: Option<&str>,
) -> AppResult<Vec<String>> {
    let mut query = registry_tags::Entity::find()
        .filter(registry_tags::Column::RepositoryId.eq(repository_id))
        .order_by_asc(registry_tags::Column::Name);
    if let Some(last) = last {
        query = query.filter(registry_tags::Column::Name.gt(last));
    }
    if let Some(limit) = limit {
        query = query.limit(limit);
    }
    Ok(query
        .all(db)
        .await?
        .into_iter()
        .map(|tag| tag.name)
        .collect())
}

/// Os manifests que descrevem `subject` (attestations, assinaturas).
///
/// # Errors
///
/// Erro do banco.
pub async fn referrers<C: ConnectionTrait>(
    db: &C,
    repository_id: i64,
    subject: &Digest,
    artifact_type: Option<&str>,
) -> AppResult<Vec<registry_manifests::Model>> {
    let mut query = registry_manifests::Entity::find()
        .filter(registry_manifests::Column::RepositoryId.eq(repository_id))
        .filter(registry_manifests::Column::SubjectDigest.eq(subject.as_str()));
    if let Some(kind) = artifact_type {
        query = query.filter(registry_manifests::Column::ArtifactType.eq(kind));
    }
    Ok(query.all(db).await?)
}

/// Apaga um manifest e as tags que apontam para ele.
///
/// # Errors
///
/// Erro do banco.
pub async fn delete_manifest<C: ConnectionTrait>(
    db: &C,
    repository_id: i64,
    digest: &str,
) -> AppResult<bool> {
    registry_tags::Entity::delete_many()
        .filter(registry_tags::Column::RepositoryId.eq(repository_id))
        .filter(registry_tags::Column::ManifestDigest.eq(digest))
        .exec(db)
        .await?;
    let result = registry_manifests::Entity::delete_many()
        .filter(registry_manifests::Column::RepositoryId.eq(repository_id))
        .filter(registry_manifests::Column::Digest.eq(digest))
        .exec(db)
        .await?;
    Ok(result.rows_affected > 0)
}

/// Apaga uma tag (o manifest fica).
///
/// # Errors
///
/// Erro do banco.
pub async fn delete_tag<C: ConnectionTrait>(
    db: &C,
    repository_id: i64,
    name: &str,
) -> AppResult<bool> {
    let result = registry_tags::Entity::delete_many()
        .filter(registry_tags::Column::RepositoryId.eq(repository_id))
        .filter(registry_tags::Column::Name.eq(name))
        .exec(db)
        .await?;
    Ok(result.rows_affected > 0)
}

/// Quantas tags apontam para o manifest.
///
/// # Errors
///
/// Erro do banco.
pub async fn tags_of<C: ConnectionTrait>(
    db: &C,
    repository_id: i64,
    digest: &str,
) -> AppResult<Vec<String>> {
    Ok(registry_tags::Entity::find()
        .filter(registry_tags::Column::RepositoryId.eq(repository_id))
        .filter(registry_tags::Column::ManifestDigest.eq(digest))
        .order_by_asc(registry_tags::Column::Name)
        .all(db)
        .await?
        .into_iter()
        .map(|tag| tag.name)
        .collect())
}

/// Quantos repositórios existem (para a tela).
///
/// # Errors
///
/// Erro do banco.
pub async fn count_repositories<C: ConnectionTrait>(db: &C) -> AppResult<u64> {
    Ok(registry_repositories::Entity::find().count(db).await?)
}
