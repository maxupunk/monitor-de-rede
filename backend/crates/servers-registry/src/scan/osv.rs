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
//!
//! `created_at` de `registry_advisories` é o instante da última gravação da
//! linha: a carga completa grava em lotes curtos com o mesmo instante (a
//! "geração") e, só no fim, apaga o que ficou de gerações anteriores — sem
//! segurar o lock de escrita do SQLite pela carga inteira.

use std::{
    collections::{BTreeMap, HashMap},
    io::Read,
    sync::{Arc, Mutex as StdMutex, OnceLock},
};

use chrono::{DateTime, Utc};
use futures::future::join_all;
use sea_orm::{
    sea_query::OnConflict, ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait,
    DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, QuerySelect, Set,
};
use serde_json::{json, Value};
use tokio::{
    io::AsyncWriteExt,
    sync::{mpsc, oneshot},
};

use netmonitor_core::{
    models::{registry_advisories, registry_feeds},
    shared::{
        errors::{AppError, AppResult},
        transaction::begin_write,
    },
};

use crate::storage::{self, Staged};

/// Base dos arquivos (`REGISTRY_OSV_URL` troca por um espelho interno).
pub const URL_ENV: &str = "REGISTRY_OSV_URL";
const DEFAULT_URL: &str = "https://storage.googleapis.com/osv-vulnerabilities";
/// Acima disto, a atualização incremental vira carga completa.
const MAX_INCREMENTAL: usize = 3000;
const INCREMENTAL_CONCURRENCY: usize = 16;
const BATCH: usize = 400;
/// Depois de uma falha, a varredura (deploy, push) espera isto antes de baixar
/// a base de novo: sem internet, cada deploy refaria o download inteiro.
const RETRY_AFTER_SECS: i64 = 15 * 60;
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

fn client() -> AppResult<&'static reqwest::Client> {
    static CLIENT: OnceLock<Result<reqwest::Client, String>> = OnceLock::new();
    let cached = CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent("NetMonitor-registry")
            .connect_timeout(std::time::Duration::from_secs(20))
            .read_timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(|error| error.to_string())
    });
    cached
        .as_ref()
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error.clone())))
}

/// Serializa a sincronização de um mesmo ecossistema entre varreduras
/// concorrentes e a atualização diária.
fn feed_lock(feed: &str) -> Arc<tokio::sync::Mutex<()>> {
    static LOCKS: OnceLock<StdMutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>> =
        OnceLock::new();
    let mut map = LOCKS
        .get_or_init(|| StdMutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    map.entry(feed.to_string())
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone()
}

/// A última tentativa deste feed falhou há pouco: a varredura não tenta de
/// novo antes de [`RETRY_AFTER_SECS`] (a rotina diária e o "Atualizar agora"
/// tentam sempre).
#[must_use]
pub fn recently_failed(feed: &registry_feeds::Model, now: DateTime<Utc>) -> bool {
    feed.error.is_some()
        && now.signed_duration_since(feed.updated_at.with_timezone(&Utc))
            < chrono::Duration::seconds(RETRY_AFTER_SECS)
}

/// O erro de rede com a saída para quem não alcança o OSV.dev.
fn unreachable(url: &str, error: &reqwest::Error) -> AppError {
    AppError::service_unavailable(format!(
        "OSV indisponível ({url}): {error}. Sem acesso à internet, aponte {URL_ENV} \
         para um espelho interno da base"
    ))
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

/// Uma linha por `(ecossistema, pacote)` afetado do aviso (entradas repetidas
/// para o mesmo pacote são mescladas em `affected` para evitar colisão em
/// `ON CONFLICT DO UPDATE` no PostgreSQL).
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
    let mut grouped: BTreeMap<(String, String), Vec<Value>> = BTreeMap::new();
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
        grouped
            .entry((ecosystem, name.to_string()))
            .or_default()
            .push(affected.clone());
    }
    let aliases = record
        .get("aliases")
        .cloned()
        .unwrap_or_else(|| Value::Array(Vec::new()));
    let summary = record
        .get("summary")
        .or_else(|| record.get("details"))
        .and_then(Value::as_str)
        .map(|text| text.chars().take(300).collect::<String>());
    let severity = record.get("severity").cloned();
    let database_specific = record
        .get("database_specific")
        .and_then(|d| d.get("severity"))
        .map(|severity| json!({"severity": severity}));
    grouped
        .into_iter()
        .map(|((ecosystem, name), affected)| {
            let compact = json!({
                "id": id,
                "aliases": aliases,
                "summary": summary,
                "severity": severity,
                "database_specific": database_specific,
                "affected": affected,
            });
            (ecosystem, name, id.to_string(), modified, compact)
        })
        .collect()
}

type Row = (String, String, String, DateTime<Utc>, Value);

/// Grava (ou regrava) as linhas com `written` em `created_at` (a geração).
async fn insert<C: ConnectionTrait>(
    db: &C,
    rows: Vec<Row>,
    written: DateTime<Utc>,
) -> AppResult<()> {
    if rows.is_empty() {
        return Ok(());
    }
    // Garante chaves únicas no mesmo lote (PostgreSQL recusa duplicata em
    // `ON CONFLICT DO UPDATE`).
    let mut deduped: BTreeMap<(String, String, String), (DateTime<Utc>, Value)> = BTreeMap::new();
    for (ecosystem, package, advisory, modified, data) in rows {
        deduped.insert((ecosystem, package, advisory), (modified, data));
    }
    let now: sea_orm::prelude::DateTimeWithTimeZone = written.into();
    let models = deduped
        .into_iter()
        .map(|((ecosystem, package, advisory), (modified, data))| {
            registry_advisories::ActiveModel {
                ecosystem: Set(ecosystem),
                package: Set(package),
                advisory: Set(advisory),
                modified: Set(modified.into()),
                data: Set(data),
                created_at: Set(now),
                ..Default::default()
            }
        });
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
                registry_advisories::Column::CreatedAt,
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

/// Linhas apagadas por comando ao tirar a geração antiga (lock curto).
const DELETE_CHUNK: u64 = 2000;

/// Carga completa do `all.zip`, em lotes curtos marcados com a geração.
/// Só depois do arquivo inteiro lido sem erro a geração anterior sai; uma
/// falha no meio deixa a base antiga (mais o que já entrou) e o cursor parado.
async fn full(db: &DatabaseConnection, feed: &str) -> AppResult<(i64, Option<DateTime<Utc>>)> {
    let url = format!("{}/{feed}/all.zip", base_url());
    let mut response = client()?
        .get(&url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|error| unreachable(&url, &error))?;
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

    let (ready_tx, ready_rx) = oneshot::channel::<Result<(), String>>();
    let (sender, mut batches) = mpsc::channel::<Vec<Row>>(4);
    let path = staged.path.clone();
    let feed_name = feed.to_string();
    let reader = tokio::task::spawn_blocking(move || -> Result<(), String> {
        let file = match std::fs::File::open(&path) {
            Ok(file) => file,
            Err(error) => {
                let message = error.to_string();
                let _ = ready_tx.send(Err(message.clone()));
                return Err(message);
            }
        };
        let mut archive = match zip::ZipArchive::new(file) {
            Ok(archive) => archive,
            Err(error) => {
                let message = error.to_string();
                let _ = ready_tx.send(Err(message.clone()));
                return Err(message);
            }
        };
        let _ = ready_tx.send(Ok(()));
        let mut batch = Vec::with_capacity(BATCH);
        let mut text = String::new();
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;
            if !entry.name().ends_with(".json") || entry.size() > 8 * 1024 * 1024 {
                continue;
            }
            text.clear();
            // Erro de leitura (CRC, arquivo truncado) é do arquivo, não do
            // aviso: aborta antes de apagar a geração anterior.
            if let Err(error) = entry.read_to_string(&mut text) {
                return Err(format!("{}: {error}", entry.name()));
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
    ready_rx
        .await
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error.to_string())))?
        .map_err(|error| AppError::validation(format!("all.zip do OSV ilegível: {error}")))?;

    let generation = generation_now();
    let mut total = 0_i64;
    let mut newest: Option<DateTime<Utc>> = None;
    while let Some(batch) = batches.recv().await {
        total += i64::try_from(batch.len()).unwrap_or(0);
        newest = batch.iter().map(|row| row.3).chain(newest).max();
        // Um comando por lote: o lock de escrita dura o lote, não a carga.
        insert(db, batch, generation).await?;
    }
    reader
        .await
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error.to_string())))?
        .map_err(|error| AppError::validation(format!("all.zip do OSV ilegível: {error}")))?;
    drop_older_generations(db, feed, generation).await?;
    Ok((total, newest))
}

/// A geração de uma carga, truncada em microssegundos (o que o PostgreSQL
/// guarda), para a comparação no fim valer igual nos dois bancos.
fn generation_now() -> DateTime<Utc> {
    use chrono::SubsecRound;
    Utc::now().trunc_subsecs(6)
}

/// Apaga, em comandos curtos, as linhas do ecossistema gravadas antes de
/// `generation` (avisos que sumiram do `all.zip`).
async fn drop_older_generations(
    db: &DatabaseConnection,
    feed: &str,
    generation: DateTime<Utc>,
) -> AppResult<()> {
    let generation: sea_orm::prelude::DateTimeWithTimeZone = generation.into();
    loop {
        let ids: Vec<i64> = registry_advisories::Entity::find()
            .select_only()
            .column(registry_advisories::Column::Id)
            .filter(of_feed(feed))
            .filter(registry_advisories::Column::CreatedAt.lt(generation))
            .limit(DELETE_CHUNK)
            .into_tuple()
            .all(db)
            .await?;
        if ids.is_empty() {
            return Ok(());
        }
        registry_advisories::Entity::delete_many()
            .filter(registry_advisories::Column::Id.is_in(ids))
            .exec(db)
            .await?;
    }
}

struct IncrementalOutcome {
    changed: i64,
    newest: Option<DateTime<Utc>>,
    partial_error: Option<String>,
}

/// Atualização pelo `modified_id.csv`: processa do mais antigo para o mais novo
/// em ondas concorrentes e só avança o cursor até antes do primeiro registro
/// que falhou (falha transitória nunca pula um CVE; um aviso que sumiu do OSV,
/// 404, é pulado e registrado no log). `None` quando mudou demais (vale a
/// carga completa).
async fn incremental(
    db: &DatabaseConnection,
    feed: &str,
    cursor: DateTime<Utc>,
) -> AppResult<Option<IncrementalOutcome>> {
    let http = client()?;
    let base = base_url();
    let url = format!("{base}/{feed}/modified_id.csv");
    let text = http
        .get(&url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|error| unreachable(&url, &error))?
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
        let id = id.trim();
        // O id vem do arquivo do OSV: só caracteres de id, nada de caminho.
        if !id.is_empty()
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'))
        {
            changed.push((when, id.to_string()));
        }
        if changed.len() > MAX_INCREMENTAL {
            return Ok(None);
        }
    }
    // `modified_id.csv` vem do mais novo para o mais antigo: invertemos para
    // avançar o cursor de forma contígua e segura.
    changed.reverse();

    let mut total = 0_i64;
    let mut newest_ok: Option<DateTime<Utc>> = None;
    let mut first_failure: Option<DateTime<Utc>> = None;
    let mut partial_error = None;
    for wave in changed.chunks(INCREMENTAL_CONCURRENCY) {
        let fetched = join_all(wave.iter().map(|(when, id)| {
            let record_url = format!("{base}/{feed}/{id}.json");
            let id = id.clone();
            let when = *when;
            async move {
                let outcome = async {
                    let response = http
                        .get(&record_url)
                        .send()
                        .await
                        .map_err(|error| format!("falha ao baixar {id}: {error}"))?;
                    // Aviso que saiu do OSV depois de listado: não volta mais,
                    // então não pode segurar o cursor.
                    if matches!(
                        response.status(),
                        reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::GONE
                    ) {
                        return Ok(None);
                    }
                    let response = response
                        .error_for_status()
                        .map_err(|error| format!("falha ao baixar {id}: {error}"))?;
                    response
                        .json::<Value>()
                        .await
                        .map(Some)
                        .map_err(|error| format!("JSON inválido em {id}: {error}"))
                }
                .await;
                (when, id, outcome)
            }
        }))
        .await;

        let mut ids_to_replace = Vec::new();
        let mut wave_rows = Vec::new();
        let mut wave_newest = None;
        for (when, id, outcome) in fetched {
            match outcome {
                Ok(record) => {
                    if record.is_none() {
                        tracing::warn!(feed, advisory = %id, "aviso listado no OSV não existe mais: pulado");
                    }
                    if let Some(record) = record {
                        wave_rows.extend(rows(&record, feed));
                    }
                    ids_to_replace.push(id);
                    wave_newest = wave_newest.max(Some(when));
                }
                Err(error) => {
                    first_failure = Some(first_failure.map_or(when, |first| first.min(when)));
                    partial_error.get_or_insert(error);
                }
            }
        }
        if !ids_to_replace.is_empty() {
            let txn = begin_write(db).await?;
            registry_advisories::Entity::delete_many()
                .filter(registry_advisories::Column::Advisory.is_in(ids_to_replace))
                .filter(of_feed(feed))
                .exec(&txn)
                .await?;
            total += i64::try_from(wave_rows.len()).unwrap_or(0);
            insert(&txn, wave_rows, generation_now()).await?;
            txn.commit().await?;
            newest_ok = newest_ok.max(wave_newest);
        }
        if first_failure.is_some() {
            break;
        }
    }
    let newest = next_cursor(cursor, newest_ok, first_failure);
    Ok(Some(IncrementalOutcome {
        changed: total,
        newest,
        partial_error,
    }))
}

/// Até onde o cursor avança: o mais novo gravado, mas nunca até o instante de
/// um aviso que falhou — no empate de horário (A gravado, B falhou no mesmo
/// `modified`), `when <= cursor` pularia B para sempre. `None` quando não
/// avança.
fn next_cursor(
    cursor: DateTime<Utc>,
    newest_ok: Option<DateTime<Utc>>,
    first_failure: Option<DateTime<Utc>>,
) -> Option<DateTime<Utc>> {
    let candidate = match (newest_ok, first_failure) {
        (_, Some(failed)) => {
            let before = failed - chrono::Duration::microseconds(1);
            newest_ok.map_or(before, |ok| ok.min(before))
        }
        (ok, None) => ok?,
    };
    (candidate > cursor).then_some(candidate)
}

/// Atualiza um ecossistema. O erro fica gravado na linha (a tela mostra).
///
/// # Errors
///
/// Erro do banco ao gravar o resultado.
pub async fn sync(db: &DatabaseConnection, feed: &registry_feeds::Model) -> AppResult<bool> {
    let lock = feed_lock(&feed.ecosystem);
    let _guard = lock.lock().await;
    let current = ensure_feed(db, &feed.ecosystem).await?;
    // Outra varredura concorrente acabou de fazer a primeira carga deste feed.
    if feed.synced_at.is_none() && current.synced_at.is_some() {
        return Ok(false);
    }
    let cursor = current.cursor.map(|time| time.with_timezone(&Utc));
    let outcome = match cursor {
        Some(cursor) => match incremental(db, &current.ecosystem, cursor).await {
            Ok(Some(done)) => Ok(done),
            Ok(None) => {
                full(db, &current.ecosystem)
                    .await
                    .map(|(changed, newest)| IncrementalOutcome {
                        changed,
                        newest,
                        partial_error: None,
                    })
            }
            Err(error) => Err(error),
        },
        None => full(db, &current.ecosystem)
            .await
            .map(|(changed, newest)| IncrementalOutcome {
                changed,
                newest,
                partial_error: None,
            }),
    };
    let mut active: registry_feeds::ActiveModel = current.clone().into();
    let changed = match outcome {
        Ok(done) => {
            if let Some(newest) = done.newest {
                active.cursor = Set(Some(newest.into()));
            }
            let exact = registry_advisories::Entity::find()
                .filter(of_feed(&current.ecosystem))
                .count(db)
                .await?;
            active.entries = Set(i64::try_from(exact).unwrap_or(i64::MAX));
            if let Some(error) = done.partial_error {
                tracing::warn!(feed = %current.ecosystem, %error, "base OSV parcialmente atualizada");
                active.error = Set(Some(error));
            } else {
                active.synced_at = Set(Some(Utc::now().into()));
                active.error = Set(None);
            }
            done.changed > 0
        }
        Err(error) => {
            tracing::warn!(feed = %current.ecosystem, %error, "base OSV não atualizada");
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
pub async fn advisories<C: ConnectionTrait>(
    db: &C,
    ecosystem: &str,
    package: &str,
) -> AppResult<Vec<registry_advisories::Model>> {
    Ok(registry_advisories::Entity::find()
        .filter(registry_advisories::Column::Ecosystem.eq(ecosystem))
        .filter(registry_advisories::Column::Package.eq(package))
        .all(db)
        .await?)
}

/// Os avisos guardados para um conjunto de pacotes do mesmo ecossistema em
/// lotes (evita N+1 por pacote na varredura).
///
/// # Errors
///
/// Erro do banco.
pub async fn advisories_for_ecosystem<C: ConnectionTrait>(
    db: &C,
    ecosystem: &str,
    packages: &[String],
) -> AppResult<Vec<registry_advisories::Model>> {
    if packages.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for chunk in packages.chunks(300) {
        let rows = registry_advisories::Entity::find()
            .filter(registry_advisories::Column::Ecosystem.eq(ecosystem))
            .filter(registry_advisories::Column::Package.is_in(chunk.iter().cloned()))
            .all(db)
            .await?;
        out.extend(rows);
    }
    Ok(out)
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
                {"package": {"ecosystem": "Debian:12", "name": "openssl"}, "versions": ["3.0.1"]},
                {"package": {"ecosystem": "Debian:12", "name": "openssl"}, "versions": ["3.0.2"]},
                {"package": {"ecosystem": "Debian:11", "name": "openssl"}},
                {"package": {"ecosystem": "npm", "name": "fora"}}
            ]
        });
        let lines = rows(&record, "Debian");
        assert_eq!(lines.len(), 2, "entradas repetidas do mesmo pacote mesclam");
        let deb12 = lines.iter().find(|row| row.0 == "Debian:12").unwrap();
        assert_eq!(deb12.4["affected"].as_array().unwrap().len(), 2);
        assert!(rows(
            &json!({"id": "X", "withdrawn": "2026-01-01T00:00:00Z"}),
            "npm"
        )
        .is_empty());
    }

    fn at(second: u32) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(&format!("2026-01-01T00:00:{second:02}Z"))
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn cursor_nao_pula_aviso_que_falhou() {
        let cursor = at(0);
        assert_eq!(next_cursor(cursor, Some(at(9)), None), Some(at(9)));
        assert_eq!(next_cursor(cursor, None, None), None);
        // Empate: A gravado e B falhou no mesmo instante — B volta na próxima.
        let tie = next_cursor(cursor, Some(at(5)), Some(at(5))).unwrap();
        assert!(tie < at(5) && tie > cursor);
        // Sucessos depois da falha não puxam o cursor para além dela.
        let after = next_cursor(cursor, Some(at(9)), Some(at(5))).unwrap();
        assert!(after < at(5));
        assert_eq!(next_cursor(cursor, Some(at(3)), Some(at(5))), Some(at(3)));
        // A primeira falha logo depois do cursor não o mexe.
        assert_eq!(
            next_cursor(
                cursor,
                None,
                Some(cursor + chrono::Duration::microseconds(1))
            ),
            None
        );
    }

    #[test]
    fn espera_depois_de_falha_recente() {
        let now = Utc::now();
        let mut feed = registry_feeds::Model {
            id: 1,
            ecosystem: "npm".into(),
            synced_at: None,
            cursor: None,
            entries: 0,
            error: Some("OSV indisponível".into()),
            created_at: now.into(),
            updated_at: (now - chrono::Duration::minutes(1)).into(),
        };
        assert!(recently_failed(&feed, now));
        feed.updated_at = (now - chrono::Duration::hours(1)).into();
        assert!(!recently_failed(&feed, now));
        feed.updated_at = now.into();
        feed.error = None;
        assert!(!recently_failed(&feed, now));
    }

    #[test]
    fn geracao_em_microssegundos() {
        let generation = generation_now();
        assert_eq!(generation.timestamp_subsec_nanos() % 1000, 0);
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
