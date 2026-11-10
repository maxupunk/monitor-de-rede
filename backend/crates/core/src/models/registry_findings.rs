use sea_orm::entity::prelude::*;

pub use super::_entities::registry_findings::{ActiveModel, Column, Entity, Model};

pub type RegistryFindings = Entity;

impl ActiveModelBehavior for ActiveModel {}
