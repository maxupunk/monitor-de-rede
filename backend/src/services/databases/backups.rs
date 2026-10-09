//! Uma execução de backup de banco, e a restauração a partir dela.
//!
//! Backup: para cada banco da conexão, dump num arquivo temporário → envio ao
//! armazenamento → uma linha em `database_backups` → retenção. Um banco que
//! falha não impede os outros; o resumo vai para o cadastro da conexão.
//!
//! Pela ponte de um agente, o dump que perde o canal é refeito do zero
//! ([`Reached::retrying`]) na mesma linha do histórico. A restauração não:
//! reaplicar um arquivo pela metade não é seguro, e ela falha com a causa.
//!
//! As duas operações rodam em segundo plano: a rota devolve o andamento inicial
//! e o resto chega pelo SSE ([`super::jobs`]). A trava ([`RunGuard`]) é pega
//! **antes** de devolver, para o segundo clique receber `409` na hora, e não
//! um job que falha depois.

use chrono::{DateTime, Utc};
use loco_rs::app::AppContext;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};

use super::{
    dump::{DumpFile, DumpStats, DumpWriter},
    jobs::{DatabaseJobKind, DatabaseJobSnapshot, Job},
    reach::{reach, Reached},
    restore::LineReader,
    service::{self, Connection},
    sql::validate_database_name,
    DatabaseError, DatabaseTarget, Progress, RestoreMode,
};
use crate::{
    models::{database_backups, database_connections},
    services::{
        events::EventBus,
        shared::{
            errors::{AppError, AppResult},
            run_guard::RunGuard,
        },
        storage::{self, service::Destination},
    },
};

pub const BACKUP_SCOPE: &str = "database-backup";
pub const RESTORE_SCOPE: &str = "database-restore";

pub const STATUS_RUNNING: &str = "running";
pub const STATUS_SUCCESS: &str = "success";
pub const STATUS_FAILED: &str = "failed";

/// Pasta dos dumps dentro do armazenamento.
pub const BACKUP_DIR: &str = "database-backups";

/// Falhas guardadas no histórico por banco — elas não têm arquivo, e só as
/// recentes ajudam a diagnosticar.
const KEEP_FAILED: usize = 10;

/// Quem disparou.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    Manual,
    Scheduled,
}

impl Trigger {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Scheduled => "scheduled",
        }
    }
}

/// Trecho de caminho seguro: minúsculas, dígitos e `-`.
#[must_use]
pub fn slug(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "banco".to_string()
    } else {
        out
    }
}

/// Chave do arquivo no armazenamento.
///
/// Leva o id da conexão além do nome: renomear a conexão não pode misturar as
/// cópias com as de outra que passe a ter o nome antigo.
#[must_use]
pub fn object_key(
    connection: &database_connections::Model,
    database: &str,
    at: DateTime<Utc>,
) -> String {
    let db = slug(database);
    format!(
        "{BACKUP_DIR}/{}-{}/{db}/{db}-{}.sql.gz",
        slug(&connection.name),
        connection.id,
        at.format("%Y%m%d-%H%M%S")
    )
}

/// Inicia o backup de uma conexão em segundo plano.
///
/// # Errors
///
/// `409` com outro backup da mesma conexão em andamento; conexão sem
/// armazenamento; armazenamento ou senha ilegíveis.
pub async fn start_backup(
    ctx: &AppContext,
    connection_id: i64,
    trigger: Trigger,
) -> AppResult<DatabaseJobSnapshot> {
    let guard = RunGuard::acquire(
        BACKUP_SCOPE,
        connection_id,
        "Já há um backup em andamento para esta conexão",
    )?;
    let connection = service::load(&ctx.db, connection_id).await?;
    let destination = destination_of(&ctx.db, &connection).await?;
    let job = Job::start(
        EventBus::from_context(ctx).ok(),
        DatabaseJobKind::Backup,
        connection_id,
        u32::try_from(connection.databases.len()).unwrap_or(0),
    );
    let snapshot = job.snapshot();
    let ctx = ctx.clone();
    tokio::spawn(async move {
        let _guard = guard;
        run_backup(&ctx, &connection, &destination, &job, trigger).await;
        service::publish_updated(&ctx).await;
    });
    Ok(snapshot)
}

/// O armazenamento para onde a conexão manda as cópias.
///
/// # Errors
///
/// Conexão sem armazenamento, ou armazenamento que não abre.
pub async fn destination_of(
    db: &DatabaseConnection,
    connection: &Connection,
) -> AppResult<Destination> {
    let id = connection.row.storage_destination_id.ok_or_else(|| {
        AppError::validation(format!(
            "A conexão \"{}\" não tem armazenamento — edite-a e escolha para onde vão as cópias",
            connection.row.name
        ))
    })?;
    storage::service::load(db, id).await
}

/// Executa o backup de todos os bancos da conexão e grava o resumo nela.
///
/// Não devolve erro: cada falha fica no histórico do banco e no resumo da
/// conexão, que é o que a tela e o agendador leem.
pub async fn run_backup(
    ctx: &AppContext,
    connection: &Connection,
    destination: &Destination,
    job: &Job,
    trigger: Trigger,
) {
    let db = &ctx.db;
    let started = Utc::now();
    let outcome = backup_all(ctx, connection, destination, job, trigger).await;

    let (ok, message) = match &outcome {
        Ok(summary) if summary.failures.is_empty() => (
            true,
            format!(
                "{} {} para {}",
                list_names(&summary.copied),
                if summary.copied.len() == 1 {
                    "copiado"
                } else {
                    "copiados"
                },
                destination.row.name
            ),
        ),
        Ok(summary) => (false, summary.failures.join("; ")),
        Err(error) => (false, error.to_string()),
    };
    let mut row: database_connections::ActiveModel = connection.row.clone().into();
    row.last_backup_at = Set(Some(started.into()));
    row.last_backup_status = Set(Some(
        if ok { STATUS_SUCCESS } else { STATUS_FAILED }.to_string(),
    ));
    row.last_backup_error = Set((!ok).then(|| message.clone()));
    if let Err(error) = row.update(db).await {
        tracing::warn!(%error, "não foi possível gravar o resultado do backup de banco");
    }
    job.finish(ok, message);
}

/// "vendas", "vendas e estoque", "vendas, estoque e mais 3".
fn list_names(names: &[String]) -> String {
    match names {
        [] => "Nenhum banco".to_string(),
        [one] => one.clone(),
        [a, b] => format!("{a} e {b}"),
        [a, b, rest @ ..] => format!("{a}, {b} e mais {}", rest.len()),
    }
}

/// O que uma execução fez, banco a banco.
struct Summary {
    copied: Vec<String>,
    /// `"vendas: mensagem"`.
    failures: Vec<String>,
}

/// `Err` só quando nem deu para começar; falha de um banco vai no resumo.
async fn backup_all(
    ctx: &AppContext,
    connection: &Connection,
    destination: &Destination,
    job: &Job,
    trigger: Trigger,
) -> AppResult<Summary> {
    let db = &ctx.db;
    let explorer = destination.explorer()?;
    if connection.row.via_probe_id.is_some() {
        job.stage("Abrindo a ponte pelo agente");
    }
    // A ponte (se houver) vive até o último banco ser copiado.
    let reached = reach(ctx, connection.target(), connection.row.via_probe_id).await?;
    let databases = if connection.databases.is_empty() {
        let driver = connection.engine.driver();
        reached
            .retrying(job, || driver.probe(&reached.target))
            .await?
            .value
            .databases
    } else {
        connection.databases.clone()
    };
    if databases.is_empty() {
        return Err(AppError::business_rule(
            "Nenhum banco encontrado nesta conexão",
        ));
    }
    job.total(u32::try_from(databases.len()).unwrap_or(u32::MAX));

    let mut summary = Summary {
        copied: Vec::new(),
        failures: Vec::new(),
    };
    let run = Run {
        db,
        connection,
        destination,
        explorer: explorer.as_ref(),
        reached: &reached,
        job,
        trigger,
    };
    for database in &databases {
        job.begin_database(database);
        match backup_one(&run, database).await {
            Ok(()) => summary.copied.push(database.clone()),
            Err(error) => summary.failures.push(format!("{database}: {error}")),
        }
        job.database_done();
    }
    Ok(summary)
}

/// O que é igual para todos os bancos de uma execução.
struct Run<'a> {
    db: &'a DatabaseConnection,
    connection: &'a Connection,
    destination: &'a Destination,
    explorer: &'a dyn storage::StorageExplorer,
    /// Já resolvido: direto ou a ponta local da ponte do agente.
    reached: &'a Reached,
    job: &'a Job,
    trigger: Trigger,
}

async fn backup_one(run: &Run<'_>, database: &str) -> AppResult<()> {
    let Run {
        db,
        connection,
        destination,
        explorer,
        reached,
        job,
        trigger,
    } = *run;
    let started = Utc::now();
    let history = database_backups::ActiveModel {
        connection_id: Set(connection.row.id),
        database_name: Set(database.to_string()),
        storage_destination_id: Set(Some(destination.row.id)),
        status: Set(STATUS_RUNNING.to_string()),
        warnings: Set(serde_json::json!([])),
        trigger: Set(trigger.as_str().to_string()),
        started_at: Set(started.into()),
        ..Default::default()
    }
    .insert(db)
    .await?;

    let temp = std::env::temp_dir().join(format!(
        "netmonitor-db-{}-{}.sql.gz",
        connection.row.id,
        uuid::Uuid::new_v4()
    ));
    let result = async {
        // Cada tentativa recria o arquivo: refazer é começar do zero.
        let attempted = reached
            .retrying(job, || {
                dump_to(connection, &reached.target, database, &temp, job)
            })
            .await?;
        let (mut stats, file) = attempted.value;
        if attempted.attempts > 1 {
            stats.warnings.push(format!(
                "O canal com o agente caiu durante a cópia; ela foi refeita do zero e \
                 concluída na tentativa {}",
                attempted.attempts
            ));
        }
        job.stage("Enviando ao armazenamento");
        let key = object_key(&connection.row, database, started);
        explorer
            .put_file(&key, &file.path)
            .await
            .map_err(AppError::from)?;
        Ok::<_, AppError>((stats, file, key))
    }
    .await;
    let _ = tokio::fs::remove_file(&temp).await;

    let elapsed = (Utc::now() - started).num_milliseconds();
    let mut row: database_backups::ActiveModel = history.into();
    row.duration_ms = Set(elapsed);
    row.finished_at = Set(Some(Utc::now().into()));
    match result {
        Ok((stats, file, key)) => {
            row.status = Set(STATUS_SUCCESS.to_string());
            row.object_key = Set(Some(key));
            row.size_bytes = Set(i64::try_from(file.size).ok());
            row.checksum = Set(Some(file.checksum));
            row.tables = Set(i32::try_from(stats.tables).unwrap_or(i32::MAX));
            row.rows = Set(i64::try_from(stats.rows).unwrap_or(i64::MAX));
            row.warnings = Set(serde_json::json!(stats.warnings));
            row.update(db).await?;
            if let Err(error) = prune(db, connection, database).await {
                tracing::warn!(%error, database, "retenção dos backups de banco falhou");
            }
            Ok(())
        }
        Err(error) => {
            row.status = Set(STATUS_FAILED.to_string());
            row.error = Set(Some(error.to_string()));
            row.update(db).await?;
            Err(error)
        }
    }
}

/// Uma tentativa de dump de `database` em `path`.
async fn dump_to(
    connection: &Connection,
    target: &DatabaseTarget,
    database: &str,
    path: &std::path::Path,
    job: &Job,
) -> Result<(DumpStats, DumpFile), DatabaseError> {
    let mut writer = DumpWriter::create(path).await?;
    let stats = connection
        .engine
        .driver()
        .dump(target, database, &mut writer, job)
        .await?;
    Ok((stats, writer.finish().await?))
}

/// Apaga as cópias além da retenção — o arquivo e a linha.
///
/// O arquivo é apagado do armazenamento **em que foi gravado**, que pode não
/// ser o atual da conexão: trocar de destino não pode deixar órfãs as cópias
/// antigas.
async fn prune(db: &DatabaseConnection, connection: &Connection, database: &str) -> AppResult<()> {
    let keep = usize::try_from(connection.row.backup_retention.max(1)).unwrap_or(1);
    let history = database_backups::Entity::find()
        .filter(database_backups::Column::ConnectionId.eq(connection.row.id))
        .filter(database_backups::Column::DatabaseName.eq(database))
        .order_by_desc(database_backups::Column::StartedAt)
        .all(db)
        .await?;
    let (successes, failures): (Vec<_>, Vec<_>) = history
        .into_iter()
        .filter(|row| row.status != STATUS_RUNNING)
        .partition(|row| row.status == STATUS_SUCCESS);

    for old in successes.into_iter().skip(keep) {
        if let (Some(destination_id), Some(key)) = (old.storage_destination_id, &old.object_key) {
            match storage::service::load(db, destination_id).await {
                Ok(destination) => {
                    destination
                        .explorer()?
                        .delete_object(key, false)
                        .await
                        .or_else(|error| match error {
                            // Já não estava lá: o objetivo foi alcançado.
                            storage::StorageError::NotFound(_) => Ok(()),
                            other => Err(other),
                        })?;
                }
                Err(error) => tracing::debug!(%error, "armazenamento da cópia antiga não abre"),
            }
        }
        database_backups::Entity::delete_by_id(old.id)
            .exec(db)
            .await?;
    }
    for old in failures.into_iter().skip(KEEP_FAILED) {
        database_backups::Entity::delete_by_id(old.id)
            .exec(db)
            .await?;
    }
    Ok(())
}

/// Histórico de uma conexão, mais recentes primeiro.
///
/// # Errors
///
/// Erro do banco.
pub async fn history(
    db: &DatabaseConnection,
    connection_id: i64,
) -> AppResult<Vec<database_backups::Model>> {
    Ok(database_backups::Entity::find()
        .filter(database_backups::Column::ConnectionId.eq(connection_id))
        .order_by_desc(database_backups::Column::StartedAt)
        .all(db)
        .await?)
}

/// O que a restauração precisa saber.
pub struct RestoreRequest {
    pub backup_id: i64,
    pub target_connection_id: i64,
    pub database: String,
    pub mode: RestoreMode,
}

/// Inicia a restauração de uma cópia em segundo plano.
///
/// # Errors
///
/// Cópia sem arquivo, SGBD diferente, nome inválido, armazenamento removido ou
/// outra restauração na mesma conexão.
pub async fn start_restore(
    ctx: &AppContext,
    request: RestoreRequest,
) -> AppResult<DatabaseJobSnapshot> {
    let backup = database_backups::Entity::find_by_id(request.backup_id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Backup não encontrado"))?;
    let (Some(key), STATUS_SUCCESS) = (backup.object_key.clone(), backup.status.as_str()) else {
        return Err(AppError::validation(
            "Só uma cópia concluída pode ser restaurada",
        ));
    };
    let source = service::find(&ctx.db, backup.connection_id).await?;
    let target = service::load(&ctx.db, request.target_connection_id).await?;
    let source_engine = super::DatabaseEngine::parse(&source.engine)?;
    if source_engine.family() != target.engine.family() {
        let label = source_engine.label();
        return Err(AppError::validation(format!(
            "Uma cópia de {label} só restaura em {label}"
        )));
    }
    let database = validate_database_name(request.database.trim())
        .map_err(AppError::from)?
        .to_string();
    let destination_id = backup
        .storage_destination_id
        .ok_or_else(|| AppError::business_rule("O armazenamento desta cópia foi removido"))?;
    let destination = storage::service::load(&ctx.db, destination_id).await?;

    let guard = RunGuard::acquire(
        RESTORE_SCOPE,
        target.row.id,
        "Já há uma restauração em andamento nesta conexão",
    )?;
    let job = Job::start(
        EventBus::from_context(ctx).ok(),
        DatabaseJobKind::Restore,
        target.row.id,
        1,
    );
    job.begin_database(&database);
    let snapshot = job.snapshot();
    let checksum = backup.checksum.clone();
    let ctx = ctx.clone();
    tokio::spawn(async move {
        let _guard = guard;
        let result = async {
            if target.row.via_probe_id.is_some() {
                job.stage("Abrindo a ponte pelo agente");
            }
            let reached = reach(&ctx, target.target(), target.row.via_probe_id).await?;
            let reader = destination
                .explorer()?
                .read_object(&key)
                .await
                .map_err(AppError::from)?;
            let mut lines = LineReader::gzip(reader, checksum);
            let stats = target
                .engine
                .driver()
                .restore(&reached.target, &database, request.mode, &mut lines, &job)
                .await?;
            Ok::<_, AppError>(stats)
        }
        .await;
        match result {
            Ok(stats) => {
                job.database_done();
                job.finish(
                    true,
                    format!(
                        "Banco \"{database}\" restaurado: {} comandos, {} linhas",
                        stats.statements, stats.rows
                    ),
                );
            }
            Err(error) => job.finish(false, error.to_string()),
        }
    });
    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn resume_a_lista_de_bancos_copiados() {
        let names = |list: &[&str]| list.iter().map(ToString::to_string).collect::<Vec<_>>();
        assert_eq!(list_names(&names(&["erp"])), "erp");
        assert_eq!(list_names(&names(&["erp", "rh"])), "erp e rh");
        assert_eq!(list_names(&names(&["a", "b", "c", "d"])), "a, b e mais 2");
    }

    #[test]
    fn slug_so_tem_minusculas_digitos_e_hifen() {
        assert_eq!(slug("ERP Produção"), "erp-produ-o");
        assert_eq!(slug("vendas_2026"), "vendas-2026");
        assert_eq!(slug("../.."), "banco");
    }

    #[test]
    fn a_chave_separa_conexao_e_banco_e_carrega_o_instante() {
        let now = Utc::now().into();
        let connection = database_connections::Model {
            id: 7,
            name: "ERP".into(),
            engine: "postgres".into(),
            host: "db".into(),
            port: 5432,
            username: "u".into(),
            password_encrypted: String::new(),
            ssl_mode: "prefer".into(),
            databases: serde_json::json!([]),
            storage_destination_id: None,
            via_probe_id: None,
            backup_enabled: false,
            backup_interval_hours: 24,
            backup_retention: 7,
            last_backup_at: None,
            last_backup_status: None,
            last_backup_error: None,
            created_at: now,
            updated_at: now,
        };
        let at = Utc.with_ymd_and_hms(2026, 10, 7, 3, 0, 9).unwrap();
        assert_eq!(
            object_key(&connection, "Vendas", at),
            "database-backups/erp-7/vendas/vendas-20261007-030009.sql.gz"
        );
    }
}
