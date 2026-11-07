//! Os datasets de alerta que mais de uma parte da central monta (a interface
//! observada pelo SNMP da área Rede e avaliada pelo motor) e o que eles
//! compartilham.

pub mod interface_state;

/// Duração curta para a mensagem do alerta: `45s`, `12 min`, `2h05`.
#[must_use]
pub fn format_seconds(seconds: i64) -> String {
    if seconds < 60 {
        return format!("{seconds}s");
    }
    let minutes = seconds / 60;
    if minutes < 60 {
        return format!("{minutes} min");
    }
    format!("{}h{:02}", minutes / 60, minutes % 60)
}
