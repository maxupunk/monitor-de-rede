//! Regra de dependência entre áreas (ADR 017): o que o compilador não vê.
//!
//! Desde a Fase 10, cada área mora no seu crate e quem garante a regra é o
//! `Cargo.toml`: um `use` de outra área nem compila. Ficam de fora do
//! compilador três coisas, e é delas que este teste cuida:
//!
//! 1. **O próprio `Cargo.toml`.** Uma dependência proibida acrescentada nele
//!    compilaria — o núcleo (`core`, `nucleo`) passaria a depender de uma área,
//!    ou uma área de outra.
//! 2. **O ponto de montagem.** O pacote raiz depende de tudo para montar; só
//!    `app.rs`, os registros (`registries`) e os initializers do servidor
//!    podem nomear um crate de área. Tarefa, binário ou SPA que importasse uma
//!    área esconderia ali o acoplamento que os registros existem para evitar.
//! 3. **Toda área tem consumidor.** Uma área sem nada registrado no ponto de
//!    montagem, ou sem a sua parte no frontend, é código morto que compila.
//!
//! A lista de exceções que existiu até a Fase 10 (ARQ-02 em
//! `docs/debitos_tecnicos.md`) chegou a zero e não volta: este teste não tem
//! lista de exceções.

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

/// Os crates de área e o assistente (que segue a mesma regra: ninguém depende
/// dele, e ele não depende de área).
const AREAS: &[&str] = &["network", "servers", "vpn", "assistant"];

/// Submódulos de área (Fase 14): crate próprio que continua sendo a área e só
/// depende da base dela. `(diretório do crate, área)`.
const SUBMODULOS: &[(&str, &str)] = &[
    ("network-wifi", "network"),
    ("network-link-tests", "network"),
];

/// O que cada crate pode ter como dependência do workspace (sem o prefixo
/// `netmonitor-`). Cada crate leva o próprio esquema (Fase 13).
const PERMITIDAS: &[(&str, &[&str])] = &[
    ("agent-protocol", &[]),
    ("core", &["agent-protocol"]),
    ("nucleo", &["core", "agent-protocol"]),
    ("network", &["core", "agent-protocol"]),
    ("servers", &["core", "agent-protocol"]),
    ("vpn", &["core", "agent-protocol"]),
    ("assistant", &["core", "agent-protocol"]),
    ("network-wifi", &["core", "agent-protocol", "network"]),
    ("network-link-tests", &["core", "agent-protocol", "network"]),
];

/// Arquivos do pacote raiz que montam as áreas.
const PONTO_DE_MONTAGEM: &[&str] = &[
    "app.rs",
    "migrator.rs",
    "registries.rs",
    "registries/",
    "initializers/",
];

fn backend() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// As dependências `netmonitor-*` da seção `[dependencies]` de um manifesto.
fn dependencias_do_workspace(manifesto: &str) -> BTreeSet<String> {
    let mut dentro = false;
    let mut deps = BTreeSet::new();
    for linha in manifesto.lines() {
        let linha = linha.trim();
        if linha.starts_with('[') {
            dentro = linha == "[dependencies]";
            continue;
        }
        if !dentro {
            continue;
        }
        if let Some(nome) = linha
            .split('=')
            .next()
            .map(str::trim)
            .and_then(|nome| nome.strip_prefix("netmonitor-"))
        {
            deps.insert(nome.to_owned());
        }
    }
    deps
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

/// As áreas que um código-fonte nomeia (`netmonitor_network::…`, ou um
/// submódulo dela, `netmonitor_network_wifi::…`).
fn areas_citadas(source: &str) -> BTreeSet<&'static str> {
    let code = sem_comentarios(source);
    let mut citadas: BTreeSet<&'static str> = AREAS
        .iter()
        .copied()
        .filter(|area| code.contains(&format!("netmonitor_{area}::")))
        .collect();
    for (submodulo, area) in SUBMODULOS {
        if code.contains(&format!("netmonitor_{}::", submodulo.replace('-', "_"))) {
            citadas.insert(area);
        }
    }
    citadas
}

#[test]
fn os_manifestos_seguem_a_regra_de_dependencia() {
    let mut violacoes = Vec::new();
    for (krate, permitidas) in PERMITIDAS {
        let manifesto = fs::read_to_string(backend().join("crates").join(krate).join("Cargo.toml"))
            .expect("manifesto legível");
        for dep in dependencias_do_workspace(&manifesto) {
            if !permitidas.contains(&dep.as_str()) {
                violacoes.push(format!("{krate} → {dep}"));
            }
        }
    }
    assert!(
        violacoes.is_empty(),
        "dependências que ferem a regra do ADR 017 (núcleo não depende de área; área não \
         depende de outra nem do `nucleo`) — resolva por um trait do `core`: {violacoes:?}"
    );
}

#[test]
fn todo_crate_do_workspace_tem_regra() {
    let mut sem_regra = Vec::new();
    for entry in fs::read_dir(backend().join("crates")).expect("crates/ legível") {
        let nome = entry
            .expect("entrada")
            .file_name()
            .to_string_lossy()
            .into_owned();
        if !PERMITIDAS.iter().any(|(krate, _)| *krate == nome) {
            sem_regra.push(nome);
        }
    }
    assert!(
        sem_regra.is_empty(),
        "crate novo sem regra de dependência em PERMITIDAS: {sem_regra:?}"
    );
}

#[test]
fn so_o_ponto_de_montagem_importa_area() {
    let src = backend().join("src");
    let mut files = Vec::new();
    rust_files(&src, &mut files);
    let mut violacoes = Vec::new();
    for file in files {
        let relativo = file
            .strip_prefix(&src)
            .expect("dentro de src")
            .to_string_lossy()
            .replace('\\', "/");
        if PONTO_DE_MONTAGEM
            .iter()
            .any(|permitido| relativo == *permitido || relativo.starts_with(permitido))
        {
            continue;
        }
        let source = fs::read_to_string(&file).expect("arquivo legível");
        for area in areas_citadas(&source) {
            violacoes.push(format!("src/{relativo} → {area}"));
        }
    }
    assert!(
        violacoes.is_empty(),
        "fora do ponto de montagem (app.rs, registries, initializers), o pacote raiz não \
         nomeia área — peça pelo registro: {violacoes:?}"
    );
}

#[test]
fn toda_area_tem_consumidor() {
    let registries =
        fs::read_to_string(backend().join("src/registries.rs")).expect("registries legível");
    let montadas = areas_citadas(&registries);
    let frontend = backend().join("../frontend/src/areas");
    for area in AREAS {
        assert!(
            montadas.contains(area),
            "a área `{area}` não contribui nada ao ponto de montagem (src/registries.rs)"
        );
        // O assistente não é área de produto: não tem menu próprio.
        if *area != "assistant" {
            assert!(
                frontend.join(area).join("index.ts").is_file(),
                "a área `{area}` não tem a sua parte no frontend (frontend/src/areas/{area})"
            );
        }
    }
}

/// Tabelas criadas e FKs (`fk("filha", "coluna", "pai", ...)`) das migrations
/// de um crate.
fn esquema(crate_dir: &str) -> (BTreeSet<String>, Vec<(String, String)>) {
    let mut arquivos = Vec::new();
    rust_files(
        &backend()
            .join("crates")
            .join(crate_dir)
            .join("src/migrations"),
        &mut arquivos,
    );
    let mut tabelas = BTreeSet::new();
    let mut fks = Vec::new();
    for arquivo in arquivos {
        if arquivo
            .components()
            .any(|parte| parte.as_os_str() == "logs")
        {
            continue;
        }
        let fonte = sem_comentarios(&fs::read_to_string(&arquivo).expect("migration"));
        let compacta: String = fonte.split_whitespace().collect();
        // `table("x")` (helper do esquema) ou `create_table(m, "x", ...)` (o do Loco).
        let criadas = compacta
            .split("table(\"")
            .skip(1)
            .chain(compacta.split("create_table(m,\"").skip(1));
        for trecho in criadas {
            if let Some(nome) = trecho.split('"').next() {
                tabelas.insert(nome.to_owned());
            }
        }
        for trecho in compacta.split("fk(\"").skip(1) {
            let partes: Vec<&str> = trecho.split('"').collect();
            if partes.len() > 4 {
                fks.push((partes[0].to_owned(), partes[4].to_owned()));
            }
        }
    }
    (tabelas, fks)
}

#[test]
fn tabela_de_area_so_aponta_para_o_nucleo_ou_para_a_propria_area() {
    let (nucleo, _) = esquema("core");
    assert!(
        nucleo.contains("devices"),
        "o detector achou as tabelas do núcleo"
    );
    let mut erradas = Vec::new();
    let crates = AREAS
        .iter()
        .map(|area| (*area, None))
        .chain(SUBMODULOS.iter().map(|(dir, area)| (*dir, Some(*area))));
    for (dir, base) in crates {
        let (proprias, fks) = esquema(dir);
        assert!(
            !proprias.is_empty(),
            "o crate `{dir}` sem tabela no detector"
        );
        let da_base = base.map(|area| esquema(area).0).unwrap_or_default();
        for (filha, pai) in fks {
            if !nucleo.contains(&pai) && !proprias.contains(&pai) && !da_base.contains(&pai) {
                erradas.push(format!("{dir}: {filha} → {pai}"));
            }
        }
    }
    assert!(
        erradas.is_empty(),
        "FK de área para fora do núcleo: {erradas:?}"
    );
}

#[test]
fn o_detector_ignora_comentarios_e_le_os_manifestos() {
    let source = r"
        // use netmonitor_servers::docker::Fake;
        fn f() { netmonitor_network::snmp::client::run(); }
        const DOC: &str = /* netmonitor_vpn::x */ 1;
    ";
    let citadas = areas_citadas(source);
    assert!(citadas.contains("network"));
    assert!(
        areas_citadas("fn g() { netmonitor_network_wifi::watch::run(); }").contains("network"),
        "submódulo conta como a área"
    );
    assert!(!citadas.contains("servers"), "comentário não conta");
    assert!(!citadas.contains("vpn"), "comentário de bloco não conta");

    let manifesto = "[package]\nname = \"x\"\n[dependencies]\nnetmonitor-core = { workspace = true }\nserde = \"1\"\n[dev-dependencies]\nnetmonitor-network = { workspace = true }\n";
    let deps = dependencias_do_workspace(manifesto);
    assert_eq!(
        deps.into_iter().collect::<Vec<_>>(),
        vec!["core".to_owned()],
        "só [dependencies] conta"
    );
}
