use sea_orm::{entity::prelude::*, QueryOrder};

pub use super::_entities::audit_logs::{ActiveModel, Column, Entity, Model};

pub type AuditLogs = Entity;

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {}

impl Entity {
    /// Logs mais recentes primeiro.
    pub fn find_ordered() -> Select<Entity> {
        Entity::find().order_by_desc(Column::CreatedAt)
    }
}
