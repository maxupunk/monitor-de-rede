//! O que um padrão de log significa: falha de login, link caindo, reboot…
//!
//! A categoria vem uma vez por **padrão** (ver `syslog::template`), nunca por
//! linha. Aqui ficam as categorias e o padrão como o classificador o lê.

use serde::Serialize;

/// As categorias, com a descrição em inglês que o Decisor lê. Os ids são o
/// contrato com a tela (`frontend/src/utils/logCategories.ts`).
pub const LOG_CATEGORIES: &[(&str, &str)] = &[
    ("auth_failure", "Failed login or authentication: wrong password, invalid user, access denied."),
    ("link_change", "An interface, link, port, PPPoE/PPP session or wireless client went down, up or flapped."),
    ("reboot", "The device rebooted, powered on, restarted or the system started."),
    ("routing", "Routing protocol or route events: OSPF, BGP, neighbor, route added or removed."),
    ("config_change", "Configuration was changed, saved or committed, or a user changed a setting."),
    ("dhcp", "DHCP lease, offer, request, release or address pool events."),
    ("resource_exhaustion", "Out of memory, disk full, CPU overload, table or pool exhausted."),
    ("security", "Firewall drop, intrusion, port scan, brute force or other security event (not a single failed login)."),
    ("hardware", "Hardware, power supply, fan, temperature, voltage or sensor event."),
    ("informational", "Routine informational message with nothing to act on (successful login, periodic status)."),
];

/// Um padrão como o Decisor o lê.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogPattern<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topics: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<i16>,
    pub template: &'a str,
    pub example: &'a str,
}

impl LogPattern<'_> {
    #[must_use]
    pub fn state(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// A categoria é uma das conhecidas?
#[must_use]
pub fn is_category(id: &str) -> bool {
    LOG_CATEGORIES.iter().any(|(known, _)| *known == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estado_leva_padrao_e_exemplo() {
        let state = LogPattern {
            app: None,
            topics: Some("system,error,critical"),
            severity: Some(3),
            template: "login failure for user admin from #",
            example: "login failure for user admin from 10.0.0.5",
        }
        .state();
        assert!(state.contains("\"template\":\"login failure for user admin from #\""));
        assert!(!state.contains("\"app\""));
    }
}
