use sea_orm::entity::prelude::*;

pub use super::_entities::push_subscriptions::{ActiveModel, Column, Entity, Model};

pub type PushSubscriptions = Entity;

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {}
