use sea_orm::{entity::prelude::*, sea_query::OnConflict, ActiveValue};

pub use super::_entities::system_settings::{ActiveModel, Column, Entity, Model};

use crate::shared::errors::AppResult;

pub type SystemSettings = Entity;

/// Layout do dashboard, sincronizado entre abas (§7.1).
pub const DASHBOARD_LAYOUT: &str = "dashboard_layout";

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(self, _db: &C, insert: bool) -> std::result::Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if !insert && self.updated_at.is_unchanged() {
            let mut this = self;
            this.updated_at = sea_orm::ActiveValue::Set(chrono::Utc::now().into());
            Ok(this)
        } else {
            Ok(self)
        }
    }
}

impl Entity {
    pub fn find_by_key(key: &str) -> Select<Entity> {
        Entity::find().filter(Column::Key.eq(key))
    }
}

impl Model {
    /// Lê um ajuste. `None` quando a chave nunca foi gravada — diferente de
    /// gravada com valor nulo, que devolve `Some(None)`.
    ///
    /// # Errors
    ///
    /// Propaga erro do banco.
    pub async fn get<C: ConnectionTrait>(db: &C, key: &str) -> AppResult<Option<Self>> {
        Ok(Entity::find_by_key(key).one(db).await?)
    }

    /// Grava (ou sobrescreve) um ajuste via upsert atômico (`ON CONFLICT`),
    /// evitando corrida TOCTOU em gravações concorrentes no SQLite e PostgreSQL.
    ///
    /// # Errors
    ///
    /// Propaga erro do banco.
    pub async fn set<C: ConnectionTrait>(
        db: &C,
        key: &str,
        value: Option<String>,
    ) -> AppResult<Self> {
        let now: DateTimeWithTimeZone = chrono::Utc::now().into();
        let active = ActiveModel {
            key: ActiveValue::Set(key.to_string()),
            value: ActiveValue::Set(value),
            updated_at: ActiveValue::Set(now),
            ..Default::default()
        };
        Entity::insert(active)
            .on_conflict(
                OnConflict::column(Column::Key)
                    .update_columns([Column::Value, Column::UpdatedAt])
                    .to_owned(),
            )
            .exec(db)
            .await?;

        Entity::find_by_key(key).one(db).await?.ok_or_else(|| {
            crate::shared::errors::AppError::Internal(anyhow::anyhow!(
                "system_settings '{key}' ausente após upsert"
            ))
        })
    }
}
