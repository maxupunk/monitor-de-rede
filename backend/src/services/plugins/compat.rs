//! Este plugin serve para este equipamento?
//!
//! Quatro respostas, porque "sim/não" esconderia a diferença que importa ao
//! operador:
//!
//! * **validado** — já passou no teste funcional num equipamento de mesmo
//!   sistema, modelo e firmware;
//! * **provável** — as regras do manifesto casam com o que se sabe;
//! * **possível** — nada contradiz, mas falta evidência (sistema não
//!   identificado, firmware ainda não lido). Rodar `detect` resolve;
//! * **incompatível** — alguma regra contradiz o que se sabe.

use regex::Regex;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{manifest::PluginManifest, package::CompatEntry, version};
use crate::{models::devices, services::devices::systems};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum Compat {
    Incompatible,
    Possible,
    Likely,
    Validated,
}

/// O quanto se sabe do sistema do equipamento.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Certainty {
    /// Nada identificou.
    #[default]
    Unknown,
    /// Palpite: o fabricante/modelo do cadastro (é hardware — uma RouterBOARD
    /// pode rodar OpenWrt) ou o Laya. Levanta dúvida, nunca reprova.
    Hint,
    /// O operador declarou ou o equipamento mostrou (SSH, SNMP, um plugin).
    Certain,
}

/// O que se sabe do equipamento.
#[derive(Debug, Clone, Default)]
pub struct DeviceFacts {
    pub platform: String,
    pub certainty: Certainty,
    /// De onde veio o sistema, para a tela explicar.
    pub platform_reason: String,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub firmware: Option<String>,
}

impl DeviceFacts {
    #[must_use]
    pub fn from_device(device: &devices::Model) -> Self {
        let mut facts = Self {
            vendor: non_empty(device.vendor.as_deref()),
            model: non_empty(device.model.as_deref()),
            firmware: non_empty(device.firmware_version.as_deref()),
            ..Self::default()
        };
        facts.set_system(&systems::detect(&systems::Evidence::from_device(device)));
        facts
    }

    /// O sistema de uma detecção, com o quanto ela vale.
    pub fn set_system(&mut self, detection: &systems::Detection) {
        self.platform = detection.system.id.to_string();
        self.platform_reason = detection.reason.clone();
        self.certainty = if detection.source == systems::source::DEFAULT {
            Certainty::Unknown
        } else if systems::source::is_certain(detection.source) {
            Certainty::Certain
        } else {
            Certainty::Hint
        };
    }
}

fn non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

/// O veredito com os porquês, para a tela explicar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub level: Compat,
    pub reasons: Vec<String>,
}

fn regex_matches(pattern: &str, value: &str) -> bool {
    Regex::new(&format!("(?i){pattern}")).is_ok_and(|re| re.is_match(value))
}

fn same(a: Option<&str>, b: Option<&str>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a.trim().eq_ignore_ascii_case(b.trim()),
        _ => false,
    }
}

/// O plugin diz, pelo sistema, que é para este equipamento — e o sistema dele
/// é conhecido. É a prova que liga um plugin sozinho no cadastro; plugin sem
/// regra de sistema (uma página HTTP qualquer) nunca liga sozinho.
#[must_use]
pub fn names_platform(manifest: &PluginManifest, facts: &DeviceFacts) -> bool {
    facts.certainty != Certainty::Unknown
        && manifest
            .matcher
            .platforms
            .iter()
            .any(|platform| platform.eq_ignore_ascii_case(&facts.platform))
}

#[must_use]
pub fn evaluate(
    manifest: &PluginManifest,
    validated: &[CompatEntry],
    facts: &DeviceFacts,
) -> Verdict {
    let rule = &manifest.matcher;
    let mut against = Vec::new();
    let mut missing = Vec::new();

    if !rule.platforms.is_empty() {
        let listed = rule
            .platforms
            .iter()
            .any(|platform| platform.eq_ignore_ascii_case(&facts.platform));
        // Só o sistema certo reprova. Fabricante/modelo descrevem o hardware
        // e o Laya é palpite: discordar deles é dúvida, não incompatibilidade.
        match facts.certainty {
            Certainty::Unknown => missing.push(
                "o sistema do equipamento ainda não foi identificado — use \"Verificar sistema\""
                    .to_owned(),
            ),
            Certainty::Hint if !listed => missing.push(format!(
                "o sistema parece {} ({}), mas isso é palpite — use \"Verificar sistema\" ou defina o sistema no cadastro",
                facts.platform, facts.platform_reason
            )),
            Certainty::Certain if !listed => against.push(format!(
                "o plugin é para {}, e o equipamento é {} ({})",
                rule.platforms.join("/"),
                facts.platform,
                facts.platform_reason
            )),
            _ => {}
        }
    }
    for (label, pattern, value) in [
        ("fabricante", &rule.vendor_regex, &facts.vendor),
        ("modelo", &rule.model_regex, &facts.model),
    ] {
        let Some(pattern) = pattern else { continue };
        match value {
            None => missing.push(format!("o {label} do equipamento não está cadastrado")),
            Some(value) if !regex_matches(pattern, value) => {
                against.push(format!("o {label} `{value}` não casa com `{pattern}`"));
            }
            Some(_) => {}
        }
    }
    if let Some(constraint) = &rule.firmware {
        match &facts.firmware {
            None => missing.push("o firmware ainda não foi lido — rode \"Detectar\"".to_owned()),
            // `SNAPSHOT`, `master`, build próprio: a regra de versão não decide.
            Some(firmware) if !firmware.chars().next().is_some_and(|c| c.is_ascii_digit()) => {
                missing.push(format!(
                    "o firmware `{firmware}` não tem versão numérica — a regra `{constraint}` não decide"
                ));
            }
            Some(firmware) if !version::satisfies(firmware, constraint).unwrap_or(false) => {
                against.push(format!("o firmware {firmware} está fora de `{constraint}`"));
            }
            Some(_) => {}
        }
    }

    if !against.is_empty() {
        return Verdict {
            level: Compat::Incompatible,
            reasons: against,
        };
    }
    let proven = validated.iter().any(|entry| {
        entry.status == "passed"
            && same(entry.platform.as_deref(), Some(&facts.platform))
            && same(entry.model.as_deref(), facts.model.as_deref())
            && (entry.firmware.is_none()
                || facts.firmware.is_none()
                || same(entry.firmware.as_deref(), facts.firmware.as_deref()))
    });
    if proven {
        return Verdict {
            level: Compat::Validated,
            reasons: vec!["já validado em um equipamento igual".to_owned()],
        };
    }
    if missing.is_empty() {
        Verdict {
            level: Compat::Likely,
            reasons: vec!["as regras do manifesto casam com o equipamento".to_owned()],
        }
    } else {
        Verdict {
            level: Compat::Possible,
            reasons: missing,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn so_o_sistema_conhecido_e_citado_liga_sozinho() {
        let manifest: PluginManifest = serde_json::from_value(serde_json::json!({
            "slug": "x", "name": "x", "version": "1.0.0", "transports": ["ssh"],
            "match": { "platforms": ["openwrt"] },
            "actions": [{ "id": "detect", "title": "D", "effect": "read" }]
        }))
        .unwrap();
        let mut facts = DeviceFacts {
            platform: "openwrt".into(),
            certainty: Certainty::Certain,
            ..DeviceFacts::default()
        };
        assert!(names_platform(&manifest, &facts));
        facts.certainty = Certainty::Unknown;
        assert!(!names_platform(&manifest, &facts));
        let mut generic = manifest.clone();
        generic.matcher.platforms.clear();
        facts.certainty = Certainty::Certain;
        assert!(!names_platform(&generic, &facts));
    }
    use crate::services::plugins::manifest::MatchRule;
    use serde_json::json;

    fn manifest(matcher: MatchRule) -> PluginManifest {
        let mut manifest: PluginManifest = serde_json::from_value(json!({
            "slug": "x", "name": "x", "version": "1.0.0", "transports": ["ssh"],
            "actions": [ { "id": "detect", "title": "Detectar", "effect": "read" } ]
        }))
        .unwrap();
        manifest.matcher = matcher;
        manifest
    }

    fn openwrt(firmware: Option<&str>) -> DeviceFacts {
        DeviceFacts {
            platform: "openwrt".into(),
            certainty: Certainty::Certain,
            platform_reason: "definido no cadastro".into(),
            vendor: Some("TP-Link".into()),
            model: Some("Archer C7 v5".into()),
            firmware: firmware.map(str::to_owned),
        }
    }

    fn rule() -> MatchRule {
        MatchRule {
            platforms: vec!["openwrt".into()],
            firmware: Some(">=21.02".into()),
            model_regex: Some("archer".into()),
            ..MatchRule::default()
        }
    }

    #[test]
    fn regras_que_casam_sao_provaveis() {
        let verdict = evaluate(&manifest(rule()), &[], &openwrt(Some("23.05.2")));
        assert_eq!(verdict.level, Compat::Likely);
    }

    #[test]
    fn firmware_desconhecido_e_possivel_e_fora_da_faixa_e_incompativel() {
        assert_eq!(
            evaluate(&manifest(rule()), &[], &openwrt(None)).level,
            Compat::Possible
        );
        assert_eq!(
            evaluate(&manifest(rule()), &[], &openwrt(Some("19.07.10"))).level,
            Compat::Incompatible
        );
    }

    #[test]
    fn firmware_sem_versao_numerica_e_possivel() {
        assert_eq!(
            evaluate(&manifest(rule()), &[], &openwrt(Some("SNAPSHOT"))).level,
            Compat::Possible
        );
    }

    #[test]
    fn outro_sistema_e_incompativel_e_sistema_desconhecido_e_possivel() {
        let mut routeros = openwrt(Some("23.05.2"));
        routeros.platform = "routeros".into();
        assert_eq!(
            evaluate(&manifest(rule()), &[], &routeros).level,
            Compat::Incompatible
        );
        routeros.certainty = Certainty::Unknown;
        assert_eq!(
            evaluate(&manifest(rule()), &[], &routeros).level,
            Compat::Possible
        );
    }

    /// Uma RouterBOARD rodando OpenWrt: o cadastro diz "MikroTik" (hardware),
    /// e isso não pode reprovar o plugin de OpenWrt — só levantar dúvida.
    #[test]
    fn palpite_pelo_hardware_e_duvida_e_nao_incompatibilidade() {
        let mut rb922 = openwrt(Some("25.12.5"));
        rb922.platform = "routeros".into();
        rb922.certainty = Certainty::Hint;
        let verdict = evaluate(&manifest(rule()), &[], &rb922);
        assert_eq!(verdict.level, Compat::Possible, "{verdict:?}");
        assert!(verdict.reasons[0].contains("palpite"));
    }

    #[test]
    fn observacao_do_equipamento_vence_o_fabricante_do_cadastro() {
        let device = crate::models::devices::Model {
            id: 3,
            site_id: None,
            network_id: None,
            parent_id: None,
            ip_address: Some("10.0.0.3".into()),
            name: "HeartOfGold".into(),
            r#type: "router".into(),
            vendor: Some("MikroTik".into()),
            model: Some("RouterBOARD 922UAGS-5HPacD".into()),
            serial_number: None,
            description: None,
            is_monitored: true,
            snmp_enabled: false,
            snmp_community: None,
            snmp_version: None,
            snmp_poll_interval_seconds: 15,
            access_mode: None,
            operating_system: None,
            syslog_server_address: None,
            system_key: None,
            link_interface_id: None,
            link_interface_name: None,
            firmware_version: None,
            observed_os: None,
            observed_os_source: None,
            observed_os_reason: None,
            status: "online".into(),
            last_seen_at: None,
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
        };
        let pelo_hardware = DeviceFacts::from_device(&device);
        assert_eq!(pelo_hardware.platform, "routeros");
        assert_eq!(pelo_hardware.certainty, Certainty::Hint);

        let observado = crate::models::devices::Model {
            observed_os: Some("openwrt".into()),
            observed_os_source: Some(systems::source::PROBE.into()),
            observed_os_reason: Some("o servidor SSH se identifica como `dropbear`".into()),
            ..device
        };
        let facts = DeviceFacts::from_device(&observado);
        assert_eq!(facts.platform, "openwrt");
        assert_eq!(facts.certainty, Certainty::Certain);
        assert!(facts.platform_reason.contains("dropbear"));
    }

    #[test]
    fn validacao_anterior_em_equipamento_igual_vence() {
        let entry = CompatEntry {
            platform: Some("openwrt".into()),
            vendor: None,
            model: Some("archer c7 v5".into()),
            firmware: Some("23.05.2".into()),
            status: "passed".into(),
            validated_at: "2026-09-30T00:00:00Z".into(),
            plugin_version: "1.0.0".into(),
            run_id: None,
        };
        assert_eq!(
            evaluate(&manifest(rule()), &[entry], &openwrt(Some("23.05.2"))).level,
            Compat::Validated
        );
    }

    #[test]
    fn plugin_generico_serve_para_qualquer_um() {
        assert_eq!(
            evaluate(
                &manifest(MatchRule::default()),
                &[],
                &DeviceFacts::default()
            )
            .level,
            Compat::Likely
        );
    }
}
