//! §6 #07 — `devices`. Núcleo do inventário.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let mut stmt = table("devices");
        stmt.col(big_pk_auto("id"))
            .col(big_integer_null("site_id"))
            .col(big_integer_null("network_id"))
            .col(big_integer_null("parent_id"))
            .col(big_integer_null("zabbix_template_id"))
            .col(string_null("ip_address"))
            .col(string("name"))
            .col(string("type"))
            .col(string_null("vendor"))
            .col(string_null("model"))
            .col(string_null("serial_number"))
            .col(text_null("description"))
            .col(boolean("is_monitored").default(false).take())
            .col(boolean("snmp_enabled").default(false).take())
            .col(string_null("snmp_community"))
            .col(string_null("snmp_version"))
            .col(string("status").default("unknown").take())
            .col(timestamp_with_time_zone_null("last_seen_at"))
            .foreign_key(&mut fk(
                "devices",
                "site_id",
                "sites",
                ForeignKeyAction::Cascade,
            ))
            .foreign_key(&mut fk(
                "devices",
                "network_id",
                "networks",
                ForeignKeyAction::SetNull,
            ))
            // Auto-referência: a hierarquia pai/filho da topologia. `SET NULL`
            // porque apagar o pai não pode apagar a subárvore inteira.
            .foreign_key(&mut fk(
                "devices",
                "parent_id",
                "devices",
                ForeignKeyAction::SetNull,
            ))
            .foreign_key(&mut fk(
                "devices",
                "zabbix_template_id",
                "zabbix_templates",
                ForeignKeyAction::SetNull,
            ));

        m.create_table(with_timestamps(stmt.take())).await?;

        // Integridade do IPAM: impede dois dispositivos com o mesmo IP na mesma
        // rede. NULLs são distintos tanto no PostgreSQL quanto no SQLite, então
        // dispositivos sem IP definido não colidem entre si.
        m.create_index(unique(
            "devices_network_ip_unique",
            "devices",
            &["network_id", "ip_address"],
        ))
        .await?;

        // O checker SNMP resolve o equipamento por `ip_address = ? OR name = ?`.
        // O UNIQUE acima já cobre buscas por `network_id`, mas não por IP isolado.
        m.create_index(index(
            "devices_ip_address_index",
            "devices",
            &["ip_address"],
        ))
        .await?;
        m.create_index(index("devices_name_index", "devices", &["name"]))
            .await?;
        m.create_index(index("devices_site_id_index", "devices", &["site_id"]))
            .await?;
        m.create_index(index(
            "devices_zabbix_template_id_index",
            "devices",
            &["zabbix_template_id"],
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("devices")).await?;
        Ok(())
    }
}
