//! Que caminhos o espelho serve. Puro: caminho → URL do repositório oficial.

/// O repositório oficial do OpenWrt — a única origem do espelho.
pub const UPSTREAM: &str = "https://downloads.openwrt.org";

const MAX_PATH_CHARS: usize = 512;

/// Raízes do repositório: versões lançadas e as de desenvolvimento.
const ROOTS: [&str; 2] = ["releases", "snapshots"];

/// Índices e pacotes do `apk` (25.x) e do `opkg` (≤ 24.10).
const FILE_SUFFIXES: [&str; 3] = [".adb", ".apk", ".ipk"];
const FILE_NAMES: [&str; 4] = ["Packages", "Packages.gz", "Packages.sig", "index.json"];

fn valid_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment != "."
        && segment != ".."
        && segment
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._+~-".contains(c))
}

fn valid_file(name: &str) -> bool {
    FILE_NAMES.contains(&name) || FILE_SUFFIXES.iter().any(|suffix| name.ends_with(suffix))
}

/// URL de origem para `path` (`releases/25.12.5/packages/…/packages.adb`), ou
/// `None` quando o caminho não é um índice ou pacote do repositório oficial.
#[must_use]
pub fn upstream_url(path: &str) -> Option<String> {
    if path.len() > MAX_PATH_CHARS {
        return None;
    }
    let segments: Vec<&str> = path.split('/').collect();
    let (root, file) = (segments.first()?, segments.last()?);
    if segments.len() < 3
        || !ROOTS.contains(root)
        || !segments.iter().all(|segment| valid_segment(segment))
        || !valid_file(file)
    {
        return None;
    }
    Some(format!("{UPSTREAM}/{path}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indices_e_pacotes_do_apk_e_do_opkg_passam() {
        for path in [
            "releases/25.12.5/packages/aarch64_cortex-a53/base/packages.adb",
            "releases/25.12.5/packages/aarch64_cortex-a53/packages/iperf3-3.20-r1.apk",
            "releases/24.10.0/targets/x86/64/packages/Packages.gz",
            "releases/24.10.0/packages/x86_64/base/Packages.sig",
            "releases/24.10.0/packages/x86_64/packages/iperf3_3.17.1-r3_x86_64.ipk",
            "snapshots/packages/x86_64/base/packages.adb",
        ] {
            assert_eq!(
                upstream_url(path).as_deref(),
                Some(format!("{UPSTREAM}/{path}").as_str()),
                "{path}"
            );
        }
    }

    #[test]
    fn fora_do_repositorio_ou_com_truque_de_caminho_e_recusado() {
        for path in [
            "",
            "releases",
            "releases/25.12.5",
            "releases/../etc/passwd",
            "releases/25.12.5/../../x.apk",
            "releases/25.12.5//x.apk",
            "releases/25.12.5/packages/./x.apk",
            "releases/25.12.5/index.html",
            "releases/25.12.5/sha256sums",
            "outra-coisa/25.12.5/packages/x.apk",
            "releases/25.12.5/packages/x.apk?y=1",
            "releases/25.12.5/packages/x%2F..%2Fy.apk",
            "releases/25.12.5/packages/x y.apk",
        ] {
            assert_eq!(upstream_url(path), None, "{path}");
        }
        assert_eq!(
            upstream_url(&format!("releases/{}/x.apk", "a".repeat(600))),
            None
        );
    }
}
