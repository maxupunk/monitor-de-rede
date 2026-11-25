//! Gerenciamento e persistência de chaves VAPID do NetMonitor.

use sea_orm::{ConnectionTrait, DatabaseBackend, Statement, TransactionSession, TransactionTrait};
use tracing::info;

use super::crypto::{generate_vapid_key_pair, VapidKeyPair};
use crate::{
    models::system_settings,
    services::shared::{errors::AppResult, transaction::begin_write},
};

const DEFAULT_SUBJECT: &str = "mailto:admin@netmonitor.local";
const SETTING_PUBLIC_KEY: &str = "vapid_public_key";
const SETTING_PRIVATE_KEY: &str = "vapid_private_key";
const SETTING_SUBJECT: &str = "vapid_subject";

/// Obtém ou inicializa o par de chaves VAPID do sistema.
///
/// Prioridade:
/// 1. Variáveis de ambiente (`VAPID_PUBLIC_KEY`, `VAPID_PRIVATE_KEY`, `VAPID_SUBJECT`)
/// 2. Banco de dados (`system_settings`)
/// 3. Geração automática e persistência em `system_settings` (Zero-Config)
///
/// # Errors
///
/// Propaga erro de conexão com o banco de dados.
pub async fn get_or_create_vapid_keys<C: ConnectionTrait + TransactionTrait>(
    db: &C,
) -> AppResult<VapidKeyPair> {
    // 1. Tenta carregar do ambiente
    let env_public = std::env::var("VAPID_PUBLIC_KEY")
        .ok()
        .filter(|s| !s.trim().is_empty());
    let env_private = std::env::var("VAPID_PRIVATE_KEY")
        .ok()
        .filter(|s| !s.trim().is_empty());
    let env_subject = std::env::var("VAPID_SUBJECT")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_SUBJECT.to_string());

    if let (Some(pub_key), Some(priv_key)) = (env_public, env_private) {
        return Ok(VapidKeyPair {
            public_key_base64: pub_key,
            private_key_base64: priv_key,
            subject: env_subject,
        });
    }

    // 2. Caminho rápido fora da transação de escrita
    if let Some(existing) = load_from_db(db).await? {
        return Ok(existing);
    }

    // 3. Cold-start atômico: evita corrida entre requisições simultâneas
    // gravando chaves pública e privada de pares diferentes.
    let txn = begin_write(db).await?;
    if txn.get_database_backend() == DatabaseBackend::Postgres {
        txn.execute_raw(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT pg_advisory_xact_lock(88421039412)",
        ))
        .await?;
    }

    if let Some(existing) = load_from_db(&txn).await? {
        txn.commit().await?;
        return Ok(existing);
    }

    let db_subject = system_settings::Model::get(&txn, SETTING_SUBJECT)
        .await?
        .and_then(|row| row.value)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_SUBJECT.to_string());

    info!("Gerando novo par de chaves VAPID para notificações Web Push...");
    let new_keys = generate_vapid_key_pair(&db_subject);

    system_settings::Model::set(
        &txn,
        SETTING_PUBLIC_KEY,
        Some(new_keys.public_key_base64.clone()),
    )
    .await?;
    system_settings::Model::set(
        &txn,
        SETTING_PRIVATE_KEY,
        Some(new_keys.private_key_base64.clone()),
    )
    .await?;
    system_settings::Model::set(&txn, SETTING_SUBJECT, Some(new_keys.subject.clone())).await?;
    txn.commit().await?;

    info!("Chaves VAPID geradas e gravadas com sucesso.");
    Ok(new_keys)
}

/// Obtém a chave pública VAPID do sistema para envio ao navegador.
///
/// # Errors
///
/// Propaga erro de banco de dados.
pub async fn get_public_key<C: ConnectionTrait + TransactionTrait>(db: &C) -> AppResult<String> {
    let keys = get_or_create_vapid_keys(db).await?;
    Ok(keys.public_key_base64)
}

async fn load_from_db<C: ConnectionTrait>(db: &C) -> AppResult<Option<VapidKeyPair>> {
    let db_public = system_settings::Model::get(db, SETTING_PUBLIC_KEY)
        .await?
        .and_then(|row| row.value);
    let db_private = system_settings::Model::get(db, SETTING_PRIVATE_KEY)
        .await?
        .and_then(|row| row.value);
    let Some((pub_key, priv_key)) = db_public.zip(db_private) else {
        return Ok(None);
    };
    let db_subject = system_settings::Model::get(db, SETTING_SUBJECT)
        .await?
        .and_then(|row| row.value)
        .unwrap_or_else(|| DEFAULT_SUBJECT.to_string());
    Ok(Some(VapidKeyPair {
        public_key_base64: pub_key,
        private_key_base64: priv_key,
        subject: db_subject,
    }))
}
