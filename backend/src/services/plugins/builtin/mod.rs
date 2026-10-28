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
    // Papéis no teste de enlace (ADR 015 §3/§4).
    (
        "mikrotik-routeros",
        include_str!("mikrotik-routeros.nmplugin.json"),
    ),
    (
        "openwrt-link-test",
        include_str!("openwrt-link-test.nmplugin.json"),
    ),
    (
        "linux-link-test",
        include_str!("linux-link-test.nmplugin.json"),
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
    use std::sync::{Arc, Mutex};

    use tokio_util::sync::CancellationToken;

    use super::*;
    use crate::services::plugins::{
        gate::AllowAll,
        package, params,
        review::{self, Severity},
        runtime::{
            self, AccessProfile, Credentials, DeviceInfo, ExecutionContext, Extras, Limits,
            ProgressFlow, ProgressSample,
        },
        testing,
        transport::{fake::FakeTransport, Login},
    };

    #[test]
    fn slug_do_arquivo_e_o_do_manifesto() {
        for ((slug, _), package) in SOURCES.iter().zip(packages()) {
            assert_eq!(*slug, package.manifest.slug);
        }
    }

    #[tokio::test]
    async fn todo_embutido_valida_e_passa_nos_proprios_testes() {
        let mut hints = Vec::new();
        for package in packages() {
            assert!(
                package::validate(&package).is_empty(),
                "{}: {:?}",
                package.manifest.slug,
                package::validate(&package)
            );
            let report = testing::run_unit_tests(&package).await;
            assert!(report.passed, "{}: {report:#?}", package.manifest.slug);
            hints.extend(
                report
                    .hints
                    .iter()
                    .map(|hint| format!("{} {}: {}", package.manifest.slug, hint.at, hint.message)),
            );
        }
        // Os embutidos são o exemplo que a IA copia: tela sem sugestão.
        assert!(
            hints.is_empty(),
            "{}",
            hints.join(
                "
"
            )
        );
    }

    /// As amostras ao vivo que o `progress` de um caso de teste produz.
    async fn samples_of(slug: &str, case: &str) -> Vec<(ProgressFlow, f64)> {
        let package = packages()
            .into_iter()
            .find(|package| package.manifest.slug == slug)
            .expect("embutido existe");
        let test = package
            .tests
            .unit
            .iter()
            .find(|test| test.name.as_deref() == Some(case))
            .expect("caso existe");
        let action = package.manifest.action(&test.action).expect("ação existe");
        let params = params::validate(action.params.as_ref(), &test.params).expect("parâmetros");
        let samples = Arc::new(Mutex::new(Vec::new()));
        let sink = samples.clone();
        let context = ExecutionContext {
            extras: Extras {
                library: testing::builtin_library(&package),
                ..Extras::default()
            },
            device: DeviceInfo {
                ip: "192.0.2.1".into(),
                platform: package.manifest.matcher.platforms[0].clone(),
                ..DeviceInfo::default()
            },
            transports: package.manifest.transports.clone(),
            action_effect: action.effect,
            reason: None,
            credentials: Credentials {
                ssh: Some(AccessProfile {
                    login: Login::with_password("root", None),
                    port: 22,
                    https: false,
                }),
                ..Credentials::default()
            },
            transport: Arc::new(FakeTransport::new(test.fixtures.clone())),
            gate: Arc::new(AllowAll),
            cancel: CancellationToken::new(),
            observer: None,
            progress: Some(Arc::new(move |sample: &ProgressSample| {
                sink.lock().unwrap().push((sample.flow, sample.bps));
            })),
            limits: Limits::default(),
        };
        let outcome = runtime::execute(&package.script, &action.id, params, context).await;
        assert!(
            outcome.output.is_ok(),
            "{slug} {case}: {:?}",
            outcome.output
        );
        let collected = samples.lock().unwrap().clone();
        collected
    }

    fn assert_samples(actual: &[(ProgressFlow, f64)], expected: &[(ProgressFlow, f64)]) {
        assert_eq!(actual.len(), expected.len(), "{actual:?}");
        for ((flow, bps), (want_flow, want_bps)) in actual.iter().zip(expected) {
            assert_eq!(flow, want_flow, "{actual:?}");
            assert!((bps - want_bps).abs() < 1.0, "{actual:?}");
        }
    }

    #[tokio::test]
    async fn papeis_de_teste_de_enlace_emitem_andamento_ao_vivo() {
        use ProgressFlow::{Rx, Tx};
        // RouterOS: `tx-current`/`rx-current` de cada quadro, nunca as médias.
        assert_samples(
            &samples_of(
                "mikrotik-routeros",
                "mede os dois sentidos com autenticação e limite",
            )
            .await,
            &[(Tx, 90.1e6), (Rx, 1.1e9), (Tx, 94.5e6), (Rx, 1.3e9)],
        );
        // iperf3: as linhas de 1 s de cada sentido; o resumo sender/receiver
        // fica de fora.
        let iperf3 = [(Tx, 94e6), (Tx, 95.5e6), (Rx, 84e6), (Rx, 85e6)];
        for slug in ["openwrt-link-test", "linux-link-test"] {
            assert_samples(
                &samples_of(
                    slug,
                    "iperf3 nos dois sentidos, com limite (envio e depois -R)",
                )
                .await,
                &iperf3,
            );
            // Com vários fluxos, só a soma.
            assert_samples(
                &samples_of(slug, "iperf3 com dois fluxos usa as linhas [SUM]").await,
                &[(Tx, 94e6)],
            );
        }
        // mikrotik_btest: um quadro por `\r`, com o número impresso.
        assert_samples(
            &samples_of("openwrt-link-test", "btest só recebendo").await,
            &[(Rx, 10e6), (Rx, 12e6), (Rx, 11e6)],
        );
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
