use sea_orm::ConnectionTrait;

use super::{
    catalog::builtin_profiles,
    model::{SnmpDeviceProfile, SnmpProfileSummary},
};
use crate::{
    models::system_settings,
    services::shared::errors::{AppError, AppResult},
};

pub const PROFILES_STORAGE_KEY: &str = "snmp_custom_profiles";

/// Carrega todos os perfis (built-in mesclados com os customizados do banco).
/// Perfis customizados sobrescrevem perfis built-in com o mesmo ID.
pub async fn load_all_profiles<C: ConnectionTrait>(db: &C) -> AppResult<Vec<SnmpDeviceProfile>> {
    let mut profiles = builtin_profiles();
    if let Some(custom) = load_custom_profiles(db).await? {
        for cp in custom {
            if let Some(pos) = profiles.iter().position(|p| p.id == cp.id) {
                profiles[pos] = cp;
            } else {
                profiles.push(cp);
            }
        }
    }
    Ok(profiles)
}

/// Carrega apenas os perfis customizados gravados em `system_settings`.
pub async fn load_custom_profiles<C: ConnectionTrait>(
    db: &C,
) -> AppResult<Option<Vec<SnmpDeviceProfile>>> {
    let row = system_settings::Model::get(db, PROFILES_STORAGE_KEY).await?;
    let Some(text) = row.and_then(|r| r.value) else {
        return Ok(None);
    };
    if text.trim().is_empty() {
        return Ok(None);
    }
    let list: Vec<SnmpDeviceProfile> = serde_json::from_str(&text).map_err(|err| {
        AppError::Internal(anyhow::anyhow!("Falha ao desserializar perfis SNMP: {err}"))
    })?;
    Ok(Some(list))
}

/// Salva ou atualiza um perfil customizado no banco de dados.
pub async fn save_custom_profile<C: ConnectionTrait>(
    db: &C,
    mut profile: SnmpDeviceProfile,
) -> AppResult<SnmpDeviceProfile> {
    profile.id = profile.id.trim().to_lowercase();
    if profile.id.is_empty() {
        return Err(AppError::validation("ID do perfil não pode ser vazio"));
    }
    if profile.name.trim().is_empty() {
        return Err(AppError::validation("Nome do perfil é obrigatório"));
    }
    profile.is_builtin = false;

    let mut current = load_custom_profiles(db).await?.unwrap_or_default();
    if let Some(pos) = current.iter().position(|p| p.id == profile.id) {
        current[pos] = profile.clone();
    } else {
        current.push(profile.clone());
    }

    let serialized = serde_json::to_string(&current).map_err(|err| {
        AppError::Internal(anyhow::anyhow!("Falha ao serializar perfis SNMP: {err}"))
    })?;
    system_settings::Model::set(db, PROFILES_STORAGE_KEY, Some(serialized)).await?;
    Ok(profile)
}

/// Exclui um perfil customizado do banco de dados.
pub async fn delete_custom_profile<C: ConnectionTrait>(db: &C, profile_id: &str) -> AppResult<()> {
    let id_clean = profile_id.trim().to_lowercase();
    // Não permite apagar perfil built-in se ele não tiver override customizado
    let mut current = load_custom_profiles(db).await?.unwrap_or_default();
    let initial_len = current.len();
    current.retain(|p| p.id != id_clean);
    if current.len() == initial_len && builtin_profiles().iter().any(|p| p.id == id_clean) {
        return Err(AppError::validation(
            "Perfis nativos (built-in) do sistema não podem ser excluídos.",
        ));
    }

    let serialized = serde_json::to_string(&current).map_err(|err| {
        AppError::Internal(anyhow::anyhow!("Falha ao serializar perfis SNMP: {err}"))
    })?;
    system_settings::Model::set(db, PROFILES_STORAGE_KEY, Some(serialized)).await?;
    Ok(())
}

/// Localiza o primeiro perfil que coincida com a identidade do equipamento.
#[must_use]
pub fn match_profile<'a>(
    profiles: &'a [SnmpDeviceProfile],
    sys_object_id: Option<&str>,
    sys_descr: Option<&str>,
) -> Option<&'a SnmpDeviceProfile> {
    for profile in profiles {
        let has_prefix = profile.sys_object_id_prefix.is_some();
        let has_pattern = profile.sys_descr_pattern.is_some();
        if !has_prefix && !has_pattern {
            continue;
        }

        let matches_prefix = match (&profile.sys_object_id_prefix, sys_object_id) {
            (Some(prefix), Some(oid)) => {
                let clean_prefix = prefix.trim_start_matches('.');
                let clean_oid = oid.trim_start_matches('.');
                clean_oid.starts_with(clean_prefix)
            }
            (Some(_), None) => false,
            (None, _) => true,
        };

        let matches_pattern = match (&profile.sys_descr_pattern, sys_descr) {
            (Some(pattern), Some(descr)) => {
                let clean_pattern = pattern.replace("(?i)", "");
                descr
                    .to_lowercase()
                    .contains(&clean_pattern.trim().to_lowercase())
            }
            (Some(_), None) => false,
            (None, _) => true,
        };

        if matches_prefix && matches_pattern {
            return Some(profile);
        }
    }
    None
}

impl From<&SnmpDeviceProfile> for SnmpProfileSummary {
    fn from(profile: &SnmpDeviceProfile) -> Self {
        Self {
            id: profile.id.clone(),
            name: profile.name.clone(),
            vendor: profile.vendor.clone(),
            category: profile.category.clone(),
            is_builtin: profile.is_builtin,
        }
    }
}
