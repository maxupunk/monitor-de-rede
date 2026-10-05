//! Registro de fabricantes do IEEE. Escrito só por
//! `services::vendors::service::refresh`, que o substitui inteiro.

pub use super::_entities::oui_vendors::{ActiveModel, Column, Entity, Model};
use sea_orm::entity::prelude::*;
pub type OuiVendors = Entity;

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {}
