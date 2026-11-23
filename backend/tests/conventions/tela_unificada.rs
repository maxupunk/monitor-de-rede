//! Guarda da Fase 6: uma tela, um recurso, uma representação.
//!
//! Estes testes leem o **código-fonte do frontend Flutter** (`frontend/lib`),
//! garantindo que as invariantes estruturais (tela única de logs, reutilização
//! de formulários/catálogos de alertas, ausência de heurística por nome/ID fixo
//! do servidor e ausência de caminhos `runtime_*` paralelos) continuem válidas.

use std::path::{Path, PathBuf};

fn frontend() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("raiz do repositório")
        .join("frontend/lib")
}

fn raiz() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("raiz do repositório")
        .to_path_buf()
}

fn ler_raiz(relativo: &str) -> String {
    let caminho = raiz().join(relativo);
    std::fs::read_to_string(&caminho)
        .unwrap_or_else(|erro| panic!("não foi possível ler {}: {erro}", caminho.display()))
}

fn ler(relativo: &str) -> String {
    let caminho = frontend().join(relativo);
    std::fs::read_to_string(&caminho)
        .unwrap_or_else(|erro| panic!("não foi possível ler {}: {erro}", caminho.display()))
}

fn arquivos(extensoes: &[&str]) -> Vec<PathBuf> {
    fn colete(dir: &Path, extensoes: &[&str], saida: &mut Vec<PathBuf>) {
        let Ok(entradas) = std::fs::read_dir(dir) else {
            return;
        };
        for entrada in entradas.flatten() {
            let caminho = entrada.path();
            if caminho.is_dir() {
                colete(&caminho, extensoes, saida);
            } else if caminho
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| extensoes.contains(&e))
            {
                saida.push(caminho);
            }
        }
    }
    let mut saida = Vec::new();
    colete(&frontend(), extensoes, &mut saida);
    saida
}

#[test]
fn existe_uma_unica_pagina_de_logs() {
    let paginas: Vec<PathBuf> = arquivos(&["dart"])
        .into_iter()
        .filter(|caminho| {
            caminho
                .file_name()
                .and_then(|nome| nome.to_str())
                .is_some_and(|nome| nome.contains("logs") && nome.ends_with("_page.dart"))
        })
        .collect();
    assert_eq!(
        paginas.len(),
        1,
        "uma segunda página de logs apareceu: {paginas:?}"
    );
}

#[test]
fn a_aba_de_logs_do_dispositivo_reutiliza_logs_page() {
    let tabs = ler("features/core/presentation/device_tabs/device_core_tabs.dart");
    assert!(
        tabs.contains("LogsPage(deviceId: deviceId, embedded: true)"),
        "a aba de logs do dispositivo precisa reutilizar LogsPage em modo embedded"
    );
}

#[test]
fn o_compose_oferece_bridge_e_host_com_portas_coerentes() {
    let compose = ler_raiz("docker-compose.yml");
    let host = ler_raiz("docker-compose.host.yml");
    let production = ler_raiz("backend/config/production.yaml");
    let healthcheck = ler_raiz("docker/healthcheck.sh");

    assert!(
        compose.contains("${APP_EXTERNAL_PORT:-3333}:${APP_PORT:-3333}")
            && compose.contains("SYSLOG_UDP_PORT: ${SYSLOG_LISTEN_PORT:-5514}")
            && compose.contains("SYSLOG_TCP_PORT: ${SYSLOG_LISTEN_PORT:-5514}"),
        "a bridge precisa separar porta publicada de porta real"
    );
    assert!(
        host.contains("network_mode: host")
            && host.contains("ports: !reset []")
            && host.contains("sysctls: !reset {}")
            && host.contains("SYSLOG_EXTERNAL_PORT: ${SYSLOG_LISTEN_PORT:-5514}"),
        "o override host precisa remover opções incompatíveis e anunciar a porta escutada"
    );
    assert!(
        production.contains("get_env(name=\"APP_PORT\", default=\"3333\")")
            && healthcheck.contains("${APP_PORT:-3333}"),
        "servidor e healthcheck precisam usar a mesma APP_PORT"
    );
}

#[test]
fn o_formulario_de_regra_e_um_componente_so_nas_duas_telas() {
    for tela in [
        "features/core/presentation/alerts/alerts_page.dart",
        "features/core/presentation/device_tabs/device_core_tabs.dart",
    ] {
        assert!(
            ler(tela).contains("showAlertRuleFormSheet"),
            "{tela} deveria reutilizar o formulário de regra, e não montar o seu"
        );
    }
}

#[test]
fn o_catalogo_de_regras_e_um_componente_so_nas_duas_telas() {
    for tela in [
        "features/core/presentation/alerts/alerts_page.dart",
        "features/core/presentation/device_tabs/device_core_tabs.dart",
    ] {
        assert!(
            ler(tela).contains("showAlertRuleCatalogSheet"),
            "{tela} deveria reutilizar o diálogo do catálogo, e não montar o seu"
        );
    }
}

#[test]
fn nenhuma_tela_deduz_o_servidor_por_nome_ou_por_id_fixo() {
    for caminho in arquivos(&["dart"]) {
        let conteudo = std::fs::read_to_string(&caminho).unwrap_or_default();
        for proibido in [
            "name == 'Servidor NetMonitor'",
            "name == \"Servidor NetMonitor\"",
            "devices[0]",
            "/devices/4",
        ] {
            assert!(
                !conteudo.contains(proibido),
                "{} identifica o servidor por {proibido:?}",
                caminho.display()
            );
        }
    }
}

#[test]
fn nao_existe_rota_store_ou_componente_runtime_paralelo() {
    for caminho in arquivos(&["dart"]) {
        let conteudo = std::fs::read_to_string(&caminho).unwrap_or_default();
        for proibido in [
            "/api/runtime",
            "runtime_logs",
            "runtimeLogs",
            "runtimeMetrics",
        ] {
            assert!(
                !conteudo.contains(proibido),
                "{} cria um caminho `runtime_*`, proibido pela seção 6: {proibido}",
                caminho.display()
            );
        }
    }
}

#[test]
fn a_pagina_do_dispositivo_nao_tem_mais_a_aba_deposito_de_metricas() {
    let pagina = ler("features/network/pages/device_detail_page.dart");
    assert!(
        !pagina.contains("Métricas & Tráfego"),
        "a aba depósito voltou; cada série pertence ao card, monitor ou interface que a produz"
    );
    assert!(
        !pagina.contains("Saúde do Servidor"),
        "não existe aba de saúde do servidor: a saúde mora na Visão Geral"
    );
}
