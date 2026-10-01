use sea_orm::entity::prelude::*;

pub use super::_entities::plugin_auto_accept::{ActiveModel, Column, Entity, Model};

pub type PluginAutoAccept = Entity;

impl ActiveModelBehavior for ActiveModel {}
