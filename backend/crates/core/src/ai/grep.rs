//! O contrato das fontes do `grep` da IA: cada fonte sabe consultar a sua
//! tabela com o maior pré-filtro possível e devolver [`GrepRecord`] em ordem
//! cronológica; o casamento fino e a forma de saída são do motor. Mora no
//! núcleo para que uma área contribua a sua fonte (a saída dos containers vem
//! de Servidores).

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use loco_rs::prelude::AppContext;

use crate::{models::devices, shared::errors::AppResult};

/// Uma ocorrência vinda de qualquer fonte (log, alerta, checagem).
#[derive(Debug, Clone, PartialEq)]
pub struct GrepRecord {
    pub at: DateTime<Utc>,
    /// Dispositivo, monitor ou IP de origem.
    pub origin: String,
    pub severity: String,
    /// Gravidade para ordenar (menor é pior).
    pub rank: i16,
    pub text: String,
    /// Id no banco de logs, para buscar o contexto (`-C`).
    pub log_id: Option<i64>,
}

/// Linhas lidas por fonte a cada grep. Acima disso a resposta avisa
/// (`capped`) e a IA estreita a janela ou o dispositivo.
pub const SCAN_CAP: u64 = 10_000;

#[derive(Debug, Clone)]
pub struct GrepFilter {
    pub device: Option<devices::Model>,
    pub since: DateTime<Utc>,
    pub until: DateTime<Utc>,
    /// Severidade syslog máxima (só logs).
    pub severity: Option<i16>,
    /// Trecho literal que o banco pode usar para pré-filtrar.
    pub needle: Option<String>,
    /// Nome ou id do container (só a fonte `docker`).
    pub container: Option<String>,
    /// Servidor Docker do container (só a fonte `docker`); `None` é a central.
    pub host: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Scan {
    pub records: Vec<GrepRecord>,
    pub capped: bool,
}

#[async_trait]
pub trait GrepSource: Send + Sync {
    /// Lê a janela. `Err(mensagem)` quando a fonte não existe neste servidor.
    async fn scan(&self, ctx: &AppContext, filter: &GrepFilter) -> AppResult<Result<Scan, String>>;

    /// Até `lines` registros antes e depois de `anchor` (o `-C`). Por padrão a
    /// fonte não tem contexto.
    async fn context(
        &self,
        _ctx: &AppContext,
        _anchor: &GrepRecord,
        _lines: u64,
    ) -> AppResult<(Vec<GrepRecord>, Vec<GrepRecord>)> {
        Ok((Vec::new(), Vec::new()))
    }
}
