//! Entidade `device_logs` — **escrita à mão**, não gerada.
//!
//! O `cargo loco db entities` aponta para a base principal e não conhece o
//! banco de logs. A regra do `AGENTS.md` continua valendo pelo motivo original:
//! os inteiros seguem o **PostgreSQL**, nunca o que o SQLite reporta. O SQLite
//! diz `INTEGER` para tudo, e uma entidade com `i64` onde o Postgres tem
//! `SMALLINT` faz o `sqlx` recusar a leitura em produção.
//!
//! Daí `facility`/`severity` serem `i16` (`smallint`) e `pid` ser `i32`
//! (`integer`), casando com
//! [`migration::logs`](../../../migration/src/logs/m20260815_000001_device_logs.rs).
//!
//! Sem `updated_at`: a tabela é append-only.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "device_logs")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    /// Sem FK: `devices` mora no outro banco. A hidratação do nome é feita pelo
    /// serviço, em segunda consulta.
    pub device_id: Option<i64>,
    pub source_ip: String,
    /// A verdade. Toda ordenação e todo filtro por período saem daqui.
    pub received_at: DateTimeWithTimeZone,
    /// O que o dispositivo alegou. Anulável porque o RouterOS cru não manda
    /// timestamp nenhum, e porque relógio de roteador erra.
    pub device_time: Option<DateTimeWithTimeZone>,
    pub facility: Option<i16>,
    pub severity: Option<i16>,
    pub hostname: Option<String>,
    pub app_name: Option<String>,
    pub pid: Option<i32>,
    /// Tópicos do RouterOS, vírgula-separados (`system,info,account`).
    pub topics: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub message: String,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
