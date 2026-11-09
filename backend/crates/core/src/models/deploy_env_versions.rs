use sea_orm::entity::prelude::*;

pub use super::_entities::deploy_env_versions::{ActiveModel, Column, Entity, Model};

pub type DeployEnvVersions = Entity;

impl ActiveModelBehavior for ActiveModel {}
