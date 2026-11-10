//! Um registro do OSV contra um pacote: afeta? qual a severidade? qual
//! versão corrige? (RF-25)
//!
//! A severidade vem, nesta ordem, do que o próprio aviso classifica
//! (`database_specific.severity` do GitHub, `urgency` do Debian) e do vetor
//! CVSS v3 (calculado aqui, pela fórmula da especificação). Sem nada disso,
//! é `unknown` — que só avisa, nunca bloqueia.

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::{packages::Package, version::Scheme};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(
    export,
    export_to = "../../frontend/src/bindings/",
    rename = "RegistrySeverity"
)]
pub enum Severity {
    Unknown,
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }

    #[must_use]
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "critical" => Self::Critical,
            "high" | "important" => Self::High,
            "medium" | "moderate" => Self::Medium,
            "low" | "negligible" | "unimportant" => Self::Low,
            _ => Self::Unknown,
        }
    }

    fn of_score(score: f64) -> Self {
        match score {
            s if s >= 9.0 => Self::Critical,
            s if s >= 7.0 => Self::High,
            s if s >= 4.0 => Self::Medium,
            s if s > 0.0 => Self::Low,
            _ => Self::Unknown,
        }
    }
}

/// Nota base de um vetor CVSS v3.x (`CVSS:3.1/AV:N/AC:L/…`).
#[must_use]
pub fn cvss3_score(vector: &str) -> Option<f64> {
    if !vector.starts_with("CVSS:3") {
        return None;
    }
    let metric = |key: &str| {
        vector
            .split('/')
            .find_map(|part| part.strip_prefix(key)?.strip_prefix(':'))
    };
    let changed = metric("S")? == "C";
    let av = match metric("AV")? {
        "N" => 0.85,
        "A" => 0.62,
        "L" => 0.55,
        "P" => 0.2,
        _ => return None,
    };
    let ac = match metric("AC")? {
        "L" => 0.77,
        "H" => 0.44,
        _ => return None,
    };
    let pr = match (metric("PR")?, changed) {
        ("N", _) => 0.85,
        ("L", false) => 0.62,
        ("L", true) => 0.68,
        ("H", false) => 0.27,
        ("H", true) => 0.5,
        _ => return None,
    };
    let ui = match metric("UI")? {
        "N" => 0.85,
        "R" => 0.62,
        _ => return None,
    };
    let cia = |key: &str| match metric(key) {
        Some("H") => Some(0.56),
        Some("L") => Some(0.22),
        Some("N") => Some(0.0),
        _ => None,
    };
    let (c, i, a) = (cia("C")?, cia("I")?, cia("A")?);
    let iss = 1.0 - (1.0 - c) * (1.0 - i) * (1.0 - a);
    let impact = if changed {
        7.52 * (iss - 0.029) - 3.25 * (iss - 0.02_f64).powi(15)
    } else {
        6.42 * iss
    };
    if impact <= 0.0 {
        return Some(0.0);
    }
    let exploitability = 8.22 * av * ac * pr * ui;
    let base = if changed {
        (1.08 * (impact + exploitability)).min(10.0)
    } else {
        (impact + exploitability).min(10.0)
    };
    // "Roundup" da especificação: para cima, uma casa.
    Some((base * 10.0 - 1e-9).ceil() / 10.0)
}

/// A severidade de um aviso para um ecossistema.
#[must_use]
pub fn severity(data: &Value, ecosystem: &str) -> Severity {
    let mut best = Severity::Unknown;
    if let Some(declared) = data
        .pointer("/database_specific/severity")
        .and_then(Value::as_str)
    {
        best = best.max(Severity::parse(declared));
    }
    for affected in data
        .get("affected")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if !same_ecosystem(
            affected
                .pointer("/package/ecosystem")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            ecosystem,
        ) {
            continue;
        }
        for key in [
            "/ecosystem_specific/urgency",
            "/ecosystem_specific/severity",
            "/database_specific/severity",
        ] {
            if let Some(level) = affected.pointer(key).and_then(Value::as_str) {
                best = best.max(Severity::parse(level));
            }
        }
    }
    if best == Severity::Unknown {
        for entry in data
            .get("severity")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(score) = entry
                .get("score")
                .and_then(Value::as_str)
                .and_then(cvss3_score)
            {
                best = best.max(Severity::of_score(score));
            }
        }
    }
    best
}

/// `Ubuntu:22.04:LTS` vale para `Ubuntu:22.04`; o resto é igualdade.
fn same_ecosystem(advisory: &str, package: &str) -> bool {
    advisory == package || advisory.starts_with(&format!("{package}:"))
}

/// O que um aviso diz de um pacote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub fixed: Option<String>,
}

/// Se o aviso afeta a versão instalada (e qual versão corrige).
#[must_use]
pub fn affects(data: &Value, package: &Package) -> Option<Hit> {
    let scheme = Scheme::of(&package.ecosystem);
    let cmp = |a: &str, b: &str| scheme.compare(a, b);
    for affected in data.get("affected").and_then(Value::as_array)? {
        let ecosystem = affected
            .pointer("/package/ecosystem")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let name = affected
            .pointer("/package/name")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !same_ecosystem(ecosystem, &package.ecosystem) || name != package.name {
            continue;
        }
        let listed = affected
            .get("versions")
            .and_then(Value::as_array)
            .is_some_and(|versions| {
                versions
                    .iter()
                    .filter_map(Value::as_str)
                    .any(|version| version == package.version)
            });
        let mut fixed_after = None;
        let mut in_range = false;
        for range in affected
            .get("ranges")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if range.get("type").and_then(Value::as_str) == Some("GIT") {
                continue;
            }
            let mut affected_now = false;
            for event in range
                .get("events")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if let Some(introduced) = event.get("introduced").and_then(Value::as_str) {
                    if introduced == "0" || cmp(&package.version, introduced) != Ordering::Less {
                        affected_now = true;
                    }
                } else if let Some(fixed) = event.get("fixed").and_then(Value::as_str) {
                    if cmp(&package.version, fixed) == Ordering::Less {
                        if affected_now {
                            fixed_after.get_or_insert_with(|| fixed.to_string());
                        }
                    } else {
                        affected_now = false;
                    }
                } else if let Some(last) = event.get("last_affected").and_then(Value::as_str) {
                    if cmp(&package.version, last) == Ordering::Greater {
                        affected_now = false;
                    }
                } else if let Some(limit) = event.get("limit").and_then(Value::as_str) {
                    if cmp(&package.version, limit) != Ordering::Less {
                        affected_now = false;
                    }
                }
            }
            in_range |= affected_now;
        }
        if listed || in_range {
            return Some(Hit { fixed: fixed_after });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn package(version: &str) -> Package {
        Package {
            ecosystem: "Debian:12".into(),
            name: "openssl".into(),
            version: version.into(),
        }
    }

    #[test]
    fn cvss_da_especificacao() {
        assert_eq!(
            cvss3_score("CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H"),
            Some(9.8)
        );
        assert_eq!(
            cvss3_score("CVSS:3.1/AV:N/AC:L/PR:N/UI:R/S:C/C:L/I:L/A:N"),
            Some(6.1)
        );
        assert_eq!(
            cvss3_score("CVSS:3.0/AV:L/AC:L/PR:L/UI:N/S:U/C:N/I:N/A:N"),
            Some(0.0)
        );
        assert_eq!(cvss3_score("CVSS:4.0/AV:N"), None);
    }

    #[test]
    fn faixa_com_correcao() {
        let data = json!({"affected": [{
            "package": {"ecosystem": "Debian:12", "name": "openssl"},
            "ranges": [{"type": "ECOSYSTEM", "events": [{"introduced": "0"}, {"fixed": "3.0.13-1~deb12u1"}]}],
            "ecosystem_specific": {"urgency": "high"}
        }]});
        let hit = affects(&data, &package("3.0.11-1~deb12u2")).unwrap();
        assert_eq!(hit.fixed.as_deref(), Some("3.0.13-1~deb12u1"));
        assert!(affects(&data, &package("3.0.13-1~deb12u1")).is_none());
        assert_eq!(severity(&data, "Debian:12"), Severity::High);
        let other = Package {
            ecosystem: "Debian:11".into(),
            ..package("1")
        };
        assert!(affects(&data, &other).is_none());
    }

    #[test]
    fn lista_de_versoes_last_affected_e_cvss() {
        let data = json!({
            "affected": [{
                "package": {"ecosystem": "npm", "name": "lodash"},
                "versions": ["4.17.20"],
                "ranges": [{"type": "SEMVER", "events": [{"introduced": "4.0.0"}, {"last_affected": "4.17.15"}]}]
            }],
            "severity": [{"type": "CVSS_V3", "score": "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H"}]
        });
        let pkg = |v: &str| Package {
            ecosystem: "npm".into(),
            name: "lodash".into(),
            version: v.into(),
        };
        assert!(affects(&data, &pkg("4.17.10")).is_some());
        assert!(affects(&data, &pkg("4.17.20")).is_some());
        assert!(affects(&data, &pkg("4.17.21")).is_none());
        assert_eq!(severity(&data, "npm"), Severity::Critical);
        let ghsa = json!({"database_specific": {"severity": "MODERATE"}});
        assert_eq!(severity(&ghsa, "npm"), Severity::Medium);
    }
}
