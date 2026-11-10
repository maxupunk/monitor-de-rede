//! Varredura de vulnerabilidades (ADR 022 §8, RF-22..27).
//!
//! A imagem é analisada uma vez ao chegar (em segundo plano) e de novo quando
//! a base do OSV muda. O deploy pede o veredito antes de aprovar
//! ([`ensure`]): se a análise ainda não rodou, roda ali. Os achados ficam no
//! manifest marcado com a tag (o índice, quando há attestations); as exceções
//! valem na hora do veredito, não na gravação — vencida a exceção, o achado
//! volta a contar sem nova análise.

pub mod advisory;
pub mod osv;
pub mod packages;
pub mod version;

use std::collections::{BTreeMap, BTreeSet};

use async_compression::tokio::bufread::GzipDecoder;
use chrono::Utc;
use loco_rs::app::AppContext;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, BufReader};

use netmonitor_core::{
    models::{registry_exceptions, registry_findings, registry_manifests},
    shared::{errors::AppResult, transaction::begin_write},
};
use netmonitor_servers::deploys::images::{ScanStatus, ScanVerdict};

use self::{
    advisory::Severity,
    packages::{Package, SPDX_HINT},
};
use crate::{
    catalog,
    digest::Digest,
    events,
    manifest::{self, Kind, SPDX_PREDICATE},
    storage::{self, BlobStore},
    tarstream,
};

/// Teto de um arquivo de banco de pacotes lido de uma camada.
const MAX_DB_FILE: u64 = 16 * 1024 * 1024;
/// Teto de um SBOM.
const MAX_SBOM: u64 = 64 * 1024 * 1024;

/// Analisa em segundo plano (imagem nova).
pub fn schedule(ctx: &AppContext, repository_id: i64, digest: String) {
    let ctx = ctx.clone();
    tokio::spawn(async move {
        match catalog::manifest_by_digest(&ctx.db, repository_id, &digest).await {
            Ok(Some(manifest)) => {
                if let Err(error) = run(&ctx, &manifest).await {
                    tracing::warn!(%digest, %error, "varredura da imagem falhou");
                }
            }
            Ok(None) => {}
            Err(error) => tracing::warn!(%digest, %error, "imagem sumiu antes da varredura"),
        }
    });
}

/// Garante a análise do manifest e devolve-o atualizado.
///
/// # Errors
///
/// Erro do banco.
pub async fn ensure(
    ctx: &AppContext,
    manifest: registry_manifests::Model,
) -> AppResult<registry_manifests::Model> {
    if manifest
        .scan_status
        .as_deref()
        .is_some_and(|status| ScanStatus::parse(status).is_final())
    {
        return Ok(manifest);
    }
    run(ctx, &manifest).await?;
    Ok(registry_manifests::Entity::find_by_id(manifest.id)
        .one(&ctx.db)
        .await?
        .unwrap_or(manifest))
}

async fn read_blob(store: &dyn BlobStore, digest: &Digest, limit: u64) -> Option<Vec<u8>> {
    let mut reader = store.open(digest, 0).await.ok()?;
    let mut data = Vec::new();
    (&mut reader)
        .take(limit + 1)
        .read_to_end(&mut data)
        .await
        .ok()?;
    (data.len() as u64 <= limit).then_some(data)
}

/// Os pacotes de uma imagem de plataforma: do SBOM (attestation no índice)
/// ou, sem ele, dos bancos de pacotes das camadas.
async fn packages_of(
    db: &DatabaseConnection,
    store: &dyn BlobStore,
    repository_id: i64,
    top: &registry_manifests::Model,
) -> AppResult<(Vec<Package>, bool)> {
    let parsed = manifest::parse(top.content.as_bytes(), Some(&top.media_type)).ok();
    let mut found = BTreeSet::new();
    let mut from_sbom = false;
    let mut images = Vec::new();
    match parsed.as_ref().map(|parsed| &parsed.kind) {
        Some(Kind::Index { .. }) => {
            let parsed = parsed.as_ref().expect("índice lido acima");
            for image in parsed.images() {
                if let Some(attestation) = parsed.attestation_of(&image.digest) {
                    if let Some(row) =
                        catalog::manifest_by_digest(db, repository_id, attestation.digest.as_str())
                            .await?
                    {
                        if let Ok(attest) = manifest::parse(row.content.as_bytes(), None) {
                            for layer in attest.blobs().into_iter().skip(1) {
                                if layer.predicate() != Some(SPDX_PREDICATE) {
                                    continue;
                                }
                                if let Some(data) = read_blob(store, &layer.digest, MAX_SBOM).await
                                {
                                    if let Ok(document) = serde_json::from_slice::<Value>(&data) {
                                        let listed = packages::from_spdx(&document);
                                        // Um SBOM que não rende pacote (formato que não
                                        // lemos) não pode calar a leitura das camadas.
                                        from_sbom |= !listed.is_empty();
                                        found.extend(listed);
                                    }
                                }
                            }
                        }
                    }
                }
                images.push(image.digest.clone());
            }
        }
        Some(Kind::Image { .. }) => images.push(
            top.digest
                .parse()
                .unwrap_or_else(|_| Digest::of(top.content.as_bytes())),
        ),
        None => {}
    }
    if !from_sbom {
        for image in images {
            let Some(row) = catalog::manifest_by_digest(db, repository_id, image.as_str()).await?
            else {
                continue;
            };
            found.extend(from_layers(store, &row).await);
        }
    }
    Ok((found.into_iter().collect(), from_sbom))
}

/// Lê `os-release` e os bancos `dpkg`/`apk` das camadas (a última camada
/// que traz o arquivo vence).
async fn from_layers(store: &dyn BlobStore, image: &registry_manifests::Model) -> Vec<Package> {
    let Ok(parsed) = manifest::parse(image.content.as_bytes(), Some(&image.media_type)) else {
        return Vec::new();
    };
    let Kind::Image { layers, .. } = &parsed.kind else {
        return Vec::new();
    };
    let mut files: BTreeMap<&'static str, String> = BTreeMap::new();
    for layer in layers {
        if !layer.media_type.contains("gzip") {
            continue;
        }
        let Ok(reader) = store.open(&layer.digest, 0).await else {
            continue;
        };
        let mut tar = tarstream::Reader::new(GzipDecoder::new(BufReader::new(reader)));
        while let Ok(Some(entry)) = tar.next().await {
            let key = match entry.path.as_str() {
                "etc/os-release" | "usr/lib/os-release" => "os-release",
                "var/lib/dpkg/status" => "dpkg",
                "lib/apk/db/installed" => "apk",
                _ => continue,
            };
            if !entry.regular {
                continue;
            }
            if let Ok(data) = tar.read_all(MAX_DB_FILE).await {
                files.insert(key, String::from_utf8_lossy(&data).into_owned());
            }
        }
    }
    let Some((id, version)) = files
        .get("os-release")
        .and_then(|text| packages::os_release(text))
    else {
        return Vec::new();
    };
    let Some(ecosystem) = packages::system_ecosystem(&id, &version) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Some(status) = files.get("dpkg") {
        out.extend(packages::from_dpkg(status, &ecosystem));
    }
    if let Some(installed) = files.get("apk") {
        out.extend(packages::from_apk(installed, &ecosystem));
    }
    out
}

/// Analisa agora: pacotes, base do OSV de cada ecossistema e achados.
///
/// # Errors
///
/// Erro do banco.
pub async fn run(ctx: &AppContext, top: &registry_manifests::Model) -> AppResult<()> {
    let db = &ctx.db;
    let store = storage::store();
    let (found, from_sbom) = packages_of(db, store.as_ref(), top.repository_id, top).await?;

    // A base de cada ecossistema visto; a primeira vez baixa agora.
    let feeds: BTreeSet<&str> = found
        .iter()
        .map(Package::feed)
        .filter(|feed| osv::SUPPORTED.contains(feed))
        .collect();
    let mut failed_feed = false;
    for feed in feeds {
        let row = osv::ensure_feed(db, feed).await?;
        if row.synced_at.is_none() {
            osv::sync(db, &row).await?;
        }
        failed_feed |= osv::ensure_feed(db, feed).await?.synced_at.is_none();
    }

    let mut findings = Vec::new();
    for package in &found {
        for advisory in osv::advisories(db, &package.ecosystem, &package.name).await? {
            let Some(hit) = advisory::affects(&advisory.data, package) else {
                continue;
            };
            findings.push(registry_findings::ActiveModel {
                manifest_id: Set(top.id),
                advisory: Set(advisory.advisory.clone()),
                aliases: Set(advisory.data.get("aliases").cloned().unwrap_or(json!([]))),
                package: Set(package.name.clone()),
                ecosystem: Set(package.ecosystem.clone()),
                installed: Set(package.version.chars().take(128).collect()),
                fixed: Set(hit.fixed.map(|fixed| fixed.chars().take(128).collect())),
                severity: Set(advisory::severity(&advisory.data, &package.ecosystem)
                    .as_str()
                    .to_string()),
                summary: Set(advisory
                    .data
                    .get("summary")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)),
                created_at: Set(Utc::now().into()),
                ..Default::default()
            });
        }
    }
    let counts = count(findings.iter().map(|finding| match &finding.severity {
        sea_orm::ActiveValue::Set(level) => Severity::parse(level),
        _ => Severity::Unknown,
    }));

    // Apagar e regravar juntos: uma falha no meio não deixa a versão sem
    // achados e com a contagem antiga.
    let txn = begin_write(db).await?;
    registry_findings::Entity::delete_many()
        .filter(registry_findings::Column::ManifestId.eq(top.id))
        .exec(&txn)
        .await?;
    for chunk in findings.chunks(200) {
        registry_findings::Entity::insert_many(chunk.to_vec())
            .exec(&txn)
            .await?;
    }
    txn.commit().await?;
    let status = if failed_feed {
        ScanStatus::Failed
    } else if found.is_empty() {
        ScanStatus::NoSbom
    } else {
        ScanStatus::Done
    };
    let mut active: registry_manifests::ActiveModel = top.clone().into();
    active.sbom_packages = Set(Some(i32::try_from(found.len()).unwrap_or(i32::MAX)));
    active.scan_status = Set(Some(status.as_str().to_string()));
    active.scan = Set(Some(json!({
        "critical": counts[4], "high": counts[3], "medium": counts[2],
        "low": counts[1], "unknown": counts[0], "source": if from_sbom { SPDX_HINT } else { "layers" },
    })));
    active.scanned_at = Set(Some(Utc::now().into()));
    active.update(db).await?;
    events::publish_changed(ctx);
    Ok(())
}

fn count(levels: impl Iterator<Item = Severity>) -> [u32; 5] {
    let mut counts = [0_u32; 5];
    for level in levels {
        counts[level as usize] += 1;
    }
    counts
}

/// As exceções que valem agora para uma aplicação (as dela e as gerais).
pub struct Exceptions(BTreeSet<String>);

impl Exceptions {
    /// # Errors
    ///
    /// Erro do banco.
    pub async fn active(db: &DatabaseConnection, application_id: Option<i64>) -> AppResult<Self> {
        let now = Utc::now();
        Ok(Self(
            registry_exceptions::Entity::find()
                .all(db)
                .await?
                .into_iter()
                .filter(|exception| {
                    exception.expires_at.with_timezone(&Utc) > now
                        && exception
                            .application_id
                            .is_none_or(|id| Some(id) == application_id)
                })
                .map(|exception| exception.advisory)
                .collect(),
        ))
    }

    /// O achado está coberto: pelo identificador ou por um alias (a exceção
    /// pode ter sido aberta pelo CVE e o aviso ser um DSA).
    #[must_use]
    pub fn covers(&self, finding: &registry_findings::Model) -> bool {
        self.0.contains(&finding.advisory)
            || finding.aliases.as_array().is_some_and(|aliases| {
                aliases
                    .iter()
                    .filter_map(Value::as_str)
                    .any(|alias| self.0.contains(alias))
            })
    }
}

/// O veredito com as exceções válidas descontadas.
///
/// # Errors
///
/// Erro do banco.
pub async fn verdict(
    db: &DatabaseConnection,
    manifest: &registry_manifests::Model,
    application_id: Option<i64>,
) -> AppResult<ScanVerdict> {
    let findings = registry_findings::Entity::find()
        .filter(registry_findings::Column::ManifestId.eq(manifest.id))
        .all(db)
        .await?;
    let exceptions = Exceptions::active(db, application_id).await?;
    let excepted = |finding: &&registry_findings::Model| exceptions.covers(finding);
    let (skipped, kept): (Vec<_>, Vec<_>) = findings.iter().partition(excepted);
    let counts = count(
        kept.iter()
            .map(|finding| Severity::parse(&finding.severity)),
    );
    let mut worst = kept.clone();
    worst.sort_by_key(|finding| std::cmp::Reverse(Severity::parse(&finding.severity)));
    Ok(ScanVerdict {
        status: manifest
            .scan_status
            .as_deref()
            .map_or(ScanStatus::Pending, ScanStatus::parse),
        critical: counts[4],
        high: counts[3],
        medium: counts[2],
        low: counts[1],
        unknown: counts[0],
        excepted: u32::try_from(skipped.len()).unwrap_or(u32::MAX),
        top: worst
            .iter()
            .take(5)
            .map(|finding| format!("{} em {}", finding.advisory, finding.package))
            .collect(),
    })
}

/// Uma atualização da base de cada vez (a rotina diária e o "Atualizar
/// agora" não correm juntos).
///
/// # Errors
///
/// `Conflict` quando já há uma em curso.
pub fn refresh_guard() -> AppResult<netmonitor_core::shared::run_guard::RunGuard> {
    netmonitor_core::shared::run_guard::RunGuard::acquire(
        "registry_advisories",
        0,
        "A base de vulnerabilidades já está sendo atualizada",
    )
}

/// Atualiza as bases e reanalisa as imagens já analisadas (rotina diária).
///
/// # Errors
///
/// Erro do banco.
pub async fn refresh(ctx: &AppContext) -> AppResult<u32> {
    let feeds = netmonitor_core::models::registry_feeds::Entity::find()
        .all(&ctx.db)
        .await?;
    let mut changed = false;
    for feed in &feeds {
        changed |= osv::sync(&ctx.db, feed).await?;
    }
    if !changed {
        return Ok(0);
    }
    let analysed = registry_manifests::Entity::find()
        .filter(registry_manifests::Column::ScanStatus.is_in(["done", "failed"]))
        .all(&ctx.db)
        .await?;
    let mut count = 0;
    for manifest in analysed {
        run(ctx, &manifest).await?;
        count += 1;
    }
    Ok(count)
}
