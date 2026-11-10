//! Retenção e coleta (RF-15..17).
//!
//! Por repositório fica: o que está fixado, o que algum alvo usa ou de que o
//! rollback precisa (a versão no ar e as anteriores que o alvo guarda) e as
//! N versões mais novas. O resto perde as tags e os manifests; depois a
//! coleta apaga os blobs que nenhum manifest restante cita — com 24 h de
//! carência, para não pegar um envio em andamento.

use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    sync::Arc,
};

use async_trait::async_trait;
use chrono::{Duration, Utc};
use loco_rs::app::AppContext;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

use netmonitor_core::{
    areas::{self, Area},
    models::{
        registry_blobs, registry_manifests, registry_repositories, registry_repository_blobs,
        registry_tags, registry_uploads,
    },
    routines::{cadence::is_due, Cycle, Routine},
    shared::errors::AppResult,
};

use netmonitor_servers::deploys::usage::images_in_use;

use crate::{catalog, digest::Digest, events, manifest, scan, settings, storage};

const COLLECT_EVERY_SECS: i64 = 6 * 60 * 60;
const ADVISORIES_EVERY_SECS: i64 = 24 * 60 * 60;
const GRACE_HOURS: i64 = 24;

/// Os digests que um manifest cita (blobs e filhos).
fn referenced(content: &str) -> Vec<Digest> {
    let Ok(parsed) = manifest::parse(content.as_bytes(), None) else {
        return Vec::new();
    };
    parsed
        .blobs()
        .into_iter()
        .map(|blob| blob.digest.clone())
        .chain(parsed.children().iter().map(|child| child.digest.clone()))
        .collect()
}

/// Uma passada de retenção e coleta. Devolve os bytes liberados.
///
/// # Errors
///
/// Erro do banco.
pub async fn collect(ctx: &AppContext) -> AppResult<u64> {
    let db = &ctx.db;
    let keep_versions = settings::load(db).await?.keep_versions as usize;
    let usage = images_in_use(db).await?;
    let grace = Utc::now() - Duration::hours(GRACE_HOURS);

    for repo in registry_repositories::Entity::find().all(db).await? {
        let tags = registry_tags::Entity::find()
            .filter(registry_tags::Column::RepositoryId.eq(repo.id))
            .all(db)
            .await?;
        let manifests = registry_manifests::Entity::find()
            .filter(registry_manifests::Column::RepositoryId.eq(repo.id))
            .order_by_desc(registry_manifests::Column::CreatedAt)
            .all(db)
            .await?;
        let tagged: BTreeMap<&str, Vec<&str>> =
            tags.iter().fold(BTreeMap::new(), |mut map, tag| {
                map.entry(tag.manifest_digest.as_str())
                    .or_default()
                    .push(tag.name.as_str());
                map
            });
        // As versões (manifests com tag), da mais nova para a mais velha.
        let mut kept: BTreeSet<String> = BTreeSet::new();
        let mut versions = 0;
        for row in &manifests {
            let Some(row_tags) = tagged.get(row.digest.as_str()) else {
                continue;
            };
            versions += 1;
            let in_use = row_tags
                .iter()
                .any(|tag| usage.is_protected(&repo.name, tag));
            if row.pinned || in_use || versions <= keep_versions {
                kept.insert(row.digest.clone());
            }
        }
        // O que as versões guardadas citam continua (filhos, attestations).
        let mut alive: HashSet<String> = HashSet::new();
        let mut queue: Vec<String> = kept.iter().cloned().collect();
        let by_digest: BTreeMap<&str, &registry_manifests::Model> = manifests
            .iter()
            .map(|row| (row.digest.as_str(), row))
            .collect();
        while let Some(digest) = queue.pop() {
            if !alive.insert(digest.clone()) {
                continue;
            }
            if let Some(row) = by_digest.get(digest.as_str()) {
                for child in referenced(&row.content) {
                    queue.push(child.to_string());
                }
            }
        }
        for row in &manifests {
            let referenced_by_subject = row
                .subject_digest
                .as_ref()
                .is_some_and(|subject| alive.contains(subject));
            if alive.contains(&row.digest) || referenced_by_subject {
                if referenced_by_subject {
                    alive.insert(row.digest.clone());
                    for child in referenced(&row.content) {
                        alive.insert(child.to_string());
                    }
                }
                continue;
            }
            // Recém-chegado sem tag (um push em andamento): espera a carência.
            if !tagged.contains_key(row.digest.as_str())
                && row.created_at.with_timezone(&Utc) > grace
            {
                alive.insert(row.digest.clone());
                continue;
            }
            catalog::delete_manifest(db, repo.id, &row.digest).await?;
        }
        // Vínculos de blob que nenhum manifest vivo cita.
        let links = registry_repository_blobs::Entity::find()
            .filter(registry_repository_blobs::Column::RepositoryId.eq(repo.id))
            .all(db)
            .await?;
        for link in links {
            if alive.contains(&link.digest) || link.created_at.with_timezone(&Utc) > grace {
                continue;
            }
            if let Ok(digest) = link.digest.parse::<Digest>() {
                catalog::unlink_blob(db, repo.id, &digest).await?;
            }
        }
    }

    // Envios abandonados.
    let stale = registry_uploads::Entity::find()
        .filter(registry_uploads::Column::UpdatedAt.lt(grace))
        .all(db)
        .await?;
    let store = storage::store();
    for upload in stale {
        if let Ok(id) = uuid::Uuid::parse_str(&upload.id) {
            if let Ok(staged) = storage::Staged::upload(store.as_ref(), &id).await {
                let _ = tokio::fs::remove_file(&staged.path).await;
            }
        }
        registry_uploads::Entity::delete_by_id(upload.id)
            .exec(db)
            .await?;
    }

    // Blobs sem nenhum vínculo: o arquivo sai do storage.
    let linked: HashSet<String> = registry_repository_blobs::Entity::find()
        .all(db)
        .await?
        .into_iter()
        .map(|link| link.digest)
        .collect();
    let mut freed: u64 = 0;
    for blob in registry_blobs::Entity::find().all(db).await? {
        if linked.contains(&blob.digest) || blob.created_at.with_timezone(&Utc) > grace {
            continue;
        }
        if let Ok(digest) = blob.digest.parse::<Digest>() {
            if store.delete(&digest).await.is_ok() {
                registry_blobs::Entity::delete_by_id(blob.id)
                    .exec(db)
                    .await?;
                freed += u64::try_from(blob.size).unwrap_or(0);
            }
        }
    }
    if freed > 0 {
        events::publish_changed(ctx);
    }
    Ok(freed)
}

/// A coleta no ciclo do scheduler. Com a área desligada, pausa (ADR 017).
pub struct RegistryCollect;

#[async_trait]
impl Routine for RegistryCollect {
    fn name(&self) -> &'static str {
        "coleta_registry"
    }

    fn area(&self) -> Area {
        Area::Servers
    }

    async fn run(&self, ctx: &AppContext, cycle: Cycle) -> AppResult<()> {
        if !is_due("registry_collect", COLLECT_EVERY_SECS, cycle.now) {
            return Ok(());
        }
        let ctx = ctx.clone();
        tokio::spawn(async move {
            if !areas::is_enabled(&ctx.db, Area::Servers)
                .await
                .unwrap_or(true)
            {
                return;
            }
            match collect(&ctx).await {
                Ok(freed) if freed > 0 => tracing::info!(freed, "registry: espaço liberado"),
                Ok(_) => {}
                Err(error) => tracing::warn!(%error, "coleta do registry falhou"),
            }
        });
        Ok(())
    }
}

/// A base de vulnerabilidades, uma vez por dia, e a reanálise.
pub struct AdvisoryRefresh;

#[async_trait]
impl Routine for AdvisoryRefresh {
    fn name(&self) -> &'static str {
        "base_cve_registry"
    }

    fn area(&self) -> Area {
        Area::Servers
    }

    async fn run(&self, ctx: &AppContext, cycle: Cycle) -> AppResult<()> {
        if !is_due("registry_advisories", ADVISORIES_EVERY_SECS, cycle.now) {
            return Ok(());
        }
        let ctx = ctx.clone();
        tokio::spawn(async move {
            if !areas::is_enabled(&ctx.db, Area::Servers)
                .await
                .unwrap_or(true)
            {
                return;
            }
            let Ok(_guard) = scan::refresh_guard() else {
                return;
            };
            match scan::refresh(&ctx).await {
                Ok(count) if count > 0 => tracing::info!(count, "registry: imagens reanalisadas"),
                Ok(_) => {}
                Err(error) => tracing::warn!(%error, "atualização da base de CVE falhou"),
            }
        });
        Ok(())
    }
}

/// As rotinas deste submódulo.
#[must_use]
pub fn routines() -> Vec<Arc<dyn Routine>> {
    vec![Arc::new(RegistryCollect), Arc::new(AdvisoryRefresh)]
}
