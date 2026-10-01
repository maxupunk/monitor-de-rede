//! Plugins que acompanham o sistema.
//!
//! Servem a dois propósitos: funcionar de saída nos casos mais comuns
//! (OpenWrt, Linux por SSH, página web) e ser **exemplo vivo** para a IA e
//! para quem escreve plugin — a skill de autoria aponta para eles. Por isso o
//! `cargo test` exige que cada um valide, passe nos próprios testes e saia da
//! análise estática com risco baixo: exemplo quebrado ensina errado.

use super::package::PluginPackage;

const SOURCES: &[(&str, &str)] = &[
    (
        "openwrt-packages",
        include_str!("openwrt-packages.nmplugin.json"),
    ),
    (
        "linux-ssh-status",
        include_str!("linux-ssh-status.nmplugin.json"),
    ),
    ("openwrt-wifi", include_str!("openwrt-wifi.nmplugin.json")),
    (
        "http-page-info",
        include_str!("http-page-info.nmplugin.json"),
    ),
];

/// Os pacotes embutidos.
///
/// # Panics
///
/// Nunca em build testado: o teste abaixo garante que todos parseiam.
#[must_use]
pub fn packages() -> Vec<PluginPackage> {
    SOURCES
        .iter()
        .map(|(slug, json)| {
            serde_json::from_str(json)
                .unwrap_or_else(|error| panic!("plugin embutido {slug} inválido: {error}"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::plugins::{
        package,
        review::{self, Severity},
        testing,
    };

    #[test]
    fn slug_do_arquivo_e_o_do_manifesto() {
        for ((slug, _), package) in SOURCES.iter().zip(packages()) {
            assert_eq!(*slug, package.manifest.slug);
        }
    }

    #[tokio::test]
    async fn todo_embutido_valida_e_passa_nos_proprios_testes() {
        for package in packages() {
            assert!(
                package::validate(&package).is_empty(),
                "{}: {:?}",
                package.manifest.slug,
                package::validate(&package)
            );
            let report = testing::run_unit_tests(&package).await;
            assert!(report.passed, "{}: {report:#?}", package.manifest.slug);
        }
    }

    #[test]
    fn todo_embutido_sai_da_revisao_com_risco_baixo() {
        for package in packages() {
            let report = review::build_report(&package, Err("sem IA no teste".into()));
            assert!(
                report.findings.iter().all(|f| f.severity <= Severity::Low)
                    || report.findings.iter().all(|f| f.severity == Severity::Info),
                "{}: {:?}",
                package.manifest.slug,
                report.findings
            );
            assert_eq!(report.risk, Severity::Low, "{}", package.manifest.slug);
        }
    }
}
