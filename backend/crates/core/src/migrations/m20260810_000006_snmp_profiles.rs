//! `snmp_profiles` — as credenciais SNMP reutilizáveis ("Credenciais SNMP").
//!
//! Um perfil é a forma de autenticar num agente SNMP: comunidade (v1/v2c) ou
//! usuário USM (v3, com autenticação e privacidade). O dispositivo aponta para
//! um perfil (`devices.snmp_profile_id`) em vez de guardar a credencial dele,
//! e a coleta, os monitores e a descoberta resolvem por aqui
//! (`devices::snmp_credentials`).
//!
//! **Segredos cifrados** com a `ENCRYPTION_KEY` (`*_encrypted`), como as
//! chaves da VPN: a comunidade, a senha de autenticação e a de privacidade. O
//! que a tela lista — nome, versão, usuário, protocolos — fica em claro, para
//! não decifrar nada à toa.
//!
//! Nasce **antes** de `devices`, porque é alvo da FK dela.

use sea_orm_migration::prelude::*;

use crate::schema::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let mut stmt = table("snmp_profiles");
        stmt.col(big_pk_auto("id"))
            .col(string_len("name", 120))
            .col(string_len("version", 8))
            .col(text_null("community_encrypted"))
            .col(string_len_null("username", 64))
            .col(string_len_null("auth_protocol", 16))
            .col(text_null("auth_key_encrypted"))
            .col(string_len_null("priv_protocol", 16))
            .col(text_null("priv_key_encrypted"))
            .col(string_len_null("context_name", 64))
            // O perfil usado por dispositivo com SNMP ligado que não escolheu
            // um, e o primeiro que a descoberta tenta. Um só por vez — quem
            // garante é o serviço, na mesma transação que marca o novo.
            .col(boolean("is_default").default(false).take());

        m.create_table(with_timestamps(stmt.take())).await?;

        // Dois perfis com o mesmo nome seriam indistinguíveis no seletor do
        // cadastro de dispositivo.
        m.create_index(unique(
            "snmp_profiles_name_unique",
            "snmp_profiles",
            &["name"],
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("snmp_profiles")).await
    }
}
