//! Vincula o monitor de tráfego à interface pelo `id`, não pelo nome.
//!
//! # O que estava errado
//!
//! O monitor de uma porta se chama `Interface {ifName}`, e essa **string** era o
//! único vínculo entre ele e a linha de `device_interfaces`. Toda pergunta
//! "esta interface é monitorada?" virava uma comparação de texto.
//!
//! Isso produz três defeitos que não têm como ser corrigidos sem a coluna:
//!
//! 1. **Homônimas dividem um monitor.** Duas linhas com o mesmo `ifName` — o que
//!    acontece quando o `ifIndex` de uma PPPoE muda e a antiga fica órfã —
//!    apontam para o mesmo `Interface pppoe-wan`. As duas se declaram
//!    monitoradas e o painel mostra a porta duas vezes, uma delas zerada.
//! 2. **Renomear a porta órfã o monitor.** O operador troca `lan1` por `uplink`
//!    no equipamento; a interface é reencontrada pela cadeia de identidade, mas
//!    o monitor continua chamado `Interface lan1` e ninguém mais o acha.
//! 3. **A remoção erra o alvo.** A limpeza de interfaces sumidas procura o
//!    monitor pelo nome da linha que está apagando — se outra porta tiver o
//!    mesmo nome, apaga o monitor da porta errada.
//!
//! # O backfill
//!
//! Casa `monitors.name = 'Interface ' || device_interfaces.name` dentro do mesmo
//! dispositivo. Roda **depois** de `m20260909_000001`, que já fundiu as
//! homônimas — então cada nome resolve para uma linha só. A dedupe por
//! `monitor_id` é cinto de segurança para uma instalação que chegue aqui com
//! duplicata que a fusão não tenha alcançado.
//!
//! Monitores sem interface (`cpu_usage`, `memory_usage`, ping, TCP) ficam com
//! `interface_id` nulo, que é o que eles são: monitores do dispositivo.

use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const INDICE: &str = "monitors_interface_id_index";

/// Pares `(monitor, interface)` deduzíveis do nome atual do monitor.
const VINCULOS: &str = "\
SELECT m.id AS monitor_id, di.id AS interface_id \
FROM monitors m \
JOIN device_interfaces di \
  ON di.device_id = m.device_id \
 AND m.name = 'Interface ' || di.name \
WHERE m.interface_id IS NULL \
ORDER BY m.id, di.id";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if !m.has_column("monitors", "interface_id").await? {
            m.alter_table(
                Table::alter()
                    .table(Alias::new("monitors"))
                    .add_column(big_integer_null("interface_id"))
                    .to_owned(),
            )
            .await?;
        }

        let db = m.get_connection();
        let backend = db.get_database_backend();

        let mut ja_vinculados = std::collections::HashSet::new();
        for par in db
            .query_all_raw(Statement::from_string(backend, VINCULOS.to_string()))
            .await?
        {
            let monitor: i64 = par.try_get("", "monitor_id")?;
            let interface: i64 = par.try_get("", "interface_id")?;
            if !ja_vinculados.insert(monitor) {
                continue;
            }
            // Ids vindos do próprio banco: interpolar inteiros mantém o SQL
            // igual nos dois dialetos, que divergem no marcador de parâmetro.
            db.execute_raw(Statement::from_string(
                backend,
                format!("UPDATE monitors SET interface_id = {interface} WHERE id = {monitor}"),
            ))
            .await?;
        }

        db.execute_raw(Statement::from_string(
            backend,
            format!("CREATE INDEX IF NOT EXISTS {INDICE} ON monitors (interface_id)"),
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        db.execute_raw(Statement::from_string(
            db.get_database_backend(),
            format!("DROP INDEX IF EXISTS {INDICE}"),
        ))
        .await?;
        if m.get_connection().get_database_backend() != sea_orm::DatabaseBackend::Sqlite
            && m.has_column("monitors", "interface_id").await?
        {
            m.alter_table(
                Table::alter()
                    .table(Alias::new("monitors"))
                    .drop_column(Alias::new("interface_id"))
                    .to_owned(),
            )
            .await?;
        }
        Ok(())
    }
}
