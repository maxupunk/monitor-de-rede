//! O registro de fabricantes em memória e a consulta por MAC.
//!
//! Um processo carrega o registro uma vez ([`install`]) e toda consulta é um
//! acesso a `HashMap` — a descoberta classifica centenas de hosts por quadro e
//! não pode ir ao banco a cada MAC.

use std::{
    collections::HashMap,
    sync::{Arc, LazyLock, PoisonError, RwLock},
};

use serde::Serialize;

use super::builtin;

/// Tamanhos de prefixo dos blocos do IEEE, do mais específico ao mais amplo:
/// MA-S (36 bits), MA-M (28 bits) e MA-L (24 bits, o "OUI" clássico).
const PREFIX_LENGTHS: [usize; 3] = [9, 7, 6];

/// Prefixo hexadecimal → organização, como o IEEE publica.
#[derive(Debug, Default)]
pub struct OuiRegistry {
    entries: HashMap<String, String>,
}

impl OuiRegistry {
    #[must_use]
    pub fn new(entries: impl IntoIterator<Item = (String, String)>) -> Self {
        Self {
            entries: entries
                .into_iter()
                .map(|(prefix, organization)| (prefix.to_ascii_lowercase(), organization))
                .collect(),
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// A organização dona do bloco mais específico que contém o MAC.
    #[must_use]
    pub fn organization(&self, mac_address: &str) -> Option<&str> {
        let hex = hex_digits(mac_address);
        PREFIX_LENGTHS
            .iter()
            .filter(|length| hex.len() >= **length)
            .find_map(|length| self.entries.get(&hex[..*length]))
            .map(String::as_str)
    }
}

static INSTALLED: LazyLock<RwLock<Arc<OuiRegistry>>> = LazyLock::new(RwLock::default);

/// Troca o registro do processo (carga do banco ou atualização).
pub fn install(registry: OuiRegistry) {
    *INSTALLED.write().unwrap_or_else(PoisonError::into_inner) = Arc::new(registry);
}

/// O registro em uso — vazio até a primeira carga.
#[must_use]
pub fn installed() -> Arc<OuiRegistry> {
    INSTALLED
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

/// Até 12 dígitos hexadecimais do MAC, em minúsculas, sem separadores.
fn hex_digits(mac_address: &str) -> String {
    mac_address
        .chars()
        .filter(char::is_ascii_hexdigit)
        .take(12)
        .map(|char| char.to_ascii_lowercase())
        .collect()
}

/// De onde veio o nome do fabricante.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum VendorSource {
    /// Registro oficial do IEEE, baixado pelo servidor.
    Ieee,
    /// Tabela embutida no NetMonitor.
    Builtin,
    /// Ninguém conhece este prefixo.
    None,
}

/// O que se sabe do fabricante de um MAC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MacVendor {
    /// Nome curto, para a tela ("Espressif").
    pub vendor: Option<String>,
    /// Nome completo registrado ("Espressif Inc.").
    pub organization: Option<String>,
    pub source: VendorSource,
    /// MAC aleatório (privacidade) ou de máquina virtual: nunca tem dono no
    /// IEEE, e dizer "fabricante desconhecido" sem explicar confunde.
    pub locally_administered: bool,
}

/// Fabricante de um MAC: o registro do IEEE primeiro, a tabela embutida
/// depois. Um contêiner Docker (`02:42:…`) é reconhecido mesmo sendo local.
#[must_use]
pub fn lookup(mac_address: &str) -> MacVendor {
    lookup_in(&installed(), mac_address)
}

/// Só o nome curto, para quem não precisa do resto.
#[must_use]
pub fn vendor_name(mac_address: &str) -> Option<String> {
    lookup(mac_address).vendor
}

fn lookup_in(registry: &OuiRegistry, mac_address: &str) -> MacVendor {
    let locally_administered = builtin::is_locally_administered(mac_address);
    if builtin::is_docker(mac_address) {
        return MacVendor {
            vendor: builtin::lookup_vendor(mac_address).map(str::to_string),
            organization: None,
            source: VendorSource::Builtin,
            locally_administered,
        };
    }
    if let Some(organization) = registry.organization(mac_address) {
        return MacVendor {
            vendor: Some(short_name(organization)),
            organization: Some(organization.to_string()),
            source: VendorSource::Ieee,
            locally_administered,
        };
    }
    match builtin::lookup_vendor(mac_address) {
        Some(vendor) => MacVendor {
            vendor: Some(vendor.to_string()),
            organization: None,
            source: VendorSource::Builtin,
            locally_administered,
        },
        None => MacVendor {
            vendor: None,
            organization: None,
            source: VendorSource::None,
            locally_administered,
        },
    }
}

/// Sufixos societários que só alongam o nome na tela.
const LEGAL_SUFFIXES: &[&str] = &[
    "co., ltd",
    "co.,ltd",
    "co ltd",
    "co. ltd",
    "co.",
    "co",
    "ltd",
    "limited",
    "inc",
    "incorporated",
    "corporation",
    "corp",
    "llc",
    "l.l.c",
    "gmbh",
    "ag",
    "s.a",
    "sa",
    "s.p.a",
    "spa",
    "b.v",
    "bv",
    "a/s",
    "ab",
    "oy",
    "pty",
    "plc",
    "s.r.l",
    "srl",
    "ltda",
    "kg",
    "k.k",
    "sas",
    "s.a.s",
];

/// "Hangzhou Hikvision Digital Technology Co.,Ltd." → "Hangzhou Hikvision
/// Digital Technology". Repete até não sobrar sufixo societário no fim.
#[must_use]
pub fn short_name(organization: &str) -> String {
    let mut name = organization.trim().to_string();
    loop {
        let trimmed = name
            .trim_end_matches(|char: char| char == '.' || char == ',' || char.is_whitespace())
            .to_string();
        let lower = trimmed.to_ascii_lowercase();
        let suffix = LEGAL_SUFFIXES.iter().find(|suffix| {
            lower.strip_suffix(*suffix).is_some_and(|rest| {
                rest.is_empty() || rest.ends_with(|char: char| char == ',' || char.is_whitespace())
            })
        });
        match suffix {
            Some(suffix) if trimmed.len() > suffix.len() => {
                name = trimmed[..trimmed.len() - suffix.len()].to_string();
            }
            _ => {
                return if trimmed.is_empty() {
                    organization.trim().into()
                } else {
                    trimmed
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> OuiRegistry {
        OuiRegistry::new([
            ("5CCF7F".to_string(), "Espressif Inc.".to_string()),
            (
                "70B3D5".to_string(),
                "IEEE Registration Authority".to_string(),
            ),
            ("70B3D51".to_string(), "Fabricante MA-M Ltda.".to_string()),
            ("70B3D5123".to_string(), "Fabricante MA-S GmbH".to_string()),
        ])
    }

    #[test]
    fn bloco_mais_especifico_vence() {
        let registry = registry();
        assert_eq!(
            registry.organization("70:B3:D5:12:34:56"),
            Some("Fabricante MA-S GmbH")
        );
        assert_eq!(
            registry.organization("70:B3:D5:19:99:99"),
            Some("Fabricante MA-M Ltda.")
        );
        assert_eq!(
            registry.organization("70:B3:D5:F0:00:00"),
            Some("IEEE Registration Authority")
        );
        assert_eq!(
            registry.organization("5c-cf-7f-00-11-22"),
            Some("Espressif Inc.")
        );
        assert_eq!(registry.organization("5c:cf"), None);
    }

    #[test]
    fn ieee_antes_da_tabela_embutida_e_ela_como_reserva() {
        let registry = registry();
        let ieee = lookup_in(&registry, "5c:cf:7f:00:11:22");
        assert_eq!(ieee.vendor.as_deref(), Some("Espressif"));
        assert_eq!(ieee.organization.as_deref(), Some("Espressif Inc."));
        assert_eq!(ieee.source, VendorSource::Ieee);

        let builtin = lookup_in(&OuiRegistry::default(), "5c:cf:7f:00:11:22");
        assert_eq!(builtin.vendor.as_deref(), Some("Espressif"));
        assert_eq!(builtin.source, VendorSource::Builtin);

        let docker = lookup_in(&registry, "02:42:ac:11:00:02");
        assert_eq!(docker.vendor.as_deref(), Some("Docker (contêiner)"));
        assert!(docker.locally_administered);

        let random = lookup_in(&registry, "da:a1:19:00:11:22");
        assert_eq!(random.source, VendorSource::None);
        assert!(random.locally_administered);
    }

    #[test]
    fn nome_curto_tira_sufixo_societario() {
        assert_eq!(
            short_name("Hangzhou Hikvision Digital Technology Co.,Ltd."),
            "Hangzhou Hikvision Digital Technology"
        );
        assert_eq!(
            short_name("TP-LINK TECHNOLOGIES CO.,LTD."),
            "TP-LINK TECHNOLOGIES"
        );
        assert_eq!(short_name("Espressif Inc."), "Espressif");
        assert_eq!(short_name("Ubiquiti Networks Inc."), "Ubiquiti Networks");
        assert_eq!(
            short_name("AVM Audiovisuelles Marketing und Computersysteme GmbH"),
            "AVM Audiovisuelles Marketing und Computersysteme"
        );
        assert_eq!(short_name("Intelbras"), "Intelbras");
        assert_eq!(short_name("Cisco Systems, Inc"), "Cisco Systems");
        // Nome que é só sufixo não vira texto vazio.
        assert_eq!(short_name("AG"), "AG");
    }
}
