//! §6 #07 — `devices`. Núcleo do inventário.
//!
//! As colunas depois de `created_at`/`updated_at` vieram com as funções que as
//! usam; ficam no fim para o esquema não mudar de forma:
//!
//! * `snmp_poll_interval_seconds` — a cadência de coleta SNMP do equipamento,
//!   compartilhada por todos os monitores SNMP dele.
//! * `access_mode` — por onde o operador alcança o equipamento (`local`, `vpn`,
//!   `remote`). `NULL` é "automático": a dedução (peer da VPN, faixa do túnel,
//!   endereço privado ou global) é recalculada a cada leitura, e gravá-la a
//!   congelaria. A validação mora no controller, não num CHECK: o vocabulário
//!   ainda pode crescer.
//! * `operating_system` — o sistema declarado, de um catálogo único. Não é o
//!   `vendor` (texto livre, em geral do OUI do MAC): um CCR da MikroTik roda
//!   RouterOS ou SwOS. `NULL` é "automático" (dedução por `sysDescr`, ver
//!   `services::devices::systems`).
//! * `system_key` — a chave técnica do dispositivo **do próprio sistema**
//!   (`netmonitor`), que não pode depender de ID, nome ou IP. Anulável, com
//!   índice único à parte: `NULL`s são distintos nos dois bancos, então só as
//!   linhas que declaram uma chave são restringidas.
//! * `syslog_server_address` — o endereço de Syslog realmente gravado neste
//!   equipamento (o catálogo global só diz quais o servidor oferece).
//! * `link_interface_id`/`link_interface_name` — a interface de entrada do
//!   link (WAN/uplink), para correlacionar banda com tempo de resposta.
//! * `firmware_version` — versão lida pelo `detect` de um plugin.
//! * `observed_os`, `observed_os_source`, `observed_os_reason` — o sistema que
//!   o equipamento **mostrou** ser (servidor SSH, SNMP, um plugin que leu
//!   `/etc/openwrt_release`) ou, sem evidência, o palpite do Decisor. É o que
//!   decide a compatibilidade de um plugin quando ninguém declarou.
//!
//! **`probes.device_id` nasce aqui**, e não na migration de `probes`: `probes`
//! é criada antes de `devices` e o PostgreSQL não aceita FK para uma tabela
//! que ainda não existe. O `ADD COLUMN ... REFERENCES` vale igual no SQLite e
//! no PostgreSQL — o SQLite só exige default nulo, e é.

use sea_orm::{ConnectionTrait, Statement};
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
            ));

        let mut stmt = with_timestamps(stmt.take());
        stmt.col(integer("snmp_poll_interval_seconds").not_null().default(15))
            .col(string_null("access_mode"))
            .col(string_null("operating_system"))
            .col(string_null("system_key"))
            .col(string_null("syslog_server_address"))
            .col(big_integer_null("link_interface_id"))
            .col(string_null("link_interface_name"))
            .col(string_len_null("firmware_version", 64))
            .col(string_null("observed_os"))
            .col(string_null("observed_os_source"))
            .col(text_null("observed_os_reason"));

        m.create_table(stmt).await?;

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
        m.create_index(unique(
            "devices_system_key_unique",
            "devices",
            &["system_key"],
        ))
        .await?;

        // O agente instalado num servidor da VPN é o mesmo equipamento que o
        // peer criou em `devices` (ADR 011). `SET NULL`: apagar o dispositivo
        // só desfaz o vínculo — o agente continua sendo um probe válido.
        let db = m.get_connection();
        db.execute_raw(Statement::from_string(
            db.get_database_backend(),
            "ALTER TABLE probes ADD COLUMN device_id BIGINT NULL \
             REFERENCES devices (id) ON DELETE SET NULL ON UPDATE CASCADE"
                .to_string(),
        ))
        .await?;
        m.create_index(index("probes_device_id_index", "probes", &["device_id"]))
            .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("devices")).await?;
        Ok(())
    }
}
