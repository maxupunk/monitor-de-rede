//! Histórico de roaming Wi-Fi (`netmonitor_network_wifi::roaming`).

use sea_orm::entity::prelude::*;

pub use super::_entities::wifi_roaming_events::{ActiveModel, Column, Entity, Model};

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {}
