//! Guardas para impedir que catálogos paralelos de plataforma reapareçam.

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("raiz do repositório")
        .to_path_buf()
}

fn read(relative: &str) -> String {
    let path = root().join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("não foi possível ler {}: {error}", path.display()))
}

fn production_part(relative: &str) -> String {
    read(relative)
        .split("#[cfg(test)]")
        .next()
        .unwrap_or_default()
        .to_owned()
}

#[test]
fn syslog_nao_mantem_catalogo_ou_match_paralelo() {
    let snippets = production_part("backend/src/services/syslog/snippets.rs");
    assert!(!snippets.contains("RECEITAS"));
    assert!(!snippets.contains("match sistema"));
    assert!(snippets.contains("adapters::registry"));

    let parser = production_part("backend/src/services/syslog/parser.rs");
    assert!(!parser.contains("topicos_do_routeros"));
    assert!(parser.contains("registry::syslog_topics"));
}

#[test]
fn consumidores_de_dispositivo_dependem_do_registro() {
    let systems = production_part("backend/src/services/devices/systems.rs");
    assert!(!systems.contains("const CATALOGO"));
    assert!(systems.contains("adapters::{registry"));

    let discovery = production_part("backend/src/services/discovery/device_identifier.rs");
    assert!(discovery.contains("adapters::registry"));

    let peers = production_part("backend/src/services/vpn/peer_service.rs");
    assert!(peers.contains("adapters::registry::by_vpn_profile"));
    assert!(!peers.contains("payload.profile =="));
}

#[test]
fn frontend_nao_duplica_os_perfis_da_vpn() {
    let store = production_part("frontend/src/stores/vpn.ts");
    assert!(!store.contains("VPN_PROFILE_LABELS"));
    assert!(!store.contains("VPN_PROFILE_ICONS"));
    assert!(!store.contains("'mikrotik' | 'openwrt'"));
    assert!(store.contains("profiles.find"));
}
