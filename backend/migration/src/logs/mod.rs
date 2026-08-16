//! Migrator do **banco de logs** — `/data/logs.sqlite`, separado do principal.
//!
//! Por que um segundo banco e um segundo migrator: a retenção apaga ~1 M de
//! linhas por dia, e no SQLite um `DELETE` desse tamanho segura o *write lock*
//! do arquivo inteiro. Na base principal isso congelaria a gravação de
//! `monitor_results` pelo tempo que durasse — o scheduler roda a cada 5 s e não
//! tem para onde escapar. Somado ao WAL crescendo por causa de um escritor de
//! alta frequência, o isolamento se paga.
//!
//! O preço está registrado em [`m20260815_000001_device_logs`]: sem FK e sem
//! `JOIN` com `devices`.
//!
//! Estas migrations **não** entram no [`crate::Migrator`] nem em
//! `models::tables::CREATION_ORDER` — aquela lista é o catálogo da base
//! principal e alimenta o `Hooks::truncate` do Loco, que não alcança este banco.

use sea_orm_migration::prelude::*;

mod m20260815_000001_device_logs;

pub struct LogsMigrator;

#[async_trait::async_trait]
impl MigratorTrait for LogsMigrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(m20260815_000001_device_logs::Migration)]
    }
}
