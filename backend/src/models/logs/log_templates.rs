//! Entidade `log_templates` — **escrita à mão**, como `device_logs`: o banco
//! de logs não passa pelo `db entities`. Inteiros no tamanho do PostgreSQL
//! (`BIGINT` → `i64`, `SMALLINT` → `i16`). Ver
//! `migration::logs::m20260930_000001_log_templates`.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "log_templates")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub template_hash: i64,
    #[sea_orm(column_type = "Text")]
    pub template: String,
    #[sea_orm(column_type = "Text")]
    pub example: String,
    pub app_name: Option<String>,
    /// O palpite do Laya.
    pub category: Option<String>,
    /// 0–100.
    pub confidence: Option<i16>,
    pub model: Option<String>,
    pub classified_at: Option<DateTimeWithTimeZone>,
    /// A correção ou confirmação do operador — vale mais que o palpite.
    pub user_category: Option<String>,
    pub confirmed_at: Option<DateTimeWithTimeZone>,
    pub first_seen_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// A categoria que vale: a do operador, senão a do Laya.
    #[must_use]
    pub fn effective_category(&self) -> Option<&str> {
        self.user_category.as_deref().or(self.category.as_deref())
    }
}
