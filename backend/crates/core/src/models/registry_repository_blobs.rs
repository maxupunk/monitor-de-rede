use sea_orm::entity::prelude::*;

pub use super::_entities::registry_repository_blobs::{ActiveModel, Column, Entity, Model};

pub type RegistryRepositoryBlobs = Entity;

impl ActiveModelBehavior for ActiveModel {}
