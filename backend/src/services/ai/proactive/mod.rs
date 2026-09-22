//! IA proativa: resumos de incidente e resumo periódico da rede, ambos
//! configuráveis e desligados por padrão.

pub mod config;
pub mod digest;
pub mod incident;
pub mod runner;
pub mod schedule;

use std::sync::Arc;

use loco_rs::prelude::AppContext;

use runner::{DriverFactory, ProviderDrivers};

/// Sobe as rotinas automáticas. Elas leem as configurações a cada evento ou
/// verificação, então ficam sempre no ar e só agem quando ligadas.
pub fn spawn(ctx: &AppContext) {
    let drivers: Arc<dyn DriverFactory> = Arc::new(ProviderDrivers);
    incident::spawn_listener(ctx.clone(), Arc::clone(&drivers));
    digest::spawn_scheduler(ctx.clone(), drivers);
}
