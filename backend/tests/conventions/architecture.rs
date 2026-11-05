//! Regra de dependência entre áreas (ADR 017), verificada pela suíte.
//!
//! Enquanto o código de uma área não vive no seu próprio crate, o compilador
//! não impede que o núcleo importe a área ou que uma área importe outra. Este
//! teste faz esse papel: lê os `use crate::…` e os caminhos
//! `crate::services::…` de cada módulo de `src/services/` e recusa qualquer
//! dependência proibida que não esteja em [`EXCECOES`]. Uma área que já tem
//! crate próprio conta pelo nome do crate (`netmonitor_servers::…` é a área
//! Servidores); dentro dos crates, quem garante a regra é o `Cargo.toml`.
//!
//! A lista de exceções é o retrato do acoplamento que já existia (ARQ-02 em
//! `docs/debitos_tecnicos.md`). Ela **só diminui**: uma dependência nova fora
//! da regra falha o teste, e uma exceção que deixou de existir também — para
//! ser apagada da lista no mesmo PR que a eliminou.

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

/// Área de cada módulo de `services/`. O que não está aqui é do núcleo.
const AREAS: &[(&str, &str)] = &[
    ("discovery", "network"),
    ("vendors", "network"),
    ("snmp", "network"),
    ("topology", "network"),
    ("link_tests", "network"),
    ("wifi", "network"),
    ("plugins", "network"),
    ("package_mirror", "network"),
    // O crate da área (`netmonitor_network::…`) fora de um módulo conhecido.
    ("network", "network"),
    ("docker", "servers"),
    ("databases", "servers"),
    // O crate da área (`netmonitor_servers::…`).
    ("servers", "servers"),
    ("vpn", "vpn"),
    // Não é área de produto, mas é um crate com a mesma regra: ninguém
    // depende dele, e ele não depende de área (ADR 017, `Classifier`).
    ("ai", "assistant"),
];

/// Dependências proibidas que já existiam quando a regra nasceu.
/// `(módulo de origem, módulo importado)`. Só pode diminuir.
const EXCECOES: &[(&str, &str)] = &[
    ("agent_runtime", "discovery"),
    ("agent_runtime", "docker"),
    ("agent_runtime", "plugins"),
    ("agents", "discovery"),
    ("agents", "plugins"),
    ("devices", "discovery"),
    ("devices", "wifi"),
    ("maintenance", "link_tests"),
    ("monitoring", "link_tests"),
    ("monitoring", "snmp"),
    ("monitoring", "vpn"),
    ("monitoring", "wifi"),
    ("network_tools", "plugins"),
    ("onboarding", "vpn"),
    ("plugins", "ai"),
    ("probes", "discovery"),
    ("syslog", "plugins"),
    ("syslog", "snmp"),
];

/// Crates de área já extraídos e o nome com que contam na regra.
const CRATES_DE_AREA: &[(&str, &str)] = &[
    ("netmonitor_servers::", "servers"),
    ("netmonitor_vpn::", "vpn"),
    // O crate do assistente conta como o módulo `ai` (área `assistant`).
    ("netmonitor_assistant::", "ai"),
];

/// O crate da área Rede, que conta pelo módulo seguinte.
const CRATE_DA_REDE: &str = "netmonitor_network::";

fn area_of(module: &str) -> &'static str {
    AREAS
        .iter()
        .find(|(name, _)| *name == module)
        .map_or("core", |(_, area)| area)
}

/// A dependência fere a regra? Núcleo não importa área; área não importa outra.
fn proibida(origem: &str, destino: &str) -> bool {
    let (de, para) = (area_of(origem), area_of(destino));
    para != "core" && de != para
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("diretório legível") {
        let path = entry.expect("entrada legível").path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Remove comentários de linha e de bloco (o suficiente para não contar
/// caminhos citados em documentação).
fn sem_comentarios(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut in_string = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if c == '\\' {
                if let Some(next) = chars.next() {
                    out.push(next);
                }
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match (c, chars.peek()) {
            ('"', _) => {
                in_string = true;
                out.push(c);
            }
            ('/', Some('/')) => {
                for next in chars.by_ref() {
                    if next == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            ('/', Some('*')) => {
                chars.next();
                let mut prev = ' ';
                for next in chars.by_ref() {
                    if prev == '*' && next == '/' {
                        break;
                    }
                    prev = next;
                }
            }
            _ => out.push(c),
        }
    }
    out
}

/// Os módulos de `services` alcançados por uma árvore de `use crate::…`.
fn modulos_da_arvore(tree: &str, out: &mut BTreeSet<String>) {
    let tree: String = tree.chars().filter(|c| !c.is_whitespace()).collect();
    collect(&tree, &[], out);
}

fn collect(tree: &str, prefix: &[String], out: &mut BTreeSet<String>) {
    if let Some(inner) = tree.strip_prefix('{').and_then(|t| t.strip_suffix('}')) {
        let mut depth = 0;
        let mut start = 0;
        for (i, c) in inner.char_indices() {
            match c {
                '{' => depth += 1,
                '}' => depth -= 1,
                ',' if depth == 0 => {
                    collect(&inner[start..i], prefix, out);
                    start = i + 1;
                }
                _ => {}
            }
        }
        if start < inner.len() {
            collect(&inner[start..], prefix, out);
        }
        return;
    }
    if tree.is_empty() {
        return;
    }
    let (head, rest) = tree.split_once("::").unwrap_or((tree, ""));
    let mut path = prefix.to_vec();
    path.push(head.to_owned());
    if path.len() == 2 && path[0] == "services" {
        out.insert(path[1].clone());
        return;
    }
    if !rest.is_empty() {
        collect(rest, &path, out);
    }
}

fn dependencias(source: &str) -> BTreeSet<String> {
    let code = sem_comentarios(source);
    let mut deps = BTreeSet::new();
    let mut rest = code.as_str();
    while let Some(pos) = rest.find("crate::") {
        let after = &rest[pos + "crate::".len()..];
        let is_use = rest[..pos].trim_end().ends_with("use");
        if is_use {
            let end = after.find(';').unwrap_or(after.len());
            modulos_da_arvore(&after[..end], &mut deps);
        } else if let Some(path) = after.strip_prefix("services::") {
            let name: String = path
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() {
                deps.insert(name);
            }
        }
        rest = after;
    }
    for (prefixo, area) in CRATES_DE_AREA {
        if code.contains(prefixo) {
            deps.insert((*area).to_owned());
        }
    }
    // O crate da Rede conta pelo módulo (`netmonitor_network::snmp::…` é o
    // módulo `snmp`), para as exceções continuarem dizendo o que acoplam.
    let mut resto = code.as_str();
    while let Some(pos) = resto.find(CRATE_DA_REDE) {
        let depois = &resto[pos + CRATE_DA_REDE.len()..];
        let nome: String = depois
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        let modulo = AREAS
            .iter()
            .find(|(modulo, area)| *modulo == nome && *area == "network")
            .map_or("network", |(modulo, _)| modulo);
        deps.insert(modulo.to_owned());
        resto = depois;
    }
    deps
}

/// `(origem, destino)` de toda dependência proibida em `src/services/`.
fn violacoes() -> BTreeSet<(String, String)> {
    let services = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/services");
    let mut files = Vec::new();
    rust_files(&services, &mut files);
    let mut found = BTreeSet::new();
    for file in files {
        let relative = file.strip_prefix(&services).expect("dentro de services");
        let first = relative
            .components()
            .next()
            .expect("caminho não vazio")
            .as_os_str()
            .to_string_lossy()
            .trim_end_matches(".rs")
            .to_owned();
        if first == "mod" {
            continue;
        }
        let source = fs::read_to_string(&file).expect("arquivo legível");
        for dep in dependencias(&source) {
            if dep != first && proibida(&first, &dep) {
                found.insert((first.clone(), dep));
            }
        }
    }
    found
}

#[test]
fn nenhuma_dependencia_nova_entre_areas() {
    let permitidas: BTreeSet<(String, String)> = EXCECOES
        .iter()
        .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
        .collect();
    let atuais = violacoes();

    let novas: Vec<_> = atuais.difference(&permitidas).collect();
    assert!(
        novas.is_empty(),
        "dependências novas que ferem a regra do ADR 017 (núcleo não importa área; \
         área não importa outra) — resolva por um trait do núcleo: {novas:?}"
    );

    let resolvidas: Vec<_> = permitidas.difference(&atuais).collect();
    assert!(
        resolvidas.is_empty(),
        "estas exceções não existem mais — apague-as de EXCECOES em \
         tests/architecture.rs (a lista só diminui): {resolvidas:?}"
    );
}

#[test]
fn o_detector_enxerga_use_agrupado_e_caminho_inline() {
    let source = r"
        // use crate::services::docker::Fake;
        use crate::{
            models::devices,
            services::{
                shared::errors::AppResult,
                vpn::{self, peers},
            },
        };
        fn f() { crate::services::wifi::collector::run(); }
        fn g() { netmonitor_servers::docker::enabled(); }
        const DOC: &str = /* crate::services::snmp */ 1;
    ";
    let deps = dependencias(source);
    assert!(deps.contains("vpn"));
    assert!(deps.contains("servers"), "crate de área conta pelo nome");
    assert!(
        dependencias("fn h() { netmonitor_vpn::probe_is_external(); }").contains("vpn"),
        "o crate da VPN também"
    );
    assert!(
        dependencias("use netmonitor_assistant::settings;").contains("ai"),
        "e o do assistente, que conta como o módulo `ai`"
    );
    assert!(
        dependencias("use netmonitor_network::snmp::client;").contains("snmp"),
        "o da Rede conta pelo módulo"
    );
    assert!(
        dependencias("use netmonitor_network::NetworkPolicy;").contains("network"),
        "e, fora de um módulo conhecido, pela área"
    );
    assert!(deps.contains("shared"));
    assert!(deps.contains("wifi"));
    assert!(!deps.contains("docker"), "comentário não conta");
    assert!(!deps.contains("snmp"), "comentário de bloco não conta");
}

#[test]
fn regra_da_area() {
    assert!(proibida("monitoring", "docker"), "núcleo não importa área");
    assert!(proibida("docker", "vpn"), "área não importa outra");
    assert!(!proibida("docker", "shared"), "área importa o núcleo");
    assert!(
        !proibida("discovery", "snmp"),
        "dentro da mesma área é livre"
    );
}
