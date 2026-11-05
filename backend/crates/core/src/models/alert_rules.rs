use sea_orm::{entity::prelude::*, QueryOrder};

pub use super::_entities::alert_rules::{ActiveModel, Column, Entity, Model};

pub type AlertRules = Entity;

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

impl Model {
    /// §6.1 — `isEnabled`: espelho de `enabled`, como no `Monitor`.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Entity {
    /// `GET /api/alert-rules` — array ordenado por id (§7.11).
    pub fn find_ordered() -> Select<Entity> {
        Entity::find().order_by_asc(Column::Id)
    }

    /// Regra já derivada de um item do catálogo.
    ///
    /// É a consulta de idempotência do `POST /api/alert-rules/catalog`: uma
    /// chave de template só produz regra uma vez. Desenhada para o índice
    /// `alert_rules_template_key_index`.
    pub fn find_by_template_key(template_key: &str) -> Select<Entity> {
        Entity::find().filter(Column::TemplateKey.eq(template_key))
    }
}
