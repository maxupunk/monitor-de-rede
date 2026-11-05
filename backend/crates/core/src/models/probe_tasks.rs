pub use super::_entities::probe_tasks::{ActiveModel, Entity, Model};
use sea_orm::entity::prelude::*;
pub type ProbeTasks = Entity;

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(self, _db: &C, _insert: bool) -> std::result::Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        Ok(self)
    }
}

// implement your read-oriented logic here
impl Model {}

/// A linha da fila vira a tarefa que viaja ao probe (`Command::Monitor`).
impl From<Model> for agent_protocol::probes::ProbeTask {
    fn from(row: Model) -> Self {
        Self {
            id: row.task_id,
            monitor_id: row.monitor_id,
            task_type: row.r#type,
            timeout_ms: row.timeout_ms,
            payload: row.payload,
        }
    }
}

// implement your write-oriented logic here
impl ActiveModel {}

// implement your custom finders, selectors oriented logic here
impl Entity {}
