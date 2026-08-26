//! Guarda da Fase 6: uma tela, um recurso, uma representação.
//!
//! Estes testes leem o **código-fonte do frontend**, pelo mesmo motivo do
//! `camel_case.rs`: o que precisa ser impedido acontece na hora em que alguém
//! escreve uma segunda tela, e não há como enumerar em runtime componentes que
//! ninguém montou. O projeto não tem runner de testes de frontend, então o
//! `grep` estruturado daqui é o único lugar onde a invariante pode ser
//! afirmada.
//!
//! O que se guarda não é estilo — é a regra que dá sentido ao roadmap inteiro:
//! **nada do que ele entrega pode ser exclusivo do servidor**. Uma `LogsPage`
//! duplicada, uma `useLogsStore` clonada ou uma aba "Servidor" em `/logs` são
//! as três formas concretas de quebrá-la.

use std::path::{Path, PathBuf};

fn frontend() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("raiz do repositório")
        .join("frontend/src")
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
    let paginas: Vec<PathBuf> = arquivos(&["vue"])
        .into_iter()
        .filter(|caminho| {
            caminho
                .file_name()
                .and_then(|nome| nome.to_str())
                .is_some_and(|nome| nome.contains("Logs") && nome.ends_with("Page.vue"))
        })
        .collect();
    assert_eq!(
        paginas.len(),
        1,
        "uma segunda página de logs apareceu: {paginas:?}"
    );
}

#[test]
fn a_aba_de_logs_do_dispositivo_usa_a_mesma_store_e_a_mesma_tabela() {
    let pagina =
        if std::path::Path::new(&frontend().join("components/devices/tabs/DeviceLogsTab.vue"))
            .exists()
        {
            ler("components/devices/tabs/DeviceLogsTab.vue")
        } else {
            ler("pages/DeviceDetailPage.vue")
        };
    assert!(
        pagina.contains("useLogsStore"),
        "a aba de logs precisa usar a store de `/logs`, não uma cópia"
    );
    assert!(
        pagina.contains("LogTable"),
        "a aba de logs precisa usar a mesma tabela de `/logs`"
    );
}

#[test]
fn a_aba_do_dispositivo_prioriza_monitores_ativos_sem_mutar_a_store() {
    let pagina = ler("components/devices/tabs/DeviceMonitorsTab.vue");
    assert!(
        pagina.contains(":monitors=\"orderedMonitors\"")
            && pagina.contains("[...detailStore.monitors].sort")
            && pagina.contains("second.isEnabled !== false"),
        "a aba do dispositivo precisa listar ativos primeiro sobre uma cópia da store"
    );
}

#[test]
fn a_ativacao_de_syslog_so_comeca_no_cadastro_do_dispositivo() {
    let cadastro = ler("components/DeviceDialog.vue");
    assert!(
        cadastro.contains("Configurar envio de logs (Syslog)")
            && cadastro.contains("SyslogAutoSetupDialog"),
        "o cadastro precisa oferecer e encadear a configuração automática de syslog"
    );

    for tela in [
        "pages/LogsPage.vue",
        "pages/DeviceDetailPage.vue",
        "components/devices/tabs/DeviceLogsTab.vue",
    ] {
        let conteudo = ler(tela);
        assert!(
            !conteudo.contains("SyslogAutoSetupDialog")
                && !conteudo.contains("SyslogSetupDialog")
                && !conteudo.contains("Configurar envio"),
            "{tela} voltou a oferecer configuração fora do cadastro"
        );
    }
}

#[test]
fn o_formulario_de_regra_e_um_componente_so_nas_duas_telas() {
    // A Fase 6 pede "criar ou editar uma regra usa um único componente
    // compartilhado nas duas páginas". Um formulário inline em `AlertsPage`
    // com um link para ele na aba do dispositivo **não** é isso: quem criasse
    // a regra a partir do equipamento perderia o escopo no caminho.
    for tela in [
        "pages/AlertsPage.vue",
        "components/devices/DeviceRulesTab.vue",
    ] {
        assert!(
            ler(tela).contains("AlertRuleFormDialog"),
            "{tela} deveria reutilizar o formulário de regra, e não montar o seu"
        );
    }
}

#[test]
fn o_detalhe_do_monitor_so_abre_em_dialogo() {
    // `/monitors/{id}` é sempre consultado a partir de um contexto — a lista,
    // a página do dispositivo, o painel. Navegar para uma tela cheia custava
    // ao operador o caminho de volta. A rota continua existindo (link colado
    // no navegador funciona), mas monta o mesmo diálogo.
    let pagina = ler("pages/MonitorDetailPage.vue");
    assert!(
        pagina.contains("MonitorDetailDialog"),
        "a rota do monitor precisa montar o diálogo, e não uma segunda tela"
    );

    // Nenhuma navegação de router para o detalhe: `<a href>` com
    // `@click.prevent` é aceito, porque preserva abrir em nova aba. A regra
    // vale para **toda** derivação que exibe um monitor, e não só para a rota:
    // o ranking de alvos instáveis e o gráfico de latência navegavam para a
    // tela cheia, custando ao operador o caminho de volta ao painel.
    for caminho in arquivos(&["vue"]) {
        let conteudo = std::fs::read_to_string(&caminho).unwrap_or_default();
        for proibido in [
            ":to=\"`/monitors/${item.id}`\"",
            ":to=\"'/monitors/' + monitor.id\"",
            "name: 'monitor-detail'",
            "name: \"monitor-detail\"",
        ] {
            assert!(
                !conteudo.contains(proibido),
                "{} ainda navega para o detalhe do monitor em vez de abrir o diálogo",
                caminho.display()
            );
        }
    }
}

#[test]
fn quem_lista_monitor_usa_a_mesma_regra_de_abrir() {
    // A Fase 4 nasceu de três listas de monitor com regras de clique próprias.
    // O `useMonitorDetail` é a resposta: quem lista monitor pega dali o estado
    // do diálogo e a função de abrir, em vez de inventar a sua.
    for tela in [
        "components/MonitorsTable.vue",
        "pages/DashboardPage.vue",
        "components/widgets/UnstableTargetsWidget.vue",
        "components/widgets/LatencyTimeSeriesWidget.vue",
    ] {
        assert!(
            ler(tela).contains("useMonitorDetail"),
            "{tela} decide por conta própria como abrir um monitor"
        );
    }
}

#[test]
fn a_linha_do_monitor_abre_o_detalhe_e_as_acoes_nao() {
    let tabela = ler("components/MonitorsTable.vue");
    assert!(
        tabela.contains("@click:row"),
        "a linha inteira precisa abrir o detalhe: exigir o clique no nome é uma          armadilha de precisão, porque o alvo tem a largura do texto"
    );
    assert!(
        !tabela.contains(":clickable=\"false\""),
        "a tabela de monitores voltou a ser não-clicável"
    );

    // Sem `@click.stop`, cada clique num botão abriria o diálogo por baixo da
    // ação. As quatro ações de linha aparecem duas vezes — desktop e cartão
    // mobile —, e o `@click` sem modificador é a forma de errar isso.
    for proibido in [
        "@click=\"run(item)\"",
        "@click=\"emit('edit', item)\"",
        "@click=\"confirmDelete(item)\"",
    ] {
        assert!(
            !tabela.contains(proibido),
            "ação de linha sem `@click.stop` na tabela de monitores: {proibido}"
        );
    }
    // O interruptor de ativação é o quarto controle, e o único sem handler
    // próprio de clique: nele o `@click.stop` aparece sozinho, uma vez por
    // variante.
    assert_eq!(
        tabela
            .matches(
                "@click.stop
"
            )
            .count(),
        2,
        "o interruptor de ativação precisa parar a propagação nas duas variantes"
    );

    // O botão de gráfico existia para abrir o detalhe. Com a linha inteira
    // fazendo isso, ele é uma segunda porta para a mesma sala.
    assert!(
        !tabela.contains("mdi-chart-timeline-variant"),
        "o botão de gráfico voltou à linha do monitor"
    );

    // O `href` do nome fica: abrir em nova aba e copiar o endereço continuam
    // valendo. O que ele deixou de ser é o **único** alvo.
    assert!(
        tabela.contains(":href=\"`/monitors/${item.id}`\""),
        "o link do nome perdeu o `href` e com ele o \"abrir em nova aba\""
    );
}

#[test]
fn o_escopo_da_regra_e_escolha_e_nao_heranca() {
    let formulario = ler("components/AlertRuleFormDialog.vue");
    assert!(
        !formulario.contains(":disabled=\"escopoFixo\""),
        "o seletor de escopo voltou a ser travado: há condições genuinamente de          parque, e quem olha um equipamento é quem percebe isso"
    );
    assert!(
        formulario.contains("escopoRestrito"),
        "o escopo aberto de dentro de um dispositivo precisa restringir as opções,          e não oferecer o inventário inteiro"
    );

    // Uma regra global criada na aba do dispositivo tem de aparecer nela: sumir
    // da tela em que nasceu é indistinguível de a criação ter falhado.
    let aba = ler("components/devices/DeviceRulesTab.vue");
    assert!(
        aba.contains("includeGlobal"),
        "a aba de regras do dispositivo precisa listar também as regras globais"
    );
    assert!(
        aba.contains("ehGlobal"),
        "uma regra global listada aqui não pode parecer pertencer ao dispositivo"
    );
}

#[test]
fn o_catalogo_de_regras_e_um_componente_so_nas_duas_telas() {
    for tela in [
        "pages/AlertsPage.vue",
        "components/devices/DeviceRulesTab.vue",
    ] {
        assert!(
            ler(tela).contains("AlertRuleCatalogDialog"),
            "{tela} deveria reutilizar o diálogo do catálogo, e não montar o seu"
        );
    }
}

#[test]
fn nenhuma_tela_deduz_o_servidor_por_nome_ou_por_id_fixo() {
    // A seção 6 do roadmap proíbe identificar o dispositivo do sistema por
    // nome, posição na lista ou ID fixo. Quem responde é o backend, por
    // `isSystem`/`capabilities`.
    for caminho in arquivos(&["vue", "ts"]) {
        let conteudo = std::fs::read_to_string(&caminho).unwrap_or_default();
        for proibido in [
            "name === 'Servidor NetMonitor'",
            "name == 'Servidor NetMonitor'",
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
    for caminho in arquivos(&["vue", "ts"]) {
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
    let pagina = ler("pages/DeviceDetailPage.vue");
    assert!(
        !pagina.contains("Métricas & Tráfego"),
        "a aba depósito voltou; cada série pertence ao card, monitor ou interface que a produz"
    );
    assert!(
        !pagina.contains("Saúde do Servidor"),
        "não existe aba de saúde do servidor: a saúde mora na Visão Geral"
    );
}
