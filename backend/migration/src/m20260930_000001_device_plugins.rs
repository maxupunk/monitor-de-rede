//! Plugins de dispositivo: scripts Rhai que agem sobre o equipamento.
//!
//! * `plugins` — o pacote (manifesto, script, uso, compatibilidade, testes e
//!   o relatório da revisão de segurança). `scope = 'model'` serve a todo
//!   equipamento compatível; `scope = 'device'` só ao `device_id`.
//! * `device_credentials` — acesso SSH/HTTP/Telnet. O segredo é cifrado
//!   (`storage = 'vault'`) ou nem é gravado (`storage = 'ask'`, pedido a cada
//!   sessão e mantido só em memória).
//! * `plugin_runs` — cada execução, com o transcript já sem segredos. É a
//!   auditoria da aba e a origem das entradas de compatibilidade.
//! * `plugin_auto_accept` — o aceite do termo do modo automático da IA, por
//!   conversa × dispositivo, com validade.
//! * `devices.firmware_version` — versão lida pelo `detect` de um plugin.
//!
//! A unicidade de `(slug, version)` entre plugins de modelo é regra de serviço,
//! não índice: os de dispositivo podem repetir o slug, e `NULL` em índice único
//! não se comporta igual nos dois bancos.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if !m.has_table("plugins").await? {
            let mut stmt = table("plugins");
            stmt.col(big_pk_auto("id"))
                .col(string_len("slug", 80))
                .col(string_len("name", 160))
                .col(string_len("version", 32))
                .col(text_null("description"))
                .col(string_len("scope", 16))
                .col(big_integer_null("device_id"))
                .col(string_len("source", 16))
                .col(string_len("status", 16))
                .col(json_binary("manifest"))
                .col(text("script"))
                .col(text("usage"))
                .col(json_binary("compatibility"))
                .col(json_binary("tests"))
                .col(json_binary_null("review"))
                .col(string_len("checksum", 64))
                .col(timestamp_with_time_zone_null("last_test_at"))
                .col(boolean_null("last_test_ok"))
                .foreign_key(&mut fk(
                    "plugins",
                    "device_id",
                    "devices",
                    ForeignKeyAction::Cascade,
                ));
            m.create_table(with_timestamps(stmt.take())).await?;
            m.create_index(index("plugins_slug_index", "plugins", &["slug"]))
                .await?;
            m.create_index(index("plugins_device_index", "plugins", &["device_id"]))
                .await?;
        }

        if !m.has_table("device_credentials").await? {
            let mut stmt = table("device_credentials");
            stmt.col(big_pk_auto("id"))
                .col(big_integer("device_id"))
                .col(string_len("kind", 16))
                .col(string_len("username", 128))
                .col(text_null("secret_encrypted"))
                .col(string_len("storage", 16))
                .col(integer_null("port"))
                .col(big_integer_null("via_probe_id"))
                .col(json_binary_null("extra"))
                .foreign_key(&mut fk(
                    "device_credentials",
                    "device_id",
                    "devices",
                    ForeignKeyAction::Cascade,
                ))
                .foreign_key(&mut fk(
                    "device_credentials",
                    "via_probe_id",
                    "probes",
                    ForeignKeyAction::SetNull,
                ));
            m.create_table(with_timestamps(stmt.take())).await?;
            // Uma credencial por tipo de acesso em cada equipamento.
            m.create_index(unique(
                "device_credentials_device_kind_unique",
                "device_credentials",
                &["device_id", "kind"],
            ))
            .await?;
        }

        if !m.has_table("plugin_runs").await? {
            let mut stmt = table("plugin_runs");
            stmt.col(big_pk_auto("id"))
                .col(big_integer_null("plugin_id"))
                .col(big_integer("device_id"))
                .col(big_integer_null("user_id"))
                .col(string_len("action", 64))
                .col(string_len("origin", 16))
                .col(string_len("status", 24))
                .col(json_binary_null("params"))
                .col(json_binary_null("output"))
                .col(json_binary("transcript"))
                .col(text_null("error"))
                .col(timestamp_with_time_zone_null("finished_at"))
                .foreign_key(&mut fk(
                    "plugin_runs",
                    "plugin_id",
                    "plugins",
                    ForeignKeyAction::SetNull,
                ))
                .foreign_key(&mut fk(
                    "plugin_runs",
                    "device_id",
                    "devices",
                    ForeignKeyAction::Cascade,
                ))
                .foreign_key(&mut fk(
                    "plugin_runs",
                    "user_id",
                    "users",
                    ForeignKeyAction::SetNull,
                ));
            m.create_table(with_timestamps(stmt.take())).await?;
            // O histórico da aba é "as execuções deste equipamento, mais
            // recentes primeiro".
            m.create_index(index(
                "plugin_runs_device_created_index",
                "plugin_runs",
                &["device_id", "created_at"],
            ))
            .await?;
        }

        if !m.has_table("plugin_auto_accept").await? {
            let mut stmt = table("plugin_auto_accept");
            stmt.col(big_pk_auto("id"))
                .col(string_len("conversation_key", 64))
                .col(big_integer("device_id"))
                .col(big_integer_null("user_id"))
                .col(string_len("terms_version", 16))
                .col(timestamp_with_time_zone("expires_at"))
                .col(timestamp_with_time_zone_null("revoked_at"))
                .foreign_key(&mut fk(
                    "plugin_auto_accept",
                    "device_id",
                    "devices",
                    ForeignKeyAction::Cascade,
                ))
                .foreign_key(&mut fk(
                    "plugin_auto_accept",
                    "user_id",
                    "users",
                    ForeignKeyAction::SetNull,
                ));
            m.create_table(append_only(stmt.take())).await?;
            m.create_index(index(
                "plugin_auto_accept_lookup_index",
                "plugin_auto_accept",
                &["conversation_key", "device_id"],
            ))
            .await?;
        }

        if !m.has_column("devices", "firmware_version").await? {
            m.alter_table(
                Table::alter()
                    .table(Alias::new("devices"))
                    .add_column(string_len_null("firmware_version", 64))
                    .to_owned(),
            )
            .await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("plugin_auto_accept")).await?;
        m.drop_table(drop("plugin_runs")).await?;
        m.drop_table(drop("device_credentials")).await?;
        m.drop_table(drop("plugins")).await?;
        if m.get_connection().get_database_backend() != sea_orm::DatabaseBackend::Sqlite
            && m.has_column("devices", "firmware_version").await?
        {
            m.alter_table(
                Table::alter()
                    .table(Alias::new("devices"))
                    .drop_column(Alias::new("firmware_version"))
                    .to_owned(),
            )
            .await?;
        }
        Ok(())
    }
}
