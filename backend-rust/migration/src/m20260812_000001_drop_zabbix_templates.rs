//! Remove o que sobrou do esquema de templates Zabbix.
//!
//! As migrations `#05` e `#06` e a coluna `devices.zabbix_template_id` saíram
//! do repositório: num banco novo essas estruturas nunca chegam a existir. Esta
//! migration existe só para os bancos que já rodaram o esquema antigo — daí
//! todo o `if_exists` e a checagem de coluna antes do `ALTER TABLE`.
//!
//! Sem `down()`: a funcionalidade foi removida do produto, não desativada.
//! Reconstruir as tabelas devolveria um esquema que nenhum código lê.

use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Nome do monitor que o coletor de template criava por dispositivo.
const MONITOR_NAME: &str = "Coleta de Template Zabbix";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        let backend = db.get_database_backend();

        // Os monitores criados por `sync_zabbix_template_monitor` ficariam
        // órfãos: o runner já os recusa como tipo desconhecido e eles só
        // poluiriam a lista de monitores.
        db.execute_raw(Statement::from_string(
            backend,
            format!("DELETE FROM monitors WHERE type = 'zabbix' OR name = '{MONITOR_NAME}'"),
        ))
        .await?;

        if m.has_column("devices", "zabbix_template_id").await? {
            m.drop_index(
                Index::drop()
                    .name("devices_zabbix_template_id_index")
                    .table(Alias::new("devices"))
                    .to_owned(),
            )
            .await
            // O índice pode já não existir num banco reconstruído à mão; a
            // falha aqui não impede o resto da limpeza.
            .ok();

            // O SQLite recusa `DROP COLUMN` em coluna que participa de FK, e
            // reconstruir `devices` inteira não se justifica: SQLite é só
            // dev/teste (§13) e ali o banco é descartável. A coluna sobra
            // inerte — nenhuma entidade a mapeia mais.
            if backend != DatabaseBackend::Sqlite {
                m.alter_table(
                    Table::alter()
                        .table(Alias::new("devices"))
                        .drop_column(Alias::new("zabbix_template_id"))
                        .to_owned(),
                )
                .await?;
            }
        }

        m.drop_table(drop("zabbix_template_items")).await?;
        m.drop_table(drop("zabbix_templates")).await?;
        Ok(())
    }

    async fn down(&self, _m: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
