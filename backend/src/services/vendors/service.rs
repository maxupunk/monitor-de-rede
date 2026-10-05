//! Guarda, carrega e atualiza o registro de fabricantes do IEEE.
//!
//! * [`ensure_loaded`] põe o registro do banco na memória do processo — a API,
//!   o agendador e quem roda a descoberta chamam antes de consultar.
//! * [`refresh`] baixa os três CSVs, troca a tabela numa transação só (ou fica
//!   tudo como estava) e reinstala a memória.
//! * [`spawn_auto_update`] mantém o registro em dia: baixa quando está vazio
//!   ou com mais de [`MAX_AGE_DAYS`]. `OUI_AUTO_UPDATE=false` desliga — uma
//!   instalação sem internet segue com a tabela embutida.

use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use chrono::{DateTime, Utc};
use loco_rs::app::AppContext;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryOrder, QuerySelect, Set,
};
use serde::Serialize;
use tokio::sync::Mutex;

use super::{
    builtin,
    ieee::{self, Assignment},
    registry::{self, OuiRegistry},
};
use crate::services::shared::transaction::begin_write;
use crate::{
    models::oui_vendors,
    services::shared::errors::{AppError, AppResult},
};

/// Idade a partir da qual o registro é baixado de novo. O IEEE publica blocos
/// novos toda semana, mas um mês de atraso não muda o que a rede tem.
pub const MAX_AGE_DAYS: i64 = 30;
/// De quanto em quanto tempo a atualização automática confere a idade.
const AUTO_UPDATE_CHECK: Duration = Duration::from_secs(24 * 60 * 60);
/// Espera antes da primeira conferência: o boot já tem trabalho de sobra.
const AUTO_UPDATE_DELAY: Duration = Duration::from_secs(90);
/// O `oui.csv` tem ~3 MB; enlace lento de filial precisa de folga.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);
/// Linhas por `INSERT`: 4 colunas × 250 fica longe do limite de variáveis do
/// SQLite e ainda é rápido no PostgreSQL.
const INSERT_CHUNK: usize = 250;

static LOADED: AtomicBool = AtomicBool::new(false);
/// Uma atualização por vez: duas baixando juntas só gastariam banda.
static REFRESHING: Mutex<()> = Mutex::const_new(());

/// Situação do registro, para a tela de configurações.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryStatus {
    /// Blocos do IEEE guardados.
    pub entries: u64,
    pub updated_at: Option<String>,
    /// Prefixos da tabela embutida (a reserva offline).
    pub builtin_entries: usize,
    /// Vazio ou mais velho que [`MAX_AGE_DAYS`].
    pub stale: bool,
    pub auto_update: bool,
    pub updating: bool,
    pub sources: Vec<String>,
}

/// Resultado de uma atualização.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshOutcome {
    pub entries: usize,
    /// Blocos por tamanho: `MA-L`, `MA-M`, `MA-S`.
    pub by_registry: BTreeMap<String, usize>,
    pub updated_at: String,
}

/// As URLs configuradas (`OUI_SOURCES`) ou as do IEEE.
#[must_use]
pub fn configured_sources() -> Vec<String> {
    std::env::var("OUI_SOURCES")
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|url| !url.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|urls| !urls.is_empty())
        .unwrap_or_else(|| {
            ieee::DEFAULT_SOURCES
                .iter()
                .map(|url| (*url).to_string())
                .collect()
        })
}

/// A atualização automática está ligada (padrão) ou foi desligada.
#[must_use]
pub fn auto_update_enabled() -> bool {
    !matches!(
        std::env::var("OUI_AUTO_UPDATE")
            .ok()
            .as_deref()
            .map(str::trim),
        Some("false" | "0" | "no" | "off")
    )
}

/// Carrega o registro do banco para a memória do processo.
///
/// # Errors
///
/// Falha de banco.
pub async fn load<C: ConnectionTrait>(db: &C) -> AppResult<usize> {
    let rows: Vec<(String, String)> = oui_vendors::Entity::find()
        .select_only()
        .column(oui_vendors::Column::Prefix)
        .column(oui_vendors::Column::Organization)
        .into_tuple()
        .all(db)
        .await?;
    let count = rows.len();
    registry::install(OuiRegistry::new(rows));
    LOADED.store(true, Ordering::Release);
    Ok(count)
}

/// Carrega uma vez por processo. Banco indisponível não impede a consulta:
/// a tabela embutida continua respondendo.
pub async fn ensure_loaded<C: ConnectionTrait>(db: &C) {
    if LOADED.load(Ordering::Acquire) {
        return;
    }
    if let Err(error) = load(db).await {
        tracing::debug!(%error, "registro de fabricantes não carregado; usando a tabela embutida");
    }
}

/// Situação do registro guardado.
///
/// # Errors
///
/// Falha de banco.
pub async fn status<C: ConnectionTrait>(db: &C) -> AppResult<RegistryStatus> {
    let entries = oui_vendors::Entity::find().count(db).await?;
    let updated_at = oui_vendors::Entity::find()
        .order_by_desc(oui_vendors::Column::UpdatedAt)
        .one(db)
        .await?
        .map(|row| row.updated_at.with_timezone(&Utc));
    Ok(RegistryStatus {
        entries,
        updated_at: updated_at.map(|value| value.to_rfc3339()),
        builtin_entries: builtin::len(),
        stale: is_stale(entries, updated_at, Utc::now()),
        auto_update: auto_update_enabled(),
        updating: REFRESHING.try_lock().is_err(),
        sources: configured_sources(),
    })
}

fn is_stale(entries: u64, updated_at: Option<DateTime<Utc>>, now: DateTime<Utc>) -> bool {
    entries == 0 || updated_at.is_none_or(|at| (now - at).num_days() >= MAX_AGE_DAYS)
}

fn client() -> AppResult<reqwest::Client> {
    // O site do IEEE recusa agentes sem cara de navegador.
    reqwest::Client::builder()
        .timeout(DOWNLOAD_TIMEOUT)
        .connect_timeout(Duration::from_secs(15))
        .user_agent("Mozilla/5.0 (compatible; NetMonitor OUI updater)")
        .build()
        .map_err(|error| AppError::Internal(anyhow::Error::new(error)))
}

async fn download(client: &reqwest::Client, url: &str) -> AppResult<Vec<Assignment>> {
    let response = client.get(url).send().await.map_err(|error| {
        AppError::business_rule(format!("Não foi possível baixar {url}: {error}"))
    })?;
    if !response.status().is_success() {
        return Err(AppError::business_rule(format!(
            "{url} respondeu HTTP {}",
            response.status().as_u16()
        )));
    }
    let text = response.text().await.map_err(|error| {
        AppError::business_rule(format!("Download de {url} interrompido: {error}"))
    })?;
    Ok(ieee::parse(&text))
}

/// Baixa o registro de `sources`, substitui a tabela e reinstala a memória.
/// Uma fonte que falhe derruba a atualização inteira: meio registro faria um
/// fabricante "sumir" sem explicação.
///
/// # Errors
///
/// Outra atualização em andamento, falha de download, registro vazio ou
/// falha de banco — em todos os casos a tabela anterior continua valendo.
pub async fn refresh(db: &DatabaseConnection, sources: &[String]) -> AppResult<RefreshOutcome> {
    let Ok(_guard) = REFRESHING.try_lock() else {
        return Err(AppError::business_rule(
            "A base de fabricantes já está sendo atualizada.",
        ));
    };
    let client = client()?;
    let mut assignments = BTreeMap::<String, Assignment>::new();
    for url in sources {
        for assignment in download(&client, url).await? {
            assignments.insert(assignment.prefix.clone(), assignment);
        }
    }
    if assignments.is_empty() {
        return Err(AppError::business_rule(
            "As fontes do registro não trouxeram nenhum fabricante.",
        ));
    }

    let now = Utc::now();
    let mut by_registry = BTreeMap::<String, usize>::new();
    let rows: Vec<oui_vendors::ActiveModel> = assignments
        .values()
        .map(|assignment| {
            *by_registry.entry(assignment.registry.clone()).or_default() += 1;
            oui_vendors::ActiveModel {
                prefix: Set(assignment.prefix.clone()),
                organization: Set(assignment.organization.clone()),
                registry: Set(assignment.registry.clone()),
                updated_at: Set(now.into()),
            }
        })
        .collect();
    let txn = begin_write(db).await?;
    oui_vendors::Entity::delete_many().exec(&txn).await?;
    for chunk in rows.chunks(INSERT_CHUNK) {
        oui_vendors::Entity::insert_many(chunk.to_vec())
            .exec(&txn)
            .await?;
    }
    txn.commit().await?;

    registry::install(OuiRegistry::new(
        assignments
            .into_values()
            .map(|assignment| (assignment.prefix, assignment.organization)),
    ));
    LOADED.store(true, Ordering::Release);
    let entries = rows.len();
    tracing::info!(entries, "registro de fabricantes do IEEE atualizado");
    Ok(RefreshOutcome {
        entries,
        by_registry,
        updated_at: now.to_rfc3339(),
    })
}

/// Carrega o registro e o mantém em dia em segundo plano (processo da API).
pub fn spawn_auto_update(ctx: &AppContext) {
    let db = ctx.db.clone();
    tokio::spawn(async move {
        ensure_loaded(&db).await;
        tokio::time::sleep(AUTO_UPDATE_DELAY).await;
        loop {
            if auto_update_enabled() {
                match status(&db).await {
                    Ok(current) if current.stale => {
                        if let Err(error) = refresh(&db, &configured_sources()).await {
                            tracing::warn!(%error, "atualização automática do registro de fabricantes falhou");
                        }
                    }
                    Ok(_) => {}
                    Err(error) => {
                        tracing::debug!(%error, "situação do registro de fabricantes indisponível")
                    }
                }
            }
            tokio::time::sleep(AUTO_UPDATE_CHECK).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use chrono::Duration as ChronoDuration;

    use super::*;

    #[test]
    fn registro_vazio_ou_velho_precisa_atualizar() {
        let now = Utc::now();
        assert!(is_stale(0, Some(now), now));
        assert!(is_stale(10, None, now));
        assert!(is_stale(
            10,
            Some(now - ChronoDuration::days(MAX_AGE_DAYS)),
            now
        ));
        assert!(!is_stale(10, Some(now - ChronoDuration::days(3)), now));
    }
}
