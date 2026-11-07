//! Ponto de extensão das rotinas periódicas (ADR 017, registro de rotinas).
//!
//! O ciclo do scheduler e a purga periódica são do núcleo; o que cada área
//! faz neles — a coleta SNMP sem monitor e a varredura agendada da Rede, a
//! telemetria dos túneis da VPN, o vigia do Wi-Fi e os testes de enlace
//! agendados, a retenção do histórico de enlace — chega por aqui, registrado
//! pelo ponto de montagem. Assim o ciclo não conhece área nenhuma.
//!
//! **Área desligada pausa a rotina dela, nunca um monitor** (Fase 5). Quem
//! decide é a rotina, que consulta [`crate::areas::is_enabled`] no ponto em
//! que já consultava: nem toda rotina de uma área pausa — a coleta SNMP e a
//! telemetria dos túneis alimentam monitores e alertas, e monitor não para.
//! Os monitores agrupados ([`crate::monitoring::MonitorGroup`]) rodam sempre.

pub mod cadence;

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use loco_rs::app::AppContext;
use sea_orm::DatabaseConnection;

use crate::{areas::Area, shared::errors::AppResult};

/// O ciclo em curso, como a rotina o enxerga.
#[derive(Debug, Clone, Copy)]
pub struct Cycle {
    /// O instante do ciclo.
    pub now: DateTime<Utc>,
    /// O ciclo roda no laço do servidor: trabalho longo vai para segundo plano
    /// em vez de segurar o ciclo (a varredura pendente, por exemplo). No ciclo
    /// avulso (`task scheduler_run`) ele roda até o fim.
    pub detached: bool,
}

/// Uma rotina de área chamada a cada ciclo do scheduler. A cadência dela é
/// dela ([`cadence::is_due`]); falha de uma não interrompe as outras.
#[async_trait]
pub trait Routine: Send + Sync {
    /// Nome curto, para o log.
    fn name(&self) -> &'static str;

    /// A área que a contribui.
    fn area(&self) -> Area;

    /// Uma passada.
    ///
    /// # Errors
    ///
    /// A falha da passada; o ciclo a registra e segue.
    async fn run(&self, ctx: &AppContext, cycle: Cycle) -> AppResult<()>;
}

/// A retenção de dados de uma área, chamada pela purga periódica do núcleo.
#[async_trait]
pub trait RetentionJob: Send + Sync {
    /// Nome curto, para o log da purga.
    fn name(&self) -> &'static str;

    /// A área que a contribui.
    fn area(&self) -> Area;

    /// Apaga o que venceu e devolve quantas linhas saíram.
    ///
    /// # Errors
    ///
    /// Erro do banco.
    async fn prune(&self, db: &DatabaseConnection, now: DateTime<Utc>) -> AppResult<u64>;
}

/// As rotinas e retenções registradas, no `shared_store` do processo (o
/// scheduler roda como `task` também: o `after_context` as instala).
#[derive(Clone, Default)]
pub struct AreaRoutines {
    routines: Arc<Vec<Arc<dyn Routine>>>,
    retention: Arc<Vec<Arc<dyn RetentionJob>>>,
}

impl AreaRoutines {
    #[must_use]
    pub fn new(routines: Vec<Arc<dyn Routine>>, retention: Vec<Arc<dyn RetentionJob>>) -> Self {
        Self {
            routines: Arc::new(routines),
            retention: Arc::new(retention),
        }
    }

    /// As do processo; vazio quando o ponto de montagem não registrou nada.
    #[must_use]
    pub fn from_context(ctx: &AppContext) -> Self {
        ctx.shared_store.get::<Self>().unwrap_or_default()
    }

    /// As rotinas do ciclo, na ordem do registro.
    pub fn routines(&self) -> impl Iterator<Item = &Arc<dyn Routine>> {
        self.routines.iter()
    }

    /// As retenções da purga, na ordem do registro.
    pub fn retention(&self) -> impl Iterator<Item = &Arc<dyn RetentionJob>> {
        self.retention.iter()
    }
}

/// Lê uma variável de retenção, caindo no padrão quando ausente ou inválida.
///
/// Valor `0` ou negativo é tratado como "use o padrão": desligar a purga por
/// engano encheria o disco em silêncio, que é pior do que ignorar a
/// configuração.
#[must_use]
pub fn retention_days(variable: &str, default: i64) -> i64 {
    std::env::var(variable)
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}
