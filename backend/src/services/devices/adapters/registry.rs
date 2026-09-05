//! Registro único de adapters de dispositivo.

use std::sync::OnceLock;

use super::{
    contract::{DeviceAdapter, DevicePlatform, SyslogConfigurationAdapter},
    platforms::{EMBEDDED, LINUX, MOBILE, OPENWRT, OTHER, ROUTEROS, UBIQUITI, WINDOWS},
};

/// A ordem é pública: apresentação e precedência da detecção usam a mesma lista.
#[must_use]
pub fn all() -> &'static [&'static dyn DeviceAdapter] {
    static ADAPTERS: OnceLock<Vec<&'static dyn DeviceAdapter>> = OnceLock::new();
    ADAPTERS
        .get_or_init(|| {
            vec![
                &ROUTEROS, &OPENWRT, &UBIQUITI, &LINUX, &WINDOWS, &MOBILE, &EMBEDDED, &OTHER,
            ]
        })
        .as_slice()
}

#[must_use]
pub fn platforms() -> &'static [&'static DevicePlatform] {
    static PLATFORMS: OnceLock<Vec<&'static DevicePlatform>> = OnceLock::new();
    PLATFORMS
        .get_or_init(|| all().iter().map(|adapter| adapter.platform()).collect())
        .as_slice()
}

#[must_use]
pub fn find(id: &str) -> Option<&'static dyn DeviceAdapter> {
    let wanted = id.trim();
    all()
        .iter()
        .copied()
        .find(|adapter| adapter.platform().id.eq_ignore_ascii_case(wanted))
}

#[must_use]
pub fn by_vpn_profile(profile: &str) -> Option<&'static dyn DeviceAdapter> {
    let wanted = profile.trim();
    all().iter().copied().find(|adapter| {
        adapter
            .vpn_profile()
            .is_some_and(|key| key.eq_ignore_ascii_case(wanted))
    })
}

pub fn with_syslog() -> impl Iterator<Item = &'static dyn DeviceAdapter> {
    all()
        .iter()
        .copied()
        .filter(|adapter| adapter.syslog().is_some())
}

#[must_use]
pub fn syslog_for(system: &str) -> Option<&'static dyn SyslogConfigurationAdapter> {
    find(system).and_then(DeviceAdapter::syslog)
}

/// Reconhece metadados de dialeto sem acoplar o parser a uma plataforma.
#[must_use]
pub fn syslog_topics(app_name: Option<&str>) -> Option<String> {
    let app_name = app_name?;
    with_syslog().find_map(|adapter| adapter.syslog()?.topics(app_name))
}

#[must_use]
pub fn syslog_severity(topics: &str) -> Option<i16> {
    with_syslog().find_map(|adapter| adapter.syslog()?.severity(topics))
}

#[must_use]
pub fn is_system_description(value: &str) -> bool {
    all()
        .iter()
        .any(|adapter| adapter.is_system_description(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_e_perfis_sao_unicos() {
        let mut ids = std::collections::HashSet::new();
        let mut profiles = std::collections::HashSet::new();
        for adapter in all() {
            assert!(ids.insert(adapter.platform().id));
            if let Some(profile) = adapter.vpn_profile() {
                assert!(profiles.insert(profile));
            }
        }
    }

    #[test]
    fn recursos_especificos_resolvem_pelo_mesmo_adapter() {
        let routeros = find("RouterOS").expect("adapter");
        assert!(routeros.syslog().is_some());
        assert_eq!(routeros.vpn_profile(), Some("mikrotik"));
        assert_eq!(
            by_vpn_profile("mikrotik").unwrap().platform().id,
            "routeros"
        );
        assert!(is_system_description("Linux host 6.1.0"));
        assert!(!is_system_description("MikroTik"));
    }
}
