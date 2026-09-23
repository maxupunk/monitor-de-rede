//! Tabela append-only: o rollup de um minuto nunca é editado, só substituído
//! por upsert quando o mesmo minuto chega de novo (reenvio do buffer).

pub use super::_entities::host_metrics_1m::{ActiveModel, Column, Entity, Model};
use sea_orm::entity::prelude::*;
pub type HostMetrics1m = Entity;

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {}
