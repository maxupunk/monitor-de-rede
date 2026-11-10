//! A cópia local do OSV.dev (RF-24).
//!
//! Só os ecossistemas que aparecem nas imagens são baixados (`registry_feeds`
//! nasce quando a varredura vê o primeiro pacote de um). A primeira carga é o
//! `all.zip`; as seguintes leem o `modified_id.csv` (mais novo primeiro) até o
//! cursor e buscam só os avisos que mudaram. A lista de pacotes das imagens
//! nunca sai da central: o que se baixa é a base inteira do ecossistema.
//!
//! Cada aviso vira uma linha por pacote afetado, com só o que a comparação
//! precisa (faixas, severidade, resumo, aliases).

use std::io::Read;

use chrono::{DateTime, Utc};
use sea_orm::{
    sea_query::OnConflict, ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection,
    EntityTrait, QueryFilter, Set,
};
use serde_json::{json, Value};
use tokio::{io::AsyncWriteExt, sync::mpsc};

use netmonitor_core::{
    models::{registry_advisories, registry_feeds},
    shared::errors::{AppError, AppResult},
};

use crate::storage::{self, Staged};

/// Base dos arquivos (`REGISTRY_OSV_URL` troca por um espelho interno).
pub const URL_ENV: &str = "REGISTRY_OSV_URL";
const DEFAULT_URL: &str = "https://storage.googleapis.com/osv-vulnerabilities";
/// Acima disto, a atualização incremental vira carga completa.
const MAX_INCREMENTAL: usize = 3000;
const BATCH: usize = 400;
/// Ecossistemas que a varredura entende (diretórios do OSV).
pub const SUPPORTED: &[&str] = &[
    "Debian",
    "Ubuntu",
    "Alpine",
    "npm",
    "PyPI",
    "crates.io",
    "Go",
    "Maven",
    "RubyGems",
    "NuGet",
    "Packagist",
];

fn base_url() -> String {
    std::env::var(URL_ENV)
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_URL.to_string())
}

fn client() -> AppResult<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent("NetMonitor-registry")
        .connect_timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|error| AppError::Internal(error.into()))
}

/// `Ubuntu:22.04:LTS` → `Ubuntu:22.04` (como a varredura nomeia os pacotes).
#[must_use]
pub fn normalize_ecosystem(raw: &str) -> Option<String> {
    let mut parts = raw.split(':');
    let base = parts.next()?;
    if !SUPPORTED.contains(&base) {
        return None;
    }
    match base {
        "Ubuntu" => {
            let release = parts.next()?;
            if release == "Pro" {
                return None;
            }
            Some(format!("Ubuntu:{release}"))
        }
        "Debian" | "Alpine" => Some(format!("{base}:{}", parts.next()?)),
        _ => Some(base.to_string()),
    }
}

/// Uma linha por pacote afetado do aviso.
#[must_use]
pub fn rows(record: &Value, feed: &str) -> Vec<(String, String, String, DateTime<Utc>, Value)> {
    let Some(id) = record.get("id").and_then(Value::as_str) else {
        return Vec::new();
    };
    if record.get("withdrawn").is_some() {
        return Vec::new();
    }
    let modified = record
        .get("modified")
        .and_then(Value::as_str)
        .and_then(|raw| DateTime::parse_from_rfc3339(raw).ok())
        .map_or_else(Utc::now, |time| time.with_timezone(&Utc));
    let mut out = Vec::new();
    for affected in record
        .get("affected")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(ecosystem) = affected
            .pointer("/package/ecosystem")
            .and_then(Value::as_str)
            .and_then(normalize_ecosystem)
            .filter(|ecosystem| ecosystem.split(':').next() == Some(feed))
        else {
            continue;
        };
        let Some(name) = affected.pointer("/package/name").and_then(Value::as_str) else {
            continue;
        };
        let compact = json!({
            "id": id,
            "aliases": record.get("aliases").cloned().unwrap_or(Value::Array(Vec::new())),
            "summary": record.get("summary").or_else(|| record.get("details")).and_then(Value::as_str)
                .map(|text| text.chars().take(300).collect::<String>()),
            "severity": record.get("severity").cloned(),
            "database_specific": record.get("database_specific").and_then(|d| d.get("severity"))
                .map(|severity| json!({"severity": severity})),
            "affected": [affected],
        });
        out.push((
            ecosystem,
            name.to_string(),
            id.to_string(),
            modified,
            compact,
        ));
    }
    out
}

type Row = (String, String, String, DateTime<Utc>, Value);

async fn insert(db: &DatabaseConnection, rows: Vec<Row>) -> AppResult<()> {
    if rows.is_empty() {
        return Ok(());
    }
    let now = Utc::now().into();
    let models = rows
        .into_iter()
        .map(
            |(ecosystem, package, advisory, modified, data)| registry_advisories::ActiveModel {
                ecosystem: Set(ecosystem),
                package: Set(package),
                advisory: Set(advisory),
                modified: Set(modified.into()),
                data: Set(data),
                created_at: Set(now),
                ..Default::default()
            },
        );
    registry_advisories::Entity::insert_many(models)
        .on_conflict(
            OnConflict::columns([
                registry_advisories::Column::Ecosystem,
                registry_advisories::Column::Package,
                registry_advisories::Column::Advisory,
            ])
            .update_columns([
                registry_advisories::Column::Modified,
                registry_advisories::Column::Data,
            ])
            .to_owned(),
        )
        .exec_without_returning(db)
        .await?;
    Ok(())
}

fn of_feed(feed: &str) -> Condition {
    Condition::any()
        .add(registry_advisories::Column::Ecosystem.eq(feed))
        .add(registry_advisories::Column::Ecosystem.starts_with(format!("{feed}:")))
}

/// A linha do ecossistema (criada se nova).
///
/// # Errors
///
/// Erro do banco.
pub async fn ensure_feed(db: &DatabaseConnection, feed: &str) -> AppResult<registry_feeds::Model> {
    if let Some(found) = registry_feeds::Entity::find()
        .filter(registry_feeds::Column::Ecosystem.eq(feed))
        .one(db)
        .await?
    {
        return Ok(found);
    }
    let now = Utc::now().into();
    registry_feeds::Entity::insert(registry_feeds::ActiveModel {
        ecosystem: Set(feed.to_string()),
        synced_at: Set(None),
        cursor: Set(None),
        entries: Set(0),
        error: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    })
    .on_conflict(
        OnConflict::column(registry_feeds::Column::Ecosystem)
            .do_nothing()
            .to_owned(),
    )
    .exec_without_returning(db)
    .await?;
    registry_feeds::Entity::find()
        .filter(registry_feeds::Column::Ecosystem.eq(feed))
        .one(db)
        .await?
        .ok_or_else(|| AppError::not_found("feed"))
}

/// Carga completa do `all.zip`.
async fn full(db: &DatabaseConnection, feed: &str) -> AppResult<(i64, Option<DateTime<Utc>>)> {
    let url = format!("{}/{feed}/all.zip", base_url());
    let mut response = client()?
        .get(&url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|error| {
            AppError::service_unavailable(format!("OSV indisponível ({url}): {error}"))
        })?;
    let store = storage::store();
    let staged = Staged::new(store.as_ref())
        .await
        .map_err(|error| AppError::Internal(error.into()))?;
    let mut file = tokio::fs::File::create(&staged.path)
        .await
        .map_err(|error| AppError::Internal(error.into()))?;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| AppError::service_unavailable(error.to_string()))?
    {
        file.write_all(&chunk)
            .await
            .map_err(|error| AppError::Internal(error.into()))?;
    }
    file.flush()
        .await
        .map_err(|error| AppError::Internal(error.into()))?;
    drop(file);

    registry_advisories::Entity::delete_many()
        .filter(of_feed(feed))
        .exec(db)
        .await?;

    let (sender, mut batches) = mpsc::channel::<Vec<Row>>(4);
    let path = staged.path.clone();
    let feed_name = feed.to_string();
    let reader = tokio::task::spawn_blocking(move || -> Result<(), String> {
        let file = std::fs::File::open(&path).map_err(|error| error.to_string())?;
        let mut archive = zip::ZipArchive::new(file).map_err(|error| error.to_string())?;
        let mut batch = Vec::with_capacity(BATCH);
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;
            if !entry.name().ends_with(".json") || entry.size() > 8 * 1024 * 1024 {
                continue;
            }
            let mut text = String::new();
            if entry.read_to_string(&mut text).is_err() {
                continue;
            }
            let Ok(record) = serde_json::from_str::<Value>(&text) else {
                continue;
            };
            batch.extend(rows(&record, &feed_name));
            if batch.len() >= BATCH && sender.blocking_send(std::mem::take(&mut batch)).is_err() {
                return Ok(());
            }
        }
        if !batch.is_empty() {
            let _ = sender.blocking_send(batch);
        }
        Ok(())
    });
    let mut total = 0_i64;
    let mut newest: Option<DateTime<Utc>> = None;
    while let Some(batch) = batches.recv().await {
        total += i64::try_from(batch.len()).unwrap_or(0);
        newest = batch.iter().map(|row| row.3).chain(newest).max();
        insert(db, batch).await?;
    }
    reader
        .await
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error.to_string())))?
        .map_err(|error| AppError::validation(format!("all.zip do OSV ilegível: {error}")))?;
    Ok((total, newest))
}

/// Atualização pelo `modified_id.csv`. `None` quando mudou demais (vale a
/// carga completa).
async fn incremental(
    db: &DatabaseConnection,
    feed: &str,
    cursor: DateTime<Utc>,
) -> AppResult<Option<(i64, Option<DateTime<Utc>>)>> {
    let http = client()?;
    let url = format!("{}/{feed}/modified_id.csv", base_url());
    let text = http
        .get(&url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|error| AppError::service_unavailable(format!("OSV indisponível: {error}")))?
        .text()
        .await
        .map_err(|error| AppError::service_unavailable(error.to_string()))?;
    let mut changed = Vec::new();
    for line in text.lines() {
        let Some((when, id)) = line.split_once(',') else {
            continue;
        };
        let Ok(when) = DateTime::parse_from_rfc3339(when.trim()) else {
            continue;
        };
        let when = when.with_timezone(&Utc);
        if when <= cursor {
            break;
        }
        changed.push((when, id.trim().to_string()));
        if changed.len() > MAX_INCREMENTAL {
            return Ok(None);
        }
    }
    let mut total = 0_i64;
    let mut newest = None;
    for (when, id) in changed {
        // O id vem do arquivo do OSV: só caracteres de id, nada de caminho.
        if !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'))
        {
            continue;
        }
        let record_url = format!("{}/{feed}/{id}.json", base_url());
        let Ok(response) = http.get(&record_url).send().await else {
            continue;
        };
        let Ok(record) = response.json::<Value>().await else {
            continue;
        };
        registry_advisories::Entity::delete_many()
            .filter(registry_advisories::Column::Advisory.eq(&id))
            .filter(of_feed(feed))
            .exec(db)
            .await?;
        let lines = rows(&record, feed);
        total += i64::try_from(lines.len()).unwrap_or(0);
        insert(db, lines).await?;
        newest = newest.max(Some(when));
    }
    Ok(Some((total, newest)))
}

/// Atualiza um ecossistema. O erro fica gravado na linha (a tela mostra).
///
/// # Errors
///
/// Erro do banco ao gravar o resultado.
pub async fn sync(db: &DatabaseConnection, feed: &registry_feeds::Model) -> AppResult<bool> {
    let cursor = feed.cursor.map(|time| time.with_timezone(&Utc));
    let result = match cursor {
        Some(cursor) => match incremental(db, &feed.ecosystem, cursor).await {
            Ok(Some(done)) => Ok(done),
            Ok(None) => full(db, &feed.ecosystem).await,
            Err(error) => Err(error),
        },
        None => full(db, &feed.ecosystem).await,
    };
    let mut active: registry_feeds::ActiveModel = feed.clone().into();
    let changed = match result {
        Ok((count, newest)) => {
            active.synced_at = Set(Some(Utc::now().into()));
            if let Some(newest) = newest {
                active.cursor = Set(Some(newest.into()));
            }
            if cursor.is_none() {
                active.entries = Set(count);
            } else {
                active.entries = Set(feed.entries + count);
            }
            active.error = Set(None);
            count > 0
        }
        Err(error) => {
            tracing::warn!(feed = %feed.ecosystem, %error, "base OSV não atualizada");
            active.error = Set(Some(error.to_string()));
            false
        }
    };
    active.update(db).await?;
    Ok(changed)
}

/// Os avisos guardados de um pacote.
///
/// # Errors
///
/// Erro do banco.
pub async fn advisories(
    db: &DatabaseConnection,
    ecosystem: &str,
    package: &str,
) -> AppResult<Vec<registry_advisories::Model>> {
    Ok(registry_advisories::Entity::find()
        .filter(registry_advisories::Column::Ecosystem.eq(ecosystem))
        .filter(registry_advisories::Column::Package.eq(package))
        .all(db)
        .await?)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn linhas_por_pacote_do_ecossistema() {
        let record = json!({
            "id": "DSA-1", "modified": "2026-01-01T00:00:00Z", "aliases": ["CVE-2026-1"],
            "affected": [
                {"package": {"ecosystem": "Debian:12", "name": "openssl"}},
                {"package": {"ecosystem": "Debian:11", "name": "openssl"}},
                {"package": {"ecosystem": "npm", "name": "fora"}}
            ]
        });
        let lines = rows(&record, "Debian");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].0, "Debian:12");
        assert!(rows(
            &json!({"id": "X", "withdrawn": "2026-01-01T00:00:00Z"}),
            "npm"
        )
        .is_empty());
    }

    #[test]
    fn ecossistemas_normalizados() {
        assert_eq!(
            normalize_ecosystem("Ubuntu:22.04:LTS").as_deref(),
            Some("Ubuntu:22.04")
        );
        assert_eq!(normalize_ecosystem("Ubuntu:Pro:18.04:LTS"), None);
        assert_eq!(
            normalize_ecosystem("Alpine:v3.19").as_deref(),
            Some("Alpine:v3.19")
        );
        assert_eq!(normalize_ecosystem("npm").as_deref(), Some("npm"));
        assert_eq!(normalize_ecosystem("Hackage"), None);
    }
}
