use sea_orm::entity::prelude::*;

pub use super::_entities::probes::{ActiveModel, Column, Entity, Model};

use crate::services::shared::crypto;

pub type Probes = Entity;

/// Status de um probe revogado — excluído de toda autenticação.
pub const STATUS_REVOKED: &str = "revoked";

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
    /// Autenticação de probe (§7.10): compara `sha256(token)` com `token_hash`.
    ///
    /// Duas coisas que **não** são detalhe:
    ///
    /// 1. o resultado pode ter mais de uma linha — `token_hash` não é único,
    ///    porque o `DEFAULT_VPN_PROBE_TOKEN` é compartilhado entre agentes
    ///    zero-config. Quem chama precisa lidar com a lista, não com `.one()`;
    /// 2. probes revogados ficam de fora aqui, e não num `if` do chamador. Um
    ///    ponto só de exclusão é o que garante que `heartbeat`, `tasks` e
    ///    `results` concordem sobre quem está revogado.
    ///
    /// Desenhada para o índice `probes_token_hash_index`.
    pub fn find_by_token(raw_token: &str) -> Select<Entity> {
        Entity::find()
            .filter(Column::TokenHash.eq(crypto::sha256_hex(raw_token)))
            .filter(Column::Status.ne(STATUS_REVOKED))
    }
}
