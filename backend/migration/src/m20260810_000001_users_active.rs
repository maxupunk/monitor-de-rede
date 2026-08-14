//! Estende a tabela `users` do scaffold Loco com a coluna `active`.
//!
//! A `users` do Loco (pid, api_key, tokens de verificação/reset/magic-link) é
//! bem mais rica que a do esquema anterior, então é ela que fica: estender, não
//! recriar. O único campo que faltava é o `active`, usado para desligar um
//! login sem apagar o histórico que aponta para o usuário.
//!
//! Migration separada em vez de editar `m20220101_000001_users.rs`: mexer numa
//! migration já aplicada deixa bancos existentes fora de sincronia sem que nada
//! reclame.

use loco_rs::schema::{add_column, remove_column, ColType};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        add_column(m, "users", "active", ColType::BooleanWithDefault(true)).await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        remove_column(m, "users", "active").await?;
        Ok(())
    }
}
