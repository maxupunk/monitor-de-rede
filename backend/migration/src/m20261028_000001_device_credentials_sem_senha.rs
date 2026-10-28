//! Senha de equipamento não fica mais no banco (ADR 016).
//!
//! Sobram dois modos: o acesso dedicado por chave (`key`) e a senha pedida na
//! janela, que vive só em memória (`ask`). As credenciais `vault` perdem a
//! senha cifrada e passam a pedi-la — usuário, porta e agente ficam.

use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        db.execute_raw(Statement::from_string(
            db.get_database_backend(),
            "UPDATE device_credentials SET storage = 'ask', secret_encrypted = NULL \
             WHERE storage <> 'key'"
                .to_string(),
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, _m: &SchemaManager) -> Result<(), DbErr> {
        // A senha apagada não volta.
        Ok(())
    }
}
