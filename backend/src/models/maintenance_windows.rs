use sea_orm::{entity::prelude::*, QueryOrder};

pub use super::_entities::maintenance_windows::{ActiveModel, Column, Entity, Model};

pub type MaintenanceWindows = Entity;

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
    /// Janelas futuras e passadas, ordenadas do mais recente ao mais antigo.
    pub fn find_ordered() -> Select<Entity> {
        Entity::find().order_by_desc(Column::StartsAt)
    }
}
