use sea_orm::entity::prelude::*;

pub use super::_entities::dns_servers::{ActiveModel, Column, Entity, Model};

pub type DnsServers = Entity;

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
    /// O par `(address, protocol)` é a identidade de um resolvedor: 1.1.1.1 por
    /// UDP e por DoH são dois servidores distintos, com latências distintas.
    ///
    /// Esta consulta é a checagem prévia do 409 do §7.15 — o UNIQUE
    /// `dns_servers_address_protocol_unique` é a rede de segurança, não a
    /// validação.
    pub fn find_by_address(address: &str, protocol: &str) -> Select<Entity> {
        Entity::find()
            .filter(Column::Address.eq(address))
            .filter(Column::Protocol.eq(protocol))
    }

    /// Resolvedores que entram na comparação de latência do dashboard.
    pub fn find_default() -> Select<Entity> {
        Entity::find().filter(Column::IsDefault.eq(true))
    }
}
