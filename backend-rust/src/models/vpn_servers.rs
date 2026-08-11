use sea_orm::entity::prelude::*;

pub use super::_entities::vpn_servers::{ActiveModel, Column, Entity, Model};

use crate::services::shared::{crypto, errors::AppResult};

pub type VpnServers = Entity;

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

impl Model {
    /// Chave privada do servidor em texto claro.
    ///
    /// Só sai daqui para montar o `wg0.conf`. O nome do método é `private_key`
    /// e o da coluna é `private_key_encrypted` justamente para que um
    /// `serde(flatten)` distraído não exponha a chave: o DTO teria de citar o
    /// campo cifrado pelo nome, o que salta aos olhos numa revisão.
    ///
    /// # Errors
    ///
    /// Falha se a `APP_KEY` mudou depois da gravação ou o dado foi adulterado.
    pub fn private_key(&self) -> AppResult<String> {
        crypto::decrypt(&self.private_key_encrypted)
    }
}

impl ActiveModel {
    /// Grava a chave privada já cifrada.
    ///
    /// # Errors
    ///
    /// Falha se a cifra não conseguir operar.
    pub fn set_private_key(&mut self, plain: &str) -> AppResult<()> {
        self.private_key_encrypted = sea_orm::ActiveValue::Set(crypto::encrypt(plain)?);
        Ok(())
    }
}

impl Entity {
    /// O servidor em uso. O produto gere **uma** instância do WireGuard; a
    /// tabela aceita mais de uma linha para não travar uma evolução futura,
    /// mas todo o §7.13 fala no singular (`GET /api/vpn/server`).
    pub fn find_active() -> Select<Entity> {
        Entity::find().filter(Column::Active.eq(true))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::ActiveValue;

    #[test]
    fn chave_privada_faz_o_ciclo_completo() {
        let mut active = ActiveModel {
            private_key_encrypted: ActiveValue::NotSet,
            ..Default::default()
        };
        active
            .set_private_key("oJ8kQ2mN4bV6cX9zA1sD3fG5hJ7kL0pQ2wE4rT6yU8i=")
            .unwrap();

        let ActiveValue::Set(cifrada) = active.private_key_encrypted else {
            panic!("a chave não foi gravada");
        };
        assert_ne!(
            cifrada, "oJ8kQ2mN4bV6cX9zA1sD3fG5hJ7kL0pQ2wE4rT6yU8i=",
            "a chave foi para a coluna em texto claro"
        );

        let model = Model {
            private_key_encrypted: cifrada,
            ..modelo_vazio()
        };
        assert_eq!(
            model.private_key().unwrap(),
            "oJ8kQ2mN4bV6cX9zA1sD3fG5hJ7kL0pQ2wE4rT6yU8i="
        );
    }

    #[test]
    fn coluna_corrompida_falha_em_vez_de_devolver_lixo() {
        let model = Model {
            private_key_encrypted: "isto-nao-e-um-criptograma".to_string(),
            ..modelo_vazio()
        };
        assert!(model.private_key().is_err());
    }

    fn modelo_vazio() -> Model {
        let now = chrono::Utc::now();
        Model {
            id: 1,
            network_id: 1,
            interface_name: "wg0".to_string(),
            listen_port: 51820,
            public_endpoint: None,
            public_key: "pub".to_string(),
            private_key_encrypted: String::new(),
            allow_peer_to_peer: false,
            mtu: 1420,
            dns_servers: None,
            active: true,
            last_synced_at: None,
            created_at: now.into(),
            updated_at: now.into(),
        }
    }
}
