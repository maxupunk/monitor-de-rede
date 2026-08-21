//! Gerenciador de cadência periódica em memória para tarefas auxiliares do scheduler.

use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

use chrono::{DateTime, Utc};

/// Cadência da leitura de status dos túneis VPN.
pub const VPN_STATUS_INTERVAL_SECONDS: i64 = 10;

/// Cadência da gravação de histórico de tráfego VPN.
pub const VPN_TRAFFIC_INTERVAL_SECONDS: i64 = 30;

/// Cadência da purga de dados antigos.
pub const DATA_PRUNE_INTERVAL_SECONDS: i64 = 3_600;

/// Cadência do rollup de resultados brutos em buckets horários.
pub const ROLLUP_INTERVAL_SECONDS: i64 = 3_600;

/// Próximo instante de cada tarefa periódica do ciclo.
fn next_run_at() -> &'static Mutex<HashMap<&'static str, DateTime<Utc>>> {
    static NEXT: OnceLock<Mutex<HashMap<&'static str, DateTime<Utc>>>> = OnceLock::new();
    NEXT.get_or_init(|| Mutex::new(HashMap::new()))
}

/// `true` quando a tarefa venceu; já reserva o próximo horário no mapa.
pub fn is_due(key: &'static str, interval_seconds: i64, now: DateTime<Utc>) -> bool {
    let Ok(mut next) = next_run_at().lock() else {
        return true;
    };
    match next.get(key) {
        Some(scheduled) if now < *scheduled => false,
        _ => {
            next.insert(key, now + chrono::Duration::seconds(interval_seconds));
            true
        }
    }
}
