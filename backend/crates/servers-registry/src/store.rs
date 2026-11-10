//! O registry como o deploy o vê ([`ImageStore`], declarado na base da área).

use async_trait::async_trait;
use loco_rs::app::AppContext;
use serde_json::Value;
use tokio::io::AsyncWriteExt;

use agent_protocol::tunnel::TunnelStream;
use netmonitor_core::shared::errors::{AppError, AppResult};
use netmonitor_servers::deploys::images::{ImageOrigin, ImageStore, ScanStatus, ScanVerdict};

use crate::{archive, catalog, events, ingest, scan, settings, storage};

pub struct Registry;

#[async_trait]
impl ImageStore for Registry {
    async fn ingest(
        &self,
        ctx: &AppContext,
        repository: &str,
        application_id: i64,
        tags: &[String],
        archive: TunnelStream,
        origin: &ImageOrigin,
    ) -> AppResult<String> {
        let repo = catalog::ensure_repository(&ctx.db, repository, Some(application_id)).await?;
        let room = settings::room(&ctx.db).await?;
        let push = catalog::Push {
            pushed_by: origin.triggered_by.clone(),
            provenance: Some(origin.to_value()),
        };
        let store = storage::store();
        let digest =
            ingest::ingest(&ctx.db, store.as_ref(), repo.id, archive, tags, &push, room).await?;
        events::publish_changed(ctx);
        scan::schedule(ctx, repo.id, digest.to_string());
        Ok(digest.to_string())
    }

    async fn find(
        &self,
        ctx: &AppContext,
        repository: &str,
        tag: &str,
    ) -> AppResult<Option<(String, i64)>> {
        let Some(repo) = catalog::repository(&ctx.db, repository).await? else {
            return Ok(None);
        };
        Ok(catalog::resolve(&ctx.db, repo.id, tag)
            .await?
            .map(|manifest| (manifest.digest, manifest.total_size)))
    }

    async fn deliver(
        &self,
        ctx: &AppContext,
        repository: &str,
        tag: &str,
        architecture: &str,
        reference: &str,
        sink: &mut TunnelStream,
    ) -> AppResult<()> {
        let repo = catalog::repository(&ctx.db, repository)
            .await?
            .ok_or_else(|| AppError::not_found(format!("O repositório {repository} não existe")))?;
        let store = storage::store();
        let sink = archive::write(
            &ctx.db,
            store.as_ref(),
            repo.id,
            tag,
            architecture,
            reference,
            sink,
        )
        .await?;
        sink.shutdown()
            .await
            .map_err(|error| AppError::service_unavailable(error.to_string()))
    }

    async fn verdict(
        &self,
        ctx: &AppContext,
        repository: &str,
        tag: &str,
        application_id: i64,
    ) -> AppResult<ScanVerdict> {
        let Some(repo) = catalog::repository(&ctx.db, repository).await? else {
            return Ok(ScanVerdict {
                status: ScanStatus::Missing,
                ..ScanVerdict::default()
            });
        };
        let Some(manifest) = catalog::resolve(&ctx.db, repo.id, tag).await? else {
            return Ok(ScanVerdict {
                status: ScanStatus::Missing,
                ..ScanVerdict::default()
            });
        };
        let manifest = scan::ensure(ctx, manifest).await?;
        scan::verdict(&ctx.db, &manifest, Some(application_id)).await
    }

    async fn provenance(
        &self,
        ctx: &AppContext,
        repository: &str,
        tag: &str,
    ) -> AppResult<Option<Value>> {
        let Some(repo) = catalog::repository(&ctx.db, repository).await? else {
            return Ok(None);
        };
        Ok(catalog::resolve(&ctx.db, repo.id, tag)
            .await?
            .and_then(|manifest| manifest.provenance))
    }
}
