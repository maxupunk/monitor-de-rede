//! Resolve o gerador correto para cada perfil de equipamento (§8.10.5).
//!
//! Quem consome depende apenas da abstração [`VpnProfileGenerator`] (DIP).

use std::sync::OnceLock;

use serde::Serialize;
use ts_rs::TS;

use super::{
    contract::VpnProfileGenerator,
    mikrotik::MikrotikProfileGenerator,
    openwrt::OpenWrtProfileGenerator,
    wg_conf::{linux_generator, mobile_generator, windows_generator},
};
use crate::services::shared::errors::{AppError, AppResult};

/// Card do wizard: o que a tela precisa saber sobre cada perfil.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ProfileCard {
    pub profile: String,
    pub label: String,
    pub icon: String,
    pub supports_qr_code: bool,
}

/// Os cinco geradores, na ordem em que aparecem no wizard.
fn generators() -> &'static [Box<dyn VpnProfileGenerator>] {
    static GENERATORS: OnceLock<Vec<Box<dyn VpnProfileGenerator>>> = OnceLock::new();
    GENERATORS.get_or_init(|| {
        vec![
            Box::new(MikrotikProfileGenerator),
            Box::new(OpenWrtProfileGenerator),
            Box::new(linux_generator()),
            Box::new(windows_generator()),
            Box::new(mobile_generator()),
        ]
    })
}

#[must_use]
pub fn has(profile: &str) -> bool {
    generators()
        .iter()
        .any(|generator| generator.profile() == profile)
}

/// # Errors
///
/// Devolve 400 com a mensagem que o wizard exibe quando o perfil não existe.
pub fn resolve(profile: &str) -> AppResult<&'static dyn VpnProfileGenerator> {
    generators()
        .iter()
        .find(|generator| generator.profile() == profile)
        .map(AsRef::as_ref)
        .ok_or_else(|| {
            AppError::business_rule(format!("Perfil de equipamento não suportado: {profile}"))
        })
}

/// Catálogo exibido nos cards do wizard.
#[must_use]
pub fn list() -> Vec<ProfileCard> {
    generators()
        .iter()
        .map(|generator| ProfileCard {
            profile: generator.profile().to_string(),
            label: generator.label().to_string(),
            icon: generator.icon().to_string(),
            supports_qr_code: generator.supports_qr_code(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::contract::tests::contexto;
    use super::*;

    #[test]
    fn os_cinco_perfis_estao_registrados() {
        let cards = list();
        assert_eq!(cards.len(), 5);
        let profiles: Vec<&str> = cards.iter().map(|card| card.profile.as_str()).collect();
        assert_eq!(
            profiles,
            vec!["mikrotik", "openwrt", "linux", "windows", "mobile"]
        );
    }

    #[test]
    fn perfil_desconhecido_e_erro_de_negocio_e_nao_panico() {
        assert!(!has("cisco"));
        // `expect_err` exigiria `Debug` no `dyn VpnProfileGenerator`; o `Ok`
        // aqui é um objeto de trait, então o casamento é explícito.
        let Err(error) = resolve("cisco") else {
            panic!("um perfil inexistente não pode resolver");
        };
        assert_eq!(error.status(), axum::http::StatusCode::BAD_REQUEST);
        assert!(error.to_string().contains("não suportado"));
    }

    #[test]
    fn todo_perfil_registrado_gera_artefato_e_dicas_de_firewall() {
        let context = contexto();
        for card in list() {
            let generator = resolve(&card.profile).expect("perfil registrado");
            let artifact = generator.generate(&context);
            assert_eq!(artifact.profile, card.profile);
            assert!(
                !artifact.content.is_empty(),
                "{} sem conteúdo",
                card.profile
            );
            assert!(!artifact.summary.is_empty(), "{} sem resumo", card.profile);
            assert!(
                !generator.firewall_hints(&context).is_empty(),
                "{} sem dicas de firewall",
                card.profile
            );
        }
    }

    #[test]
    fn nenhum_artefato_vaza_a_chave_privada_no_resumo() {
        let context = contexto();
        for card in list() {
            let artifact = resolve(&card.profile).unwrap().generate(&context);
            let summary = serde_json::to_string(&artifact.summary).unwrap();
            assert!(
                !summary.contains(&context.client_private_key),
                "{} expôs a chave privada no resumo",
                card.profile
            );
        }
    }

    #[test]
    fn so_um_perfil_suporta_qr_code() {
        assert_eq!(
            list().iter().filter(|card| card.supports_qr_code).count(),
            1
        );
    }
}
