//! Gerenciamento e persistência de chaves VAPID do NetMonitor.

use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};
use tracing::info;

use super::crypto::{generate_vapid_key_pair, VapidKeyPair};
use crate::{models::_entities::system_settings, services::shared::errors::AppResult};

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
pub async fn get_or_create_vapid_keys<C: ConnectionTrait>(db: &C) -> AppResult<VapidKeyPair> {
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

    // 2. Tenta carregar de `system_settings`
    let db_public = get_setting(db, SETTING_PUBLIC_KEY).await?;
    let db_private = get_setting(db, SETTING_PRIVATE_KEY).await?;
    let db_subject = get_setting(db, SETTING_SUBJECT)
        .await?
        .unwrap_or_else(|| DEFAULT_SUBJECT.to_string());

    if let (Some(pub_key), Some(priv_key)) = (db_public, db_private) {
        return Ok(VapidKeyPair {
            public_key_base64: pub_key,
            private_key_base64: priv_key,
            subject: db_subject,
        });
    }

    // 3. Gera novo par e persiste
    info!("Gerando novo par de chaves VAPID para notificações Web Push...");
    let new_keys = generate_vapid_key_pair(&db_subject);

    save_setting(db, SETTING_PUBLIC_KEY, &new_keys.public_key_base64).await?;
    save_setting(db, SETTING_PRIVATE_KEY, &new_keys.private_key_base64).await?;
    save_setting(db, SETTING_SUBJECT, &new_keys.subject).await?;

    info!("Chaves VAPID geradas e gravadas com sucesso.");
    Ok(new_keys)
}

/// Obtém a chave pública VAPID do sistema para envio ao navegador.
///
/// # Errors
///
/// Propaga erro de banco de dados.
pub async fn get_public_key<C: ConnectionTrait>(db: &C) -> AppResult<String> {
    let keys = get_or_create_vapid_keys(db).await?;
    Ok(keys.public_key_base64)
}

async fn get_setting<C: ConnectionTrait>(db: &C, key: &str) -> AppResult<Option<String>> {
    let row = system_settings::Entity::find()
        .filter(system_settings::Column::Key.eq(key))
        .one(db)
        .await?;
    Ok(row.and_then(|r| r.value))
}

async fn save_setting<C: ConnectionTrait>(db: &C, key: &str, value: &str) -> AppResult<()> {
    let existing = system_settings::Entity::find()
        .filter(system_settings::Column::Key.eq(key))
        .one(db)
        .await?;

    let now = chrono::Utc::now().into();
    if let Some(model) = existing {
        let mut active: system_settings::ActiveModel = model.into();
        active.value = Set(Some(value.to_string()));
        active.updated_at = Set(now);
        active.update(db).await?;
    } else {
        system_settings::ActiveModel {
            key: Set(key.to_string()),
            value: Set(Some(value.to_string())),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(db)
        .await?;
    }
    Ok(())
}
