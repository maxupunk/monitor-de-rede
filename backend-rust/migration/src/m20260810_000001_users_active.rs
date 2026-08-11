//! §6 #01 — estende a tabela `users` do scaffold Loco com `active`.
//!
//! A `users` do Loco (pid, api_key, tokens de verificação/reset/magic-link) é
//! bem mais rica que a do AdonisJS e é ela que fica — a §6 manda estendê-la, e
//! não recriá-la. O único campo que falta é o `active` do backend atual, usado
//! para desligar um login sem apagar o histórico que aponta para o usuário.
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
