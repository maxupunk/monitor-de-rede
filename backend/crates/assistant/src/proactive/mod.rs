//! IA proativa: resumos de incidente e resumo periódico da rede, ambos
//! configuráveis e desligados por padrão.

pub mod config;
pub mod digest;
pub mod incident;
pub mod runner;
pub mod schedule;
pub mod triage;

use std::sync::Arc;

use loco_rs::prelude::AppContext;

use runner::{DriverFactory, ProviderDrivers};

/// As rotinas automáticas, para sempre. Elas leem as configurações a cada
/// evento ou verificação, então ficam sempre no ar e só agem quando ligadas.
///
/// Rotina do processo líder (ADR 024): o `alert:triggered` chega a todos os
/// processos da central, e cada um resumiria o mesmo incidente.
pub async fn run(ctx: AppContext) {
    let drivers: Arc<dyn DriverFactory> = Arc::new(ProviderDrivers);
    tokio::join!(
        incident::listen(ctx.clone(), Arc::clone(&drivers)),
        digest::schedule_loop(ctx, drivers),
    );
}
