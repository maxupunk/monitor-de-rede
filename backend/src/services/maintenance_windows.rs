//! Janelas de manutenção (Fase 3 do roadmap).
//!
//! Um operador agenda intervalos em que alertas de um site ou dispositivo não
//! devem incomodar a equipe. O alerta ainda é criado — ele alimenta o histórico
//! e pode ser consultado —, mas a notificação é suprimida enquanto a janela
//! vigorar. A hierarquia é respeitada: uma janela no site cobre todos os
//! dispositivos daquele site; uma janela no dispositivo cobre só ele.

use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, EntityTrait, PaginatorTrait,
    QueryFilter, Set,
};

use crate::{
    models::{devices, maintenance_windows},
    services::shared::errors::{AppError, AppResult},
};

/// Verifica se há janela de manutenção ativa para o alvo no instante dado.
///
/// Se `device_id` for informado, a função busca o site do dispositivo e
/// considera tanto janelas daquele device quanto janelas do site dele. Se
/// apenas `site_id` for informado, considera só janelas do site. Sem nenhum
/// dos dois, nunca há manutenção.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn is_under_maintenance<C>(
    db: &C,
    site_id: Option<i64>,
    device_id: Option<i64>,
    now: DateTime<Utc>,
) -> AppResult<bool>
where
    C: ConnectionTrait,
{
    let effective_site_id = match device_id {
        Some(id) => {
            let device = devices::Entity::find_by_id(id).one(db).await?;
            device.and_then(|d| d.site_id).or(site_id)
        }
        None => site_id,
    };

    let mut condition = Condition::all()
        .add(maintenance_windows::Column::StartsAt.lte(now))
        .add(maintenance_windows::Column::EndsAt.gte(now));

    if let Some(device_id) = device_id {
        let by_device = Condition::all()
            .add(maintenance_windows::Column::DeviceId.eq(device_id))
            .add(maintenance_windows::Column::SiteId.is_null());

        if let Some(site_id) = effective_site_id {
            let by_site = Condition::all()
                .add(maintenance_windows::Column::SiteId.eq(site_id))
                .add(maintenance_windows::Column::DeviceId.is_null());
            condition = condition.add(Condition::any().add(by_device).add(by_site));
        } else {
            condition = condition.add(by_device);
        }
    } else if let Some(site_id) = effective_site_id {
        condition = condition
            .add(maintenance_windows::Column::SiteId.eq(site_id))
            .add(maintenance_windows::Column::DeviceId.is_null());
    } else {
        return Ok(false);
    }

    let count = maintenance_windows::Entity::find()
        .filter(condition)
        .count(db)
        .await?;

    Ok(count > 0)
}

/// Lista todas as janelas, da mais recente à mais antiga.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn list<C>(db: &C) -> AppResult<Vec<maintenance_windows::Model>>
where
    C: ConnectionTrait,
{
    Ok(maintenance_windows::Entity::find_ordered().all(db).await?)
}

/// Payload de criação/edição de uma janela.
#[derive(Debug, Clone)]
pub struct MaintenanceWindowInput {
    pub site_id: Option<i64>,
    pub device_id: Option<i64>,
    pub name: String,
    pub description: Option<String>,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
}

/// Cria uma nova janela de manutenção.
///
/// # Errors
///
/// Retorna erro de validação quando o intervalo é inválido ou quando a janela
/// não está vinculada a site nem dispositivo. Propaga erro do banco.
pub async fn create<C>(
    db: &C,
    input: MaintenanceWindowInput,
    created_by: Option<i64>,
) -> AppResult<maintenance_windows::Model>
where
    C: ConnectionTrait,
{
    validate(&input)?;
    ensure_target_exists(db, input.site_id, input.device_id).await?;

    let row = maintenance_windows::ActiveModel {
        site_id: Set(input.site_id),
        device_id: Set(input.device_id),
        name: Set(input.name.trim().to_string()),
        description: Set(input.description.map(|value| value.trim().to_string())),
        starts_at: Set(input.starts_at.into()),
        ends_at: Set(input.ends_at.into()),
        created_by: Set(created_by),
        ..Default::default()
    }
    .insert(db)
    .await?;

    Ok(row)
}

/// Atualiza uma janela existente.
///
/// # Errors
///
/// Retorna 404 se a janela não existe, ou erro de validação quando o intervalo
/// é inválido. Propaga erro do banco.
pub async fn update<C>(
    db: &C,
    id: i64,
    input: MaintenanceWindowInput,
) -> AppResult<maintenance_windows::Model>
where
    C: ConnectionTrait,
{
    validate(&input)?;
    ensure_target_exists(db, input.site_id, input.device_id).await?;

    let row = maintenance_windows::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::not_found("Janela de manutenção não encontrada"))?;

    let updated = maintenance_windows::ActiveModel {
        id: Set(row.id),
        site_id: Set(input.site_id),
        device_id: Set(input.device_id),
        name: Set(input.name.trim().to_string()),
        description: Set(input.description.map(|value| value.trim().to_string())),
        starts_at: Set(input.starts_at.into()),
        ends_at: Set(input.ends_at.into()),
        ..Default::default()
    }
    .update(db)
    .await?;

    Ok(updated)
}

/// Remove uma janela de manutenção.
///
/// # Errors
///
/// Retorna 404 se a janela não existe. Propaga erro do banco.
pub async fn delete<C>(db: &C, id: i64) -> AppResult<()>
where
    C: ConnectionTrait,
{
    let row = maintenance_windows::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::not_found("Janela de manutenção não encontrada"))?;

    maintenance_windows::Entity::delete_by_id(row.id)
        .exec(db)
        .await?;
    Ok(())
}

fn validate(input: &MaintenanceWindowInput) -> AppResult<()> {
    if input.name.trim().is_empty() {
        return Err(AppError::validation("Nome da janela é obrigatório"));
    }
    if input.site_id.is_none() && input.device_id.is_none() {
        return Err(AppError::validation(
            "A janela deve estar vinculada a um site ou a um dispositivo",
        ));
    }
    if input.ends_at <= input.starts_at {
        return Err(AppError::validation(
            "O horário de término deve ser posterior ao de início",
        ));
    }
    Ok(())
}

async fn ensure_target_exists<C>(
    db: &C,
    site_id: Option<i64>,
    device_id: Option<i64>,
) -> AppResult<()>
where
    C: ConnectionTrait,
{
    if let Some(site_id) = site_id {
        let exists = crate::models::sites::Entity::find_by_id(site_id)
            .one(db)
            .await?
            .is_some();
        if !exists {
            return Err(AppError::validation("Site informado não existe"));
        }
    }
    if let Some(device_id) = device_id {
        let exists = devices::Entity::find_by_id(device_id)
            .one(db)
            .await?
            .is_some();
        if !exists {
            return Err(AppError::validation("Dispositivo informado não existe"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn recusa_nome_vazio() {
        let input = MaintenanceWindowInput {
            site_id: Some(1),
            device_id: None,
            name: "   ".into(),
            description: None,
            starts_at: Utc::now(),
            ends_at: Utc::now() + Duration::hours(1),
        };
        assert!(validate(&input).is_err());
    }

    #[test]
    fn recusa_janela_sem_alvo() {
        let input = MaintenanceWindowInput {
            site_id: None,
            device_id: None,
            name: "Manutenção".into(),
            description: None,
            starts_at: Utc::now(),
            ends_at: Utc::now() + Duration::hours(1),
        };
        assert!(validate(&input).is_err());
    }

    #[test]
    fn recusa_termino_antes_do_inicio() {
        let input = MaintenanceWindowInput {
            site_id: Some(1),
            device_id: None,
            name: "Manutenção".into(),
            description: None,
            starts_at: Utc::now(),
            ends_at: Utc::now() - Duration::hours(1),
        };
        assert!(validate(&input).is_err());
    }

    #[test]
    fn aceita_janela_valida() {
        let input = MaintenanceWindowInput {
            site_id: Some(1),
            device_id: None,
            name: "Manutenção".into(),
            description: None,
            starts_at: Utc::now(),
            ends_at: Utc::now() + Duration::hours(1),
        };
        assert!(validate(&input).is_ok());
    }
}
