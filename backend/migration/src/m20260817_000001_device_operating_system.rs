//! O sistema do equipamento, escolhido de um catálogo único.
//!
//! Não substitui `devices.vendor`: aquele é texto livre e costuma vir do OUI do
//! MAC, identificando **quem fabricou a placa**. Este diz qual sistema roda nela
//! — que é o que decide os comandos de syslog, o meio de acesso e o perfil de
//! VPN. Um CCR da MikroTik pode rodar RouterOS ou SwOS, e o fabricante não
//! separa os dois.
//!
//! Anulável, e `NULL` significa "automático": a dedução por `sysDescr` do SNMP
//! continua valendo e é recalculada a cada leitura. Ver
//! `services::devices::systems`.
//!
//! # O backfill
//!
//! Dispositivo criado pelo assistente da VPN **já declarou** o sistema — é o
//! perfil que o operador escolheu para gerar a configuração. Deixá-lo em branco
//! faria a tela perguntar de novo o que já estava respondido no banco. A única
//! tradução necessária é `mikrotik` → `routeros`: lá o nome é o do gerador de
//! configuração, aqui é o do sistema.

use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if !m.has_column("devices", "operating_system").await? {
            m.alter_table(
                Table::alter()
                    .table(Alias::new("devices"))
                    .add_column(string_null("operating_system"))
                    .to_owned(),
            )
            .await?;
        }

        // Subconsulta correlacionada em vez de `UPDATE ... FROM`: a segunda
        // forma diverge entre SQLite e PostgreSQL, e esta roda igual nos dois.
        let db = m.get_connection();
        db.execute_raw(Statement::from_string(
            db.get_database_backend(),
            "UPDATE devices \
             SET operating_system = (\
                SELECT CASE vpn_peers.device_profile \
                    WHEN 'mikrotik' THEN 'routeros' \
                    ELSE vpn_peers.device_profile END \
                FROM vpn_peers WHERE vpn_peers.device_id = devices.id\
             ) \
             WHERE operating_system IS NULL \
               AND EXISTS (SELECT 1 FROM vpn_peers WHERE vpn_peers.device_id = devices.id)"
                .to_string(),
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        // Mesmo motivo das migrations anteriores de coluna: o SQLite de
        // produção suporta ADD COLUMN, mas não um DROP COLUMN compatível com
        // todas as versões que ainda atendemos.
        if m.get_connection().get_database_backend() != sea_orm::DatabaseBackend::Sqlite
            && m.has_column("devices", "operating_system").await?
        {
            m.alter_table(
                Table::alter()
                    .table(Alias::new("devices"))
                    .drop_column(Alias::new("operating_system"))
                    .to_owned(),
            )
            .await?;
        }
        Ok(())
    }
}
