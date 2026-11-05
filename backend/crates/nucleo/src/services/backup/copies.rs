//! Cópias da configuração do NetMonitor num destino.
//!
//! O arquivo é o **mesmo** JSON do "Baixar arquivo" ([`backup::export`]): uma
//! cópia tirada do S3 restaura pelo envio de arquivo, e um arquivo baixado pode
//! ser posto no NAS à mão e restaurado daqui.
//!
//! Tudo o que o sistema grava fica em [`BACKUP_DIR`], dentro do prefixo do
//! destino, com nome que carrega o instante (`netmonitor-backup-AAAAMMDD-HHMMSS.json`).
//! É o que torna a retenção segura: ela só apaga o que casa com esse padrão,
//! naquela pasta — nunca um arquivo que o operador guardou no mesmo bucket.

use chrono::{DateTime, Utc};
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use tokio::io::AsyncReadExt;

use super::{
    plan::PLAN_ID,
    service::{self as backup, BackupFile, TableCounts},
};
use crate::{
    models::system_backup_plan,
    services::{
        shared::{
            errors::{AppError, AppResult},
            run_guard::RunGuard,
        },
        storage::{
            normalize_path,
            service::{self, Destination},
            ListOptions, StorageError, StorageExplorer, MAX_LIST_LIMIT,
        },
    },
};

/// Pasta das cópias, dentro do prefixo do destino.
pub const BACKUP_DIR: &str = "netmonitor-backups";

const FILE_PREFIX: &str = "netmonitor-backup-";
const FILE_SUFFIX: &str = ".json";

/// Teto de leitura de um arquivo de backup. Configuração de milhares de
/// dispositivos ainda cabe com folga; um arquivo maior que isso não é backup
/// deste sistema, e lê-lo inteiro na memória derrubaria o processo.
const MAX_BACKUP_BYTES: u64 = 256 * 1024 * 1024;

pub const STATUS_SUCCESS: &str = "success";
pub const STATUS_FAILED: &str = "failed";

/// Escopo da trava de execução (ver [`RunGuard`]): uma cópia do sistema por vez.
pub const RUN_SCOPE: &str = "system-backup";

/// Uma cópia guardada no destino.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupEntry {
    pub key: String,
    pub name: String,
    pub size: Option<i64>,
    pub last_modified: Option<String>,
}

/// O que uma execução fez.
#[derive(Debug, Clone)]
pub struct RunOutcome {
    pub entry: BackupEntry,
    /// Cópias antigas removidas pela retenção.
    pub pruned: usize,
}

/// Nome do arquivo para um instante.
#[must_use]
pub fn file_name_for(at: DateTime<Utc>) -> String {
    format!("{FILE_PREFIX}{}{FILE_SUFFIX}", at.format("%Y%m%d-%H%M%S"))
}

/// A chave é de uma cópia feita por este sistema, na pasta dele?
///
/// É a barreira da retenção **e** da restauração: o explorador navega o bucket
/// inteiro, mas restaurar só aceita o que tem forma de backup.
#[must_use]
pub fn is_backup_key(key: &str) -> bool {
    let key = normalize_path(key);
    let Some((dir, name)) = key.rsplit_once('/') else {
        return false;
    };
    dir == BACKUP_DIR && is_backup_name(name)
}

fn is_backup_name(name: &str) -> bool {
    name.strip_prefix(FILE_PREFIX)
        .and_then(|rest| rest.strip_suffix(FILE_SUFFIX))
        .is_some_and(|stamp| {
            stamp.len() == 15
                && stamp.chars().enumerate().all(|(index, c)| {
                    if index == 8 {
                        c == '-'
                    } else {
                        c.is_ascii_digit()
                    }
                })
        })
}

/// Envia uma cópia da configuração atual ao destino do plano e aplica a
/// retenção do plano.
///
/// O resultado — sucesso ou a mensagem do erro — fica gravado no plano, que é
/// o que a tela e o agendador leem.
///
/// # Errors
///
/// `409` com outra cópia em andamento; qualquer falha da exportação, do envio
/// ou do banco. A falha da **retenção** não falha o backup: a cópia nova já
/// está lá, e é ela que importa.
pub async fn run(
    db: &DatabaseConnection,
    plan: &system_backup_plan::Model,
    destination: &Destination,
    app_version: String,
) -> AppResult<RunOutcome> {
    let _guard = RunGuard::acquire(
        RUN_SCOPE,
        PLAN_ID,
        "Já há um backup do NetMonitor em andamento",
    )?;
    let started = Utc::now();
    let result = upload(db, destination, plan.backup_retention, app_version, started).await;

    let mut row: system_backup_plan::ActiveModel = plan.clone().into();
    row.last_backup_at = Set(Some(started.into()));
    match &result {
        Ok(_) => {
            row.last_backup_status = Set(Some(STATUS_SUCCESS.to_string()));
            row.last_backup_error = Set(None);
        }
        Err(error) => {
            row.last_backup_status = Set(Some(STATUS_FAILED.to_string()));
            row.last_backup_error = Set(Some(error.to_string()));
        }
    }
    row.update(db).await?;
    result
}

async fn upload(
    db: &DatabaseConnection,
    destination: &Destination,
    retention: i32,
    app_version: String,
    at: DateTime<Utc>,
) -> AppResult<RunOutcome> {
    let explorer = destination.explorer()?;
    let file = backup::export(db, app_version).await?;
    let body = serde_json::to_vec(&file)
        .map_err(|err| AppError::Internal(anyhow::anyhow!("serializar backup: {err}")))?;

    let name = file_name_for(at);
    let key = format!("{BACKUP_DIR}/{name}");
    // O adapter envia a partir de um arquivo (o SFTP e o `opendal` fazem
    // streaming dele). O temporário leva o id do destino e um sufixo aleatório:
    // dois destinos no mesmo segundo não disputam o mesmo arquivo.
    let temp = std::env::temp_dir().join(format!(
        "netmonitor-backup-{}-{}.json",
        destination.row.id,
        uuid::Uuid::new_v4()
    ));
    tokio::fs::write(&temp, &body)
        .await
        .map_err(|err| AppError::Internal(anyhow::anyhow!("gravar temporário: {err}")))?;
    let sent = explorer.put_file(&key, &temp).await;
    let _ = tokio::fs::remove_file(&temp).await;
    sent?;

    // Primeiro uso de um SFTP: a identidade vista no envio passa a valer.
    service::remember_identity(db, destination, explorer.as_ref()).await?;

    let pruned = match prune(explorer.as_ref(), retention).await {
        Ok(pruned) => pruned,
        Err(error) => {
            tracing::warn!(
                destination = destination.row.id,
                %error,
                "backup enviado, mas a retenção das cópias antigas falhou"
            );
            0
        }
    };

    Ok(RunOutcome {
        entry: BackupEntry {
            key,
            name,
            size: i64::try_from(body.len()).ok(),
            last_modified: Some(at.to_rfc3339()),
        },
        pruned,
    })
}

/// As cópias guardadas no destino, da mais recente para a mais antiga.
///
/// # Errors
///
/// Falha de listagem do destino.
pub async fn list(destination: &Destination) -> AppResult<Vec<BackupEntry>> {
    let explorer = destination.explorer()?;
    Ok(list_with(explorer.as_ref()).await?)
}

async fn list_with(explorer: &dyn StorageExplorer) -> Result<Vec<BackupEntry>, StorageError> {
    let mut entries = Vec::new();
    let mut cursor = None;
    loop {
        let page = explorer
            .list_objects(
                BACKUP_DIR,
                &ListOptions {
                    cursor: cursor.take(),
                    limit: Some(MAX_LIST_LIMIT),
                    prefix: Some(FILE_PREFIX.to_string()),
                },
            )
            .await?;
        entries.extend(
            page.objects
                .into_iter()
                .filter(|object| !object.is_directory && is_backup_key(&object.key))
                .map(|object| BackupEntry {
                    key: object.key,
                    name: object.name,
                    size: object.size,
                    last_modified: object.last_modified,
                }),
        );
        match page.next_cursor {
            Some(next) if page.is_truncated => cursor = Some(next),
            _ => break,
        }
    }
    // O nome carrega o instante com dígitos de largura fixa: ordem de texto é
    // ordem cronológica, sem depender do `last_modified` de cada provider.
    entries.sort_by(|a, b| b.name.cmp(&a.name));
    Ok(entries)
}

/// Apaga as cópias além das `retention` mais recentes.
async fn prune(explorer: &dyn StorageExplorer, retention: i32) -> Result<usize, StorageError> {
    let keep = usize::try_from(retention.max(1)).unwrap_or(1);
    let entries = list_with(explorer).await?;
    let mut pruned = 0;
    for old in entries.iter().skip(keep) {
        explorer.delete_object(&old.key, false).await?;
        pruned += 1;
    }
    Ok(pruned)
}

/// Lê uma cópia do destino.
///
/// # Errors
///
/// Chave que não é de backup, arquivo grande demais, ausente ou que não é um
/// JSON de backup.
pub async fn read(destination: &Destination, key: &str) -> AppResult<BackupFile> {
    if !is_backup_key(key) {
        return Err(AppError::validation(
            "Escolha um arquivo de backup da pasta netmonitor-backups",
        ));
    }
    let explorer = destination.explorer()?;
    let reader = explorer.read_object(key).await?;
    let mut body = Vec::new();
    reader
        .take(MAX_BACKUP_BYTES + 1)
        .read_to_end(&mut body)
        .await
        .map_err(|err| AppError::business_rule(format!("Falha ao ler o backup: {err}")))?;
    if body.len() as u64 > MAX_BACKUP_BYTES {
        return Err(AppError::validation(
            "O arquivo passa de 256 MB — não é um backup deste sistema",
        ));
    }
    serde_json::from_slice(&body)
        .map_err(|err| AppError::validation(format!("O arquivo não é um backup válido: {err}")))
}

/// O que uma cópia contém, sem tocar no banco.
///
/// # Errors
///
/// Ver [`read`] e [`backup::inspect`].
pub async fn preview(destination: &Destination, key: &str) -> AppResult<TableCounts> {
    backup::inspect(&read(destination, key).await?)
}

/// Substitui a configuração atual pela da cópia.
///
/// # Errors
///
/// Ver [`read`] e [`backup::restore`]; o banco fica intocado em qualquer falha.
pub async fn restore(
    db: &DatabaseConnection,
    destination: &Destination,
    key: &str,
) -> AppResult<TableCounts> {
    let file = read(destination, key).await?;
    backup::restore(db, &file).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::storage::{config::LocalConfig, local::LocalExplorer};
    use chrono::TimeZone;
    use std::path::Path;

    #[test]
    fn o_nome_carrega_o_instante_em_largura_fixa() {
        let at = Utc.with_ymd_and_hms(2026, 3, 7, 4, 5, 6).unwrap();
        assert_eq!(file_name_for(at), "netmonitor-backup-20260307-040506.json");
    }

    #[test]
    fn so_e_backup_o_que_esta_na_pasta_e_tem_o_formato() {
        assert!(is_backup_key(
            "netmonitor-backups/netmonitor-backup-20260307-040506.json"
        ));
        // Arquivo do operador no mesmo bucket: a retenção não pode tocá-lo.
        for other in [
            "netmonitor-backups/notas.txt",
            "netmonitor-backups/netmonitor-backup-ontem.json",
            "netmonitor-backup-20260307-040506.json",
            "outra/netmonitor-backups/netmonitor-backup-20260307-040506.json",
            "netmonitor-backups/netmonitor-backup-20260307-040506.json.bak",
        ] {
            assert!(!is_backup_key(other), "aceitou {other}");
        }
    }

    async fn explorer_with(names: &[&str]) -> (tempfile::TempDir, LocalExplorer) {
        let dir = tempfile::tempdir().expect("temporário");
        let folder = dir.path().join(BACKUP_DIR);
        tokio::fs::create_dir_all(&folder).await.unwrap();
        for name in names {
            tokio::fs::write(folder.join(name), b"{}").await.unwrap();
        }
        let explorer = LocalExplorer::new(&LocalConfig::default(), dir.path()).unwrap();
        (dir, explorer)
    }

    #[tokio::test]
    async fn a_listagem_vem_da_mais_recente_e_ignora_o_resto() {
        let (_dir, explorer) = explorer_with(&[
            "netmonitor-backup-20260101-000000.json",
            "netmonitor-backup-20260301-000000.json",
            "leia-me.txt",
            "netmonitor-backup-20260201-000000.json",
        ])
        .await;

        let names: Vec<String> = list_with(&explorer)
            .await
            .unwrap()
            .into_iter()
            .map(|entry| entry.name)
            .collect();
        assert_eq!(
            names,
            vec![
                "netmonitor-backup-20260301-000000.json",
                "netmonitor-backup-20260201-000000.json",
                "netmonitor-backup-20260101-000000.json",
            ]
        );
    }

    #[tokio::test]
    async fn a_retencao_apaga_so_as_copias_antigas_do_sistema() {
        let (dir, explorer) = explorer_with(&[
            "netmonitor-backup-20260101-000000.json",
            "netmonitor-backup-20260201-000000.json",
            "netmonitor-backup-20260301-000000.json",
            "leia-me.txt",
        ])
        .await;

        assert_eq!(prune(&explorer, 2).await.unwrap(), 1);

        let folder = dir.path().join(BACKUP_DIR);
        assert!(!folder
            .join("netmonitor-backup-20260101-000000.json")
            .exists());
        assert!(folder
            .join("netmonitor-backup-20260301-000000.json")
            .exists());
        assert!(
            folder.join("leia-me.txt").exists(),
            "apagou arquivo do operador"
        );
    }

    #[tokio::test]
    async fn uma_pasta_sem_backups_lista_vazio() {
        let dir = tempfile::tempdir().unwrap();
        let explorer = LocalExplorer::new(&LocalConfig::default(), dir.path()).unwrap();
        assert!(list_with(&explorer).await.unwrap().is_empty());
        assert!(Path::new(dir.path()).exists());
    }
}
