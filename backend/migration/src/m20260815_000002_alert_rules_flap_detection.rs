//! Fase 3 do roadmap de alertas inteligentes — detecção de flapping.
//!
//! Dois parâmetros de regra (§3.4: nada de configuração paralela) descrevem
//! quando um alvo deixa de ser "um problema" e passa a ser "cronicamente
//! instável":
//!
//! - `flap_threshold`: quantas recaídas dentro da janela declaram o alvo
//!   oscilando. `0` desliga a detecção — é o default, para que instalações
//!   existentes não mudem de comportamento sozinhas.
//! - `flap_window_seconds`: a largura da janela deslizante. O default de 900 s
//!   dá um valor sensato já pronto para quem só ligar o limiar depois.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if !m.has_column("alert_rules", "flap_threshold").await? {
            m.alter_table(
                Table::alter()
                    .table(Alias::new("alert_rules"))
                    .add_column(integer("flap_threshold").not_null().default(0))
                    .to_owned(),
            )
            .await?;
        }
        if !m.has_column("alert_rules", "flap_window_seconds").await? {
            m.alter_table(
                Table::alter()
                    .table(Alias::new("alert_rules"))
                    .add_column(integer("flap_window_seconds").not_null().default(900))
                    .to_owned(),
            )
            .await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        // Mesmo motivo do m20260815_000001: o SQLite de produção suporta ADD
        // COLUMN, mas não um DROP COLUMN compatível com todas as versões que
        // ainda atendemos.
        if m.get_connection().get_database_backend() == sea_orm::DatabaseBackend::Sqlite {
            return Ok(());
        }
        for column in ["flap_window_seconds", "flap_threshold"] {
            if m.has_column("alert_rules", column).await? {
                m.alter_table(
                    Table::alter()
                        .table(Alias::new("alert_rules"))
                        .drop_column(Alias::new(column))
                        .to_owned(),
                )
                .await?;
            }
        }
        Ok(())
    }
}
