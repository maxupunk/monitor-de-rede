//! Plugins nascem desativados.
//!
//! `plugins.auto_enable` marca o plugin que ainda pode ser ligado sozinho —
//! quando um equipamento compatível é cadastrado. Desativar à mão desmarca:
//! o que o operador desligou não volta sem ele.
//!
//! Na atualização, embutido que nenhum equipamento usa passa a desativado (e
//! religável sozinho); o que já está instalado em algum continua ativo.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if !m.has_column("plugins", "auto_enable").await? {
            m.alter_table(
                Table::alter()
                    .table(Alias::new("plugins"))
                    .add_column(boolean("auto_enable").default(false).take())
                    .to_owned(),
            )
            .await?;
        }
        let in_use = Query::select()
            .column(Alias::new("plugin_id"))
            .from(Alias::new("device_plugin_installs"))
            .to_owned();
        m.exec_stmt(
            Query::update()
                .table(Alias::new("plugins"))
                .values([
                    (Alias::new("status"), "disabled".into()),
                    (Alias::new("auto_enable"), true.into()),
                ])
                .and_where(Expr::col(Alias::new("source")).eq("builtin"))
                .and_where(Expr::col(Alias::new("status")).eq("active"))
                .and_where(Expr::col(Alias::new("id")).not_in_subquery(in_use))
                .to_owned(),
        )
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if m.get_connection().get_database_backend() != sea_orm::DatabaseBackend::Sqlite
            && m.has_column("plugins", "auto_enable").await?
        {
            m.alter_table(
                Table::alter()
                    .table(Alias::new("plugins"))
                    .drop_column(Alias::new("auto_enable"))
                    .to_owned(),
            )
            .await?;
        }
        Ok(())
    }
}
