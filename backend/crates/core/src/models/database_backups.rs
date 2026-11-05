use sea_orm::entity::prelude::*;

pub use super::_entities::database_backups::{ActiveModel, Column, Entity, Model};

pub type DatabaseBackups = Entity;

impl ActiveModelBehavior for ActiveModel {}
