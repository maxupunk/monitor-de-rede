use sea_orm::{entity::prelude::*, Condition};

pub use super::_entities::devices::{ActiveModel, Column, Entity, Model};

pub type Devices = Entity;

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(self, _db: &C, insert: bool) -> std::result::Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if !insert && self.updated_at.is_unchanged() {
            let mut this = self;
            this.updated_at = sea_orm::ActiveValue::Set(chrono::Utc::now().into());
            Ok(this)
        } else {
            Ok(self)
        }
    }
}

impl Entity {
    /// Resolve o equipamento por IP **ou** nome — é assim que o checker SNMP e o
    /// merger de descoberta procuram um alvo, porque nem todo dispositivo tem
    /// IP fixo e nem todo tem nome resolvível.
    ///
    /// Os dois índices que sustentam esta consulta (`devices_ip_address_index`
    /// e `devices_name_index`) existem por causa dela.
    pub fn find_by_ip_or_name(ip_address: &str, name: &str) -> Select<Entity> {
        Entity::find().filter(
            Condition::any()
                .add(Column::IpAddress.eq(ip_address))
                .add(Column::Name.eq(name)),
        )
    }

    /// Dispositivo de um IP dentro de uma rede — o par que o UNIQUE
    /// `devices_network_ip_unique` protege.
    pub fn find_in_network_by_ip(network_id: i64, ip_address: &str) -> Select<Entity> {
        Entity::find()
            .filter(Column::NetworkId.eq(network_id))
            .filter(Column::IpAddress.eq(ip_address))
    }
}
