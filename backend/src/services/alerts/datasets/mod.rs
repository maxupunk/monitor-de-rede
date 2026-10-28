//! Construtores de fatos: traduzem observações para o vocabulário das regras.

pub mod interface_state;
pub mod log_pattern;
pub mod monitor_result;
pub mod vpn_peer;
pub mod wifi_client;

/// Duração curta para a mensagem do alerta: `45s`, `12 min`, `2h05`.
fn format_seconds(seconds: i64) -> String {
    if seconds < 60 {
        return format!("{seconds}s");
    }
    let minutes = seconds / 60;
    if minutes < 60 {
        return format!("{minutes} min");
    }
    format!("{}h{:02}", minutes / 60, minutes % 60)
}
