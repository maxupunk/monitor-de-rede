//! Funde interfaces duplicadas pelo par (dispositivo, nome).
//!
//! # De onde vêm as duplicatas
//!
//! Uma interface PPPoE troca de `ifIndex` a cada reconexão. Até o commit que
//! introduziu o casamento por nome em `snmp::service`, a sincronização casava a
//! interface do walk com a do banco **só pelo índice**: o índice novo não
//! encontrava ninguém e uma linha nova era inserida, deixando a antiga órfã
//! para sempre — com o mesmo `name` e sem nunca mais receber `last_seen_at`.
//!
//! A órfã não é inofensiva. `list_interfaces` calcula `is_monitored` pelo
//! **nome** do monitor, então as duas linhas se dizem monitoradas: o painel
//! mostra a mesma porta duas vezes, uma delas eternamente zerada. E a tela de
//! descoberta passa a acusar "interface removida" em toda gravação, para uma
//! interface cujo índice não aparece na varredura e que, por isso, o operador
//! não consegue marcar para calar o aviso.
//!
//! A limpeza de órfãs em `apply_monitors` não resolve: ela é condicionada a
//! `!discovered_names.contains(name)` — e o nome **está** entre os descobertos,
//! porque a interface viva é homônima. O guarda protege exatamente a linha que
//! deveria remover.
//!
//! # Quem sobrevive
//!
//! A de `last_seen_at` mais recente, com as nulas por último e desempate pelo
//! maior `id`. `(last_seen_at IS NULL)` precisa estar explícito na ordenação
//! porque `ORDER BY ... DESC` põe nulo por **último** no SQLite e por
//! **primeiro** no PostgreSQL: sem isso, uma linha nunca vista venceria a viva
//! em produção e perderia em teste.
//!
//! As referências da perdedora são repontadas antes do `DELETE` — métricas,
//! as duas pontas de `device_links` e o `link_interface_id` do dispositivo.
//! Nenhuma delas tem FK declarada, então ninguém reclamaria de um ponteiro
//! órfão: o histórico simplesmente sumiria do gráfico.

use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Pares `(perdedora, vencedora)` das interfaces homônimas do mesmo aparelho.
const DUPLICATAS: &str = "\
SELECT d.id AS loser_id, k.id AS keeper_id \
FROM device_interfaces d \
JOIN device_interfaces k \
  ON k.device_id = d.device_id \
 AND LOWER(k.name) = LOWER(d.name) \
WHERE k.id = ( \
    SELECT k2.id FROM device_interfaces k2 \
    WHERE k2.device_id = d.device_id AND LOWER(k2.name) = LOWER(d.name) \
    ORDER BY (k2.last_seen_at IS NULL), k2.last_seen_at DESC, k2.id DESC \
    LIMIT 1 \
) \
AND d.id <> k.id";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        let backend = db.get_database_backend();

        let pares = db
            .query_all_raw(Statement::from_string(backend, DUPLICATAS.to_string()))
            .await?;

        for par in pares {
            let perdedora: i64 = par.try_get("", "loser_id")?;
            let vencedora: i64 = par.try_get("", "keeper_id")?;

            // Os ids vêm do próprio banco: interpolar inteiros mantém o SQL
            // igual nos dois dialetos, que divergem no marcador de parâmetro
            // (`?` no SQLite, `$1` no PostgreSQL).
            for sql in [
                format!(
                    "UPDATE metrics SET interface_id = {vencedora} \
                     WHERE interface_id = {perdedora}"
                ),
                format!(
                    "UPDATE device_links SET source_interface_id = {vencedora} \
                     WHERE source_interface_id = {perdedora}"
                ),
                format!(
                    "UPDATE device_links SET target_interface_id = {vencedora} \
                     WHERE target_interface_id = {perdedora}"
                ),
                format!(
                    "UPDATE devices SET link_interface_id = {vencedora} \
                     WHERE link_interface_id = {perdedora}"
                ),
                format!("DELETE FROM device_interfaces WHERE id = {perdedora}"),
            ] {
                db.execute_raw(Statement::from_string(backend, sql)).await?;
            }
        }
        Ok(())
    }

    /// Sem volta: a linha órfã não guardava nada que a sobrevivente não tenha.
    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
