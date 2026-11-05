//! §6 #08 — `device_interfaces`. Portas/interfaces descobertas por SNMP.
//!
//! Os contadores SNMP (`in_octets`, `out_octets`, `in_errors`, `out_errors` e
//! o instante da leitura, `counters_at`) moram na linha da interface, não no
//! histórico: são acumulados desde o boot do equipamento, e o que importa é a
//! **última** leitura, base da taxa seguinte. Gravá-los em `metrics` a cada
//! coleta fazia dois terços daquela tabela sem tela nenhuma que os lesse. Ficam
//! depois de `created_at`/`updated_at` porque chegaram depois.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let mut stmt = table("device_interfaces");
        stmt.col(big_pk_auto("id"))
            .col(big_integer("device_id"))
            .col(integer_null("snmp_index"))
            .col(string("name"))
            .col(string_null("description"))
            .col(string_null("alias"))
            .col(string_null("mac_address"))
            .col(string_null("type"))
            // `bigint`: 100 Gbps em bits por segundo já estoura o i32.
            .col(big_integer_null("speed"))
            .col(string_null("admin_status"))
            .col(string_null("oper_status"))
            .col(timestamp_with_time_zone_null("last_seen_at"))
            .foreign_key(&mut fk(
                "device_interfaces",
                "device_id",
                "devices",
                ForeignKeyAction::Cascade,
            ));

        let mut stmt = with_timestamps(stmt.take());
        stmt.col(big_integer_null("in_octets"))
            .col(big_integer_null("out_octets"))
            .col(big_integer_null("in_errors"))
            .col(big_integer_null("out_errors"))
            .col(timestamp_with_time_zone_null("counters_at"));

        m.create_table(stmt).await?;

        // Uma consulta por interface a cada coleta SNMP — o laço mais quente do
        // módulo. Não é UNIQUE porque `snmp_index` é opcional em interfaces
        // criadas manualmente.
        m.create_index(index(
            "device_interfaces_device_id_snmp_index_index",
            "device_interfaces",
            &["device_id", "snmp_index"],
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("device_interfaces")).await?;
        Ok(())
    }
}
