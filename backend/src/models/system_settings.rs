use sea_orm::{entity::prelude::*, ActiveValue};

pub use super::_entities::system_settings::{ActiveModel, Column, Entity, Model};

use crate::services::shared::errors::AppResult;

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

    /// Grava (ou sobrescreve) um ajuste e devolve a linha resultante.
    ///
    /// Feito em duas etapas em vez de um `ON CONFLICT`: o upsert nativo diverge
    /// entre SQLite e Postgres na cláusula de conflito, e este caminho é frio —
    /// só o `POST /api/dashboard/layout` passa por aqui.
    ///
    /// # Errors
    ///
    /// Propaga erro do banco.
    pub async fn set<C: ConnectionTrait>(
        db: &C,
        key: &str,
        value: Option<String>,
    ) -> AppResult<Self> {
        if let Some(existing) = Entity::find_by_key(key).one(db).await? {
            let mut active: ActiveModel = existing.into();
            active.value = ActiveValue::Set(value);
            return Ok(active.update(db).await?);
        }

        let active = ActiveModel {
            key: ActiveValue::Set(key.to_string()),
            value: ActiveValue::Set(value),
            ..Default::default()
        };
        Ok(active.insert(db).await?)
    }
}
