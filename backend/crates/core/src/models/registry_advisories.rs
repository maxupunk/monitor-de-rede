use sea_orm::entity::prelude::*;

pub use super::_entities::registry_advisories::{ActiveModel, Column, Entity, Model};

pub type RegistryAdvisories = Entity;

impl ActiveModelBehavior for ActiveModel {}
