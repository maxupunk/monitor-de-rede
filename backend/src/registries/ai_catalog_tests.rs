//! O catálogo completo de ferramentas da IA, como o ponto de montagem o
//! monta: os testes que precisam de todas as ferramentas de todas as áreas
//! (nomes, grupos, política, o contrato do chat e o custo fixo por rodada).
//! Vivem aqui, e não no crate do assistente, porque só o ponto de montagem
//! conhece todas as áreas.

use std::{collections::HashSet, path::PathBuf};

use netmonitor_assistant::{
    harness::{compaction::estimate_tokens, contract::chat_contract, prompt::build_system_prompt},
    settings::AiSettings,
};
use netmonitor_core::{
    ai::{
        automation::AiAutomationMode,
        risk::{AiToolRisk, Approval},
        tools::{
            ToolGroup, ToolGroups, ToolKind, ToolPolicy, ToolRegistry, UnattendedTools, LOAD_TOOLS,
        },
    },
    areas::{Area, AreaSet},
};

use super::{ai_tool_catalog, install_ai_tools, install_dashboard_widgets};

#[test]
fn nomes_sao_unicos() {
    install_dashboard_widgets();
    install_ai_tools();
    let handlers = ai_tool_catalog();
    let names: HashSet<&str> = handlers.iter().map(|handler| handler.name()).collect();
    assert_eq!(names.len(), handlers.len());
}

#[test]
fn todo_contrato_e_um_objeto_json_schema() {
    install_dashboard_widgets();
    install_ai_tools();
    for handler in ai_tool_catalog() {
        let schema = handler.parameters();
        assert_eq!(schema["type"], "object", "{}", handler.name());
        assert!(schema["properties"].is_object(), "{}", handler.name());
        assert!(!handler.description().is_empty(), "{}", handler.name());
    }
}

#[test]
fn ferramentas_ativas_so_entram_quando_liberadas() {
    install_dashboard_widgets();
    install_ai_tools();
    let passivas = ToolRegistry::new(ToolPolicy::passive());
    let nomes: Vec<String> = passivas
        .definitions()
        .into_iter()
        .map(|tool| tool.function.name)
        .collect();
    assert!(nomes.contains(&"chart_monitor_latency".to_string()));
    assert!(nomes.contains(&"get_device_interfaces".to_string()));
    assert!(
        nomes.contains(&"web_search".to_string()),
        "resumo de incidente também pesquisa na web"
    );
    assert!(!nomes.contains(&"ping_host".to_string()));

    assert!(!nomes.contains(&"silence_alert".to_string()));
    assert!(
        !nomes.contains(&"ask_user".to_string()),
        "rotina automática não tem a quem perguntar"
    );

    let todas = ToolRegistry::new(ToolPolicy {
        allow_active: true,
        actions: AiAutomationMode::Confirm,
        confirm_active: false,
        interactive: true,
        container_actions: AiAutomationMode::Confirm,
        areas: AreaSet::ALL,
        writable: AreaSet::ALL,
    });
    assert_eq!(todas.definitions().len(), ai_tool_catalog().len());
}

#[test]
fn catalogo_sob_demanda_manda_o_nucleo_e_so_o_que_foi_carregado() {
    install_dashboard_widgets();
    install_ai_tools();
    let registro = ToolRegistry::new(ToolPolicy {
        allow_active: true,
        actions: AiAutomationMode::Off,
        confirm_active: false,
        interactive: true,
        container_actions: AiAutomationMode::Confirm,
        areas: AreaSet::ALL,
        writable: AreaSet::ALL,
    });
    let nomes = |carregados: &ToolGroups| -> Vec<String> {
        registro
            .definitions_for(carregados)
            .into_iter()
            .map(|tool| tool.function.name)
            .collect()
    };

    let nucleo = nomes(&ToolGroups::new());
    assert!(nucleo.contains(&"grep".to_string()));
    assert!(nucleo.contains(&"ask_user".to_string()));
    assert!(!nucleo.contains(&"chart_monitor_latency".to_string()));
    assert!(nucleo.contains(&LOAD_TOOLS.to_string()));
    assert!(
        !registro.available_groups().contains(&ToolGroup::Actions),
        "ação desligada não entra nem no catálogo"
    );

    let com_graficos = nomes(&ToolGroups::from([ToolGroup::Charts]));
    assert!(com_graficos.contains(&"chart_monitor_latency".to_string()));
    assert_eq!(
        com_graficos[..nucleo.len()],
        nucleo[..],
        "grupo carregado entra no fim: o prefixo em cache não muda"
    );

    let depois = nomes(&ToolGroups::from([ToolGroup::Charts, ToolGroup::History]));
    assert_eq!(depois[..com_graficos.len()], com_graficos[..]);

    let tudo: ToolGroups = registro.available_groups().into_iter().collect();
    assert!(
        !nomes(&tudo).contains(&LOAD_TOOLS.to_string()),
        "nada a carregar, sem load_tools"
    );
}

#[test]
fn tarefa_agendada_so_roda_o_que_dispensa_alguem_do_outro_lado() {
    install_dashboard_widgets();
    install_ai_tools();
    let catalogo = UnattendedTools::new();
    let nomes = catalogo.names();
    for agendavel in [
        "get_system_summary",
        "ping_host",
        "silence_alert",
        "create_maintenance_window",
        "toggle_alert_rule",
        "docker_container_action",
        "get_audit_log",
        "web_search",
    ] {
        assert!(nomes.contains(&agendavel), "{agendavel}");
    }
    for fora in [
        "ask_user",
        "device_ssh_exec",
        "run_plugin_action",
        "chart_monitor_latency",
        "get_alert_rules_guide",
        "search_system_docs",
        "create_scheduled_task",
        "list_scheduled_tasks",
    ] {
        assert!(!nomes.contains(&fora), "{fora} não pode virar tarefa");
        assert!(catalogo.contract(fora).is_err());
    }
    assert!(
        catalogo
            .contract("docker_container_action")
            .unwrap()
            .admin_only
    );
    assert!(catalogo.contract("get_audit_log").unwrap().admin_only);
    assert!(!catalogo.contract("ping_host").unwrap().admin_only);
}

#[test]
fn grupos_guardam_a_ordem_de_carga_sem_repetir() {
    install_dashboard_widgets();
    install_ai_tools();
    let mut grupos = ToolGroups::from_ids(&["docker", "history", "inventado", "DOCKER"]);
    assert!(!grupos.insert(ToolGroup::History));
    grupos.insert(ToolGroup::Charts);
    assert_eq!(grupos.ids(), vec!["docker", "history", "charts"]);
}

#[test]
fn nucleo_cabe_num_orcamento_pequeno() {
    install_dashboard_widgets();
    install_ai_tools();
    let registro = ToolRegistry::new(ToolPolicy::from_settings(&AiSettings::default()));
    let tamanho: usize = registro
        .definitions_for(&ToolGroups::new())
        .iter()
        .map(|tool| serde_json::to_string(tool).unwrap().len())
        .sum();
    assert!(
        tamanho < 7_500,
        "o núcleo vai em toda rodada: {tamanho} caracteres"
    );
}

#[test]
fn acao_em_container_segue_o_modo_configurado() {
    install_dashboard_widgets();
    install_ai_tools();
    let modo = |container_actions| {
        ToolRegistry::new(ToolPolicy {
            container_actions,
            ..ToolPolicy::from_settings(&AiSettings::default())
        })
    };
    let padrao = ToolRegistry::new(ToolPolicy::from_settings(&AiSettings::default()));
    assert!(
        padrao.needs_confirmation("docker_container_action"),
        "o padrão é pedir permissão"
    );

    let auto = modo(AiAutomationMode::Auto);
    assert!(!auto.needs_confirmation("docker_container_action"));
    assert!(auto
        .tool_names(ToolGroup::Docker)
        .contains(&"docker_container_action"));

    let desligado = modo(AiAutomationMode::Off);
    assert!(!desligado
        .tool_names(ToolGroup::Docker)
        .contains(&"docker_container_action"));

    let rotina = ToolRegistry::new(ToolPolicy::passive());
    assert!(
        !rotina
            .tool_names(ToolGroup::Docker)
            .contains(&"docker_container_action"),
        "rotina automática nunca mexe em container"
    );
}

#[test]
fn acoes_sempre_pedem_confirmacao_e_testes_ativos_so_quando_configurado() {
    install_dashboard_widgets();
    install_ai_tools();
    let sem = ToolPolicy {
        allow_active: true,
        actions: AiAutomationMode::Confirm,
        confirm_active: false,
        interactive: true,
        container_actions: AiAutomationMode::Confirm,
        areas: AreaSet::ALL,
        writable: AreaSet::ALL,
    };
    assert!(!sem.needs_confirmation(ToolKind::Passive));
    assert!(!sem.needs_confirmation(ToolKind::Active));
    assert!(!sem.needs_confirmation(ToolKind::Interactive));
    assert!(sem.needs_confirmation(ToolKind::Action));

    let com = ToolPolicy {
        confirm_active: true,
        ..sem
    };
    assert!(com.needs_confirmation(ToolKind::Active));

    let registro = ToolRegistry::new(com);
    assert!(registro.needs_confirmation("silence_alert"));
    assert!(registro.needs_confirmation("ping_host"));
    assert!(!registro.needs_confirmation("get_alerts"));
    assert!(!registro.needs_confirmation("inexistente"));
}

#[test]
fn automatico_roda_risco_baixo_e_medio_e_pergunta_o_alto() {
    install_dashboard_widgets();
    install_ai_tools();
    let politica = ToolPolicy {
        actions: AiAutomationMode::Auto,
        ..ToolPolicy::from_settings(&AiSettings::default())
    };
    assert!(!politica.needs_confirmation(ToolKind::Action));
    assert_eq!(
        politica.approval(ToolKind::Action, AiToolRisk::Medium),
        Approval::Run
    );
    assert_eq!(
        politica.approval(ToolKind::Action, AiToolRisk::High),
        Approval::Ask
    );

    let pedir = ToolPolicy {
        actions: AiAutomationMode::Confirm,
        ..politica
    };
    assert_eq!(
        pedir.approval(ToolKind::Action, AiToolRisk::Low),
        Approval::Ask,
        "pedir permissão pergunta mesmo o risco baixo"
    );
    assert_eq!(
        politica.approval(ToolKind::Passive, AiToolRisk::Low),
        Approval::Run
    );
}

#[test]
fn acesso_a_equipamento_nunca_e_liberado_pelo_risco() {
    install_dashboard_widgets();
    install_ai_tools();
    let politica = ToolPolicy {
        allow_active: true,
        confirm_active: false,
        actions: AiAutomationMode::Auto,
        interactive: true,
        container_actions: AiAutomationMode::Auto,
        areas: AreaSet::ALL,
        writable: AreaSet::ALL,
    };
    assert!(politica.needs_confirmation(ToolKind::DeviceAccess));
    assert_eq!(
        politica.approval(ToolKind::DeviceAccess, AiToolRisk::Low),
        Approval::Ask,
        "ADR 012: só o aceitar automaticamente da conversa dispensa"
    );
}

#[test]
fn sem_a_area_servidores_nenhuma_ferramenta_docker_nem_grupo_docker() {
    install_dashboard_widgets();
    install_ai_tools();
    let politica = ToolPolicy::from_settings(&AiSettings::default())
        .with_areas(AreaSet::ALL.without(Area::Servers));
    assert_eq!(politica.container_actions, AiAutomationMode::Off);
    let registro = ToolRegistry::new(politica);
    assert!(
        !registro
            .definitions()
            .iter()
            .any(|tool| tool.function.name.contains("docker")),
        "ferramenta docker_* com a área desligada"
    );
    assert!(!registro.available_groups().contains(&ToolGroup::Docker));
    let catalogo = UnattendedTools::for_areas(AreaSet::ALL.without(Area::Servers));
    assert!(!catalogo.names().iter().any(|name| name.contains("docker")));
    // E com tudo ligado elas voltam.
    assert!(UnattendedTools::new()
        .names()
        .iter()
        .any(|name| name.contains("docker")));
}

/// Reescreve `backend/contracts/ai_chat.json` quando o contrato muda.
#[tokio::test]
async fn exporta_o_contrato_do_chat() {
    let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
    install_dashboard_widgets();
    install_ai_tools();
    let contract = chat_contract(&db).await;
    assert!(!contract.core.is_empty());
    assert!(contract.load_tools.is_some());
    assert!(contract.groups.iter().all(|group| !group.tools.is_empty()));
    assert!(contract.system_prompt_on_demand.contains("{{today}}"));
    assert!(contract.turn_context.contains("{{now}}"));

    let json = serde_json::to_string_pretty(&contract).unwrap() + "\n";
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("contracts/ai_chat.json");
    if std::fs::read_to_string(&path).ok().as_deref() != Some(json.as_str()) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, json).unwrap();
    }
}

/// O que toda pergunta de verdade paga em cada rodada, antes de qualquer
/// dado: o system prompt e o núcleo de ferramentas. Medido em 2.970
/// tokens (era 3.038, depois 2.795, 2.589, 2.675 e 2.727). Subir o teto é
/// decisão, não acidente: cada token a mais aqui se paga em toda rodada de
/// toda pergunta. Última subida (2.985 → 2.995): `get_deploy_guide` na
/// linha do grupo `docker` (o guia do primeiro deploy, que a IA conduz),
/// ~5 tokens. Antes (2.975 → 2.985): `get_builds` e
/// `get_build_log` na linha do grupo `docker` (compilação no servidor,
/// ADR 023), ~9 tokens. Antes (2.775 → 2.975): a ferramenta `web_search`
/// no núcleo (`ToolGroup::Core`), ~195 tokens. Antes (2.750 → 2.775): o
/// grupo `dashboard` (layout do dashboard) no catálogo e no `enum` do
/// `load_tools`, ~10 tokens. Antes (2.700 → 2.750): o grupo `schedules`
/// (tarefas agendadas) no catálogo, ~50 tokens — a linha com os cinco
/// nomes é o que deixa a IA agendar sem um `load_tools` às cegas. Antes
/// (2.650 → 2.700): o grupo `audit` e a `list_plugin_apps`, ~25 tokens.
#[test]
fn custo_fixo_por_rodada_cabe_no_orcamento() {
    const BUDGET: u64 = 2_995;
    install_dashboard_widgets();
    install_ai_tools();
    let settings = AiSettings {
        action_mode: AiAutomationMode::Auto,
        container_actions: AiAutomationMode::Auto,
        ..AiSettings::default()
    };
    let policy = ToolPolicy::from_settings(&settings);
    let prompt = build_system_prompt(&settings, policy, true);
    let core = ToolRegistry::new(policy).definitions_for(&ToolGroups::new());
    let fixed = estimate_tokens(&prompt) + estimate_tokens(&serde_json::to_string(&core).unwrap());
    assert!(
        fixed <= BUDGET,
        "prompt + núcleo = ~{fixed} tokens, acima do orçamento de {BUDGET}"
    );
}
