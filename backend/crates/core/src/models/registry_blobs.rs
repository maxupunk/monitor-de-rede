use sea_orm::entity::prelude::*;

pub use super::_entities::registry_blobs::{ActiveModel, Column, Entity, Model};

pub type RegistryBlobs = Entity;

impl ActiveModelBehavior for ActiveModel {}
