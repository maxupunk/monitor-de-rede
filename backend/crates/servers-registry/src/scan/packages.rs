//! Os pacotes de uma imagem (RF-22/23).
//!
//! Fonte primária: o SBOM SPDX que o BuildKit anexa à imagem (`--sbom`),
//! com um `purl` por pacote. Sem ele (push manual), lê-se só o que diz quais
//! pacotes do sistema estão instalados — `var/lib/dpkg/status`,
//! `lib/apk/db/installed` e o `os-release` —, com teto de tamanho; nada da
//! imagem é executado (RS-08).

use std::collections::BTreeSet;

use serde_json::Value;

use crate::text::percent_decode;

/// Origem da lista de pacotes gravada na análise.
pub const SPDX_HINT: &str = "sbom";

/// Um pacote com o ecossistema no formato do OSV (`Debian:12`, `npm`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Package {
    pub ecosystem: String,
    pub name: String,
    pub version: String,
}

impl Package {
    /// O diretório do OSV com a base desse ecossistema (`Debian`, `npm`).
    #[must_use]
    pub fn feed(&self) -> &str {
        self.ecosystem.split(':').next().unwrap_or(&self.ecosystem)
    }
}

/// `debian-12` / `12.4` → `Debian:12`; `3.19.1` → `Alpine:v3.19`.
fn distro_ecosystem(kind: &str, distro: Option<&str>) -> Option<String> {
    let distro = distro.unwrap_or_default();
    let version = distro.rsplit('-').next().unwrap_or(distro);
    match kind {
        "debian" => {
            let major = version
                .split('.')
                .next()
                .filter(|major| !major.is_empty())?;
            Some(format!("Debian:{major}"))
        }
        "ubuntu" => {
            let mut parts = version.split('.');
            let (major, minor) = (parts.next()?, parts.next()?);
            Some(format!("Ubuntu:{major}.{minor}"))
        }
        "alpine" => {
            let mut parts = version.trim_start_matches('v').split('.');
            let (major, minor) = (parts.next()?, parts.next()?);
            Some(format!("Alpine:v{major}.{minor}"))
        }
        _ => None,
    }
}

/// Um `purl` (`pkg:deb/debian/openssl@3.0.11?distro=debian-12`).
#[must_use]
pub fn from_purl(purl: &str) -> Option<Package> {
    let rest = purl.strip_prefix("pkg:")?;
    let (rest, qualifiers) = rest.split_once('?').unwrap_or((rest, ""));
    let rest = rest.split('#').next().unwrap_or(rest);
    let (path, version) = rest.rsplit_once('@')?;
    let version = percent_decode(version);
    let (kind, path) = path.split_once('/')?;
    let qualifier = |key: &str| {
        qualifiers
            .split('&')
            .find_map(|pair| pair.strip_prefix(key)?.strip_prefix('='))
            .map(percent_decode)
    };
    let segments: Vec<String> = path.split('/').map(percent_decode).collect();
    let last = segments.last()?.clone();
    let (ecosystem, name) = match kind {
        "deb" => {
            let vendor = segments.first().map(String::as_str).unwrap_or("debian");
            (
                distro_ecosystem(vendor, qualifier("distro").as_deref())?,
                last,
            )
        }
        "apk" => (
            distro_ecosystem("alpine", qualifier("distro").as_deref())?,
            last,
        ),
        "npm" => ("npm".to_string(), segments.join("/")),
        "pypi" => (
            "PyPI".to_string(),
            last.to_ascii_lowercase().replace('_', "-"),
        ),
        "cargo" => ("crates.io".to_string(), last),
        "golang" => ("Go".to_string(), segments.join("/")),
        "maven" => ("Maven".to_string(), segments.join(":")),
        "gem" => ("RubyGems".to_string(), last),
        "nuget" => ("NuGet".to_string(), last),
        "composer" => ("Packagist".to_string(), segments.join("/")),
        _ => return None,
    };
    (!name.is_empty() && !version.is_empty()).then_some(Package {
        ecosystem,
        name,
        version,
    })
}

/// Os pacotes de um documento SPDX (JSON).
#[must_use]
pub fn from_spdx(document: &Value) -> Vec<Package> {
    let mut found = BTreeSet::new();
    let packages = document
        .get("predicate")
        .unwrap_or(document)
        .get("packages")
        .and_then(Value::as_array);
    for package in packages.into_iter().flatten() {
        let refs = package.get("externalRefs").and_then(Value::as_array);
        for reference in refs.into_iter().flatten() {
            if reference.get("referenceType").and_then(Value::as_str) != Some("purl") {
                continue;
            }
            if let Some(parsed) = reference
                .get("referenceLocator")
                .and_then(Value::as_str)
                .and_then(from_purl)
            {
                found.insert(parsed);
            }
        }
    }
    found.into_iter().collect()
}

/// `ID` e `VERSION_ID` do `os-release`.
#[must_use]
pub fn os_release(content: &str) -> Option<(String, String)> {
    let field = |key: &str| {
        content.lines().find_map(|line| {
            line.strip_prefix(key)?
                .strip_prefix('=')
                .map(|value| value.trim().trim_matches('"').to_string())
        })
    };
    Some((field("ID")?, field("VERSION_ID")?))
}

/// Pacotes instalados do `var/lib/dpkg/status`.
#[must_use]
pub fn from_dpkg(status: &str, ecosystem: &str) -> Vec<Package> {
    status
        .split("\n\n")
        .filter_map(|stanza| {
            let field = |key: &str| {
                stanza
                    .lines()
                    .find_map(|line| line.strip_prefix(key)?.strip_prefix(": "))
                    .map(str::trim)
            };
            if !field("Status")?.ends_with("installed") {
                return None;
            }
            // O OSV do Debian indexa pelo pacote-fonte.
            let name = field("Source")
                .map(|source| source.split_whitespace().next().unwrap_or(source))
                .or_else(|| field("Package"))?;
            Some(Package {
                ecosystem: ecosystem.to_string(),
                name: name.to_string(),
                version: field("Version")?.to_string(),
            })
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Pacotes instalados do `lib/apk/db/installed` (`P:` nome, `V:` versão,
/// `o:` origem).
#[must_use]
pub fn from_apk(installed: &str, ecosystem: &str) -> Vec<Package> {
    installed
        .split("\n\n")
        .filter_map(|stanza| {
            let field = |key: &str| stanza.lines().find_map(|line| line.strip_prefix(key));
            let name = field("o:").or_else(|| field("P:"))?;
            Some(Package {
                ecosystem: ecosystem.to_string(),
                name: name.trim().to_string(),
                version: field("V:")?.trim().to_string(),
            })
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// O ecossistema do sistema da imagem a partir do `os-release`.
#[must_use]
pub fn system_ecosystem(id: &str, version_id: &str) -> Option<String> {
    distro_ecosystem(id, Some(version_id))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn purls_dos_ecossistemas() {
        let deb = from_purl("pkg:deb/debian/openssl@3.0.11-1~deb12u2?arch=amd64&distro=debian-12")
            .unwrap();
        assert_eq!(deb.ecosystem, "Debian:12");
        assert_eq!(deb.version, "3.0.11-1~deb12u2");
        let apk = from_purl("pkg:apk/alpine/musl@1.2.4-r2?arch=x86_64&distro=3.19.1").unwrap();
        assert_eq!(apk.ecosystem, "Alpine:v3.19");
        let npm = from_purl("pkg:npm/%40vue/reactivity@3.4.21").unwrap();
        assert_eq!(
            (npm.name.as_str(), npm.ecosystem.as_str()),
            ("@vue/reactivity", "npm")
        );
        let go = from_purl("pkg:golang/golang.org/x/net@v0.17.0").unwrap();
        assert_eq!(go.name, "golang.org/x/net");
        assert_eq!(from_purl("pkg:pypi/Django@4.2").unwrap().name, "django");
        assert!(from_purl("pkg:generic/foo@1").is_none());
        assert!(from_purl("não é purl").is_none());
    }

    #[test]
    fn spdx_do_buildkit() {
        let doc = json!({"predicate": {"packages": [
            {"name": "openssl", "externalRefs": [
                {"referenceType": "purl", "referenceLocator": "pkg:deb/debian/openssl@3.0.11-1?distro=debian-12"}]},
            {"name": "sem purl"}
        ]}});
        let found = from_spdx(&doc);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].feed(), "Debian");
    }

    #[test]
    fn dpkg_e_apk_instalados() {
        let status = "Package: libssl3\nStatus: install ok installed\nSource: openssl\nVersion: 3.0.11-1\n\n\
                      Package: removido\nStatus: deinstall ok config-files\nVersion: 1\n";
        let found = from_dpkg(status, "Debian:12");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "openssl");
        let apk = "P:musl\nV:1.2.4-r2\no:musl\n\nP:libcrypto3\nV:3.1.4-r5\no:openssl\n";
        let found = from_apk(apk, "Alpine:v3.19");
        assert_eq!(
            found.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
            ["musl", "openssl"]
        );
        assert_eq!(
            os_release("ID=debian\nVERSION_ID=\"12\"\n"),
            Some(("debian".into(), "12".into()))
        );
        assert_eq!(
            system_ecosystem("alpine", "3.19.1").as_deref(),
            Some("Alpine:v3.19")
        );
    }
}
