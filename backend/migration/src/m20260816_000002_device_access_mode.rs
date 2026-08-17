//! Por onde o operador alcança cada equipamento.
//!
//! A coluna guarda **a declaração**, não a dedução: `NULL` significa
//! "automático", e é o valor de todo dispositivo que já existia. O sistema sabe
//! deduzir (peer da VPN, IP dentro da faixa do túnel, endereço privado ou
//! global), e a dedução é recalculada a cada leitura — gravá-la aqui a
//! congelaria, e um equipamento que sai da VPN continuaria marcado como VPN
//! para sempre.
//!
//! Sem backfill, então. O que se ganha com a coluna é o caso em que a dedução
//! não tem como acertar: o roteador de uma filial acessado por IP público
//! estático, que o sistema classificaria como "remoto" corretamente, e o
//! roteador de uma filial atrás de outra VPN, que ele não tem como distinguir
//! de um vizinho de LAN. Ali o operador declara, e a declaração vence.
//!
//! Valores aceitos: `local`, `vpn`, `remote`. A validação mora no controller —
//! um CHECK constraint em SQLite não é alterável depois sem recriar a tabela,
//! e este é um vocabulário que ainda pode crescer.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if !m.has_column("devices", "access_mode").await? {
            m.alter_table(
                Table::alter()
                    .table(Alias::new("devices"))
                    .add_column(string_null("access_mode"))
                    .to_owned(),
            )
            .await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        // Mesmo motivo das migrations anteriores de coluna: o SQLite de
        // produção suporta ADD COLUMN, mas não um DROP COLUMN compatível com
        // todas as versões que ainda atendemos.
        if m.get_connection().get_database_backend() != sea_orm::DatabaseBackend::Sqlite
            && m.has_column("devices", "access_mode").await?
        {
            m.alter_table(
                Table::alter()
                    .table(Alias::new("devices"))
                    .drop_column(Alias::new("access_mode"))
                    .to_owned(),
            )
            .await?;
        }
        Ok(())
    }
}
