//! A tela em que o usuário está enquanto conversa: o chat sabe que "esses
//! erros" são os da tela de Logs e que "esse container" está no Docker.
//!
//! A SPA manda o id da tela (o `meta.aiScreen` da rota, declarado pela área
//! dona dela) e, quando a tela tem, uma linha com o que está filtrado. Aqui
//! mora o que cada tela significa para a IA: o que ela mostra, como consultar
//! o mesmo dado e quais grupos de ferramentas já entram carregados — o que
//! poupa uma rodada de `load_tools` na pergunta mais provável.
//!
//! Id desconhecido é ignorado, nunca erro: uma SPA mais nova que o backend
//! continua conversando.

use netmonitor_core::{
    ai::tools::{ToolGroup, ToolGroups},
    areas::Area,
};

use crate::dtos::AiScreenContext;

/// Uma tela que a IA entende.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Screen {
    pub id: &'static str,
    pub label: &'static str,
    /// Área do produto (ADR 017): com ela desligada, a tela nem abre.
    pub area: Area,
    /// O que a tela mostra e como a IA consulta o mesmo dado.
    pub about: &'static str,
    /// Grupos que entram carregados enquanto o usuário está nela.
    pub groups: &'static [ToolGroup],
}

pub const SCREENS: [Screen; 24] = [
    Screen {
        id: "dashboard",
        label: "Dashboard",
        area: Area::Core,
        about: "visão geral: status dos equipamentos, alertas abertos e latência — get_system_summary; os cards: get_dashboard_layout",
        groups: &[],
    },
    Screen {
        id: "devices",
        label: "Equipamentos",
        area: Area::Core,
        about: "lista de equipamentos com status e IP — list_devices; um deles: get_device_detail",
        groups: &[],
    },
    Screen {
        id: "device-detail",
        label: "Detalhe do equipamento",
        area: Area::Core,
        about: "um equipamento: interfaces, métricas, monitores e plugins (o equipamento vem no contexto)",
        groups: &[ToolGroup::History],
    },
    Screen {
        id: "monitors",
        label: "Monitores",
        area: Area::Core,
        about: "checagens (ping, HTTP, porta, SNMP) e o status de cada uma — list_monitors; histórico: get_monitor_history",
        groups: &[ToolGroup::History],
    },
    Screen {
        id: "monitor-detail",
        label: "Detalhe do monitor",
        area: Area::Core,
        about: "um monitor: latência, disponibilidade e falhas recentes (o monitor vem no contexto)",
        groups: &[ToolGroup::History, ToolGroup::Charts],
    },
    Screen {
        id: "alerts",
        label: "Alertas",
        area: Area::Core,
        about: "alertas abertos e resolvidos — get_alerts; por que disparou: explain_alert",
        groups: &[ToolGroup::AlertRules, ToolGroup::Analysis],
    },
    Screen {
        id: "events",
        label: "Eventos",
        area: Area::Core,
        about: "linha do tempo de quedas, voltas e alertas — get_incident_timeline; procura: grep source='alerts'",
        groups: &[ToolGroup::Analysis],
    },
    Screen {
        id: "logs",
        label: "Logs",
        area: Area::Core,
        about: "syslog dos equipamentos e da aplicação — get_logs_overview para o panorama, grep source='logs' para procurar",
        groups: &[ToolGroup::Logs],
    },
    Screen {
        id: "maintenance-windows",
        label: "Manutenção",
        area: Area::Core,
        about: "janelas de manutenção ativas e futuras — get_platform_status area='maintenance'",
        groups: &[ToolGroup::Platform, ToolGroup::Actions],
    },
    Screen {
        id: "scheduled-tasks",
        label: "Tarefas agendadas",
        area: Area::Core,
        about: "tarefas que rodam sozinhas (cron) — list_scheduled_tasks",
        groups: &[ToolGroup::Schedules],
    },
    Screen {
        id: "remote-servers",
        label: "Servidores remotos",
        area: Area::Core,
        about: "agentes remotos: conexão, versão e host — get_platform_status area='agents'",
        groups: &[ToolGroup::Platform],
    },
    Screen {
        id: "audit",
        label: "Auditoria",
        area: Area::Core,
        about: "quem mudou o quê e quando — get_audit_log",
        groups: &[ToolGroup::Audit],
    },
    Screen {
        id: "wifi",
        label: "Clientes Wi-Fi",
        area: Area::Network,
        about: "aparelhos conectados a cada roteador/AP: sinal, SNR, retransmissões, saúde e os vigiados — get_platform_status area='wifi'",
        groups: &[ToolGroup::Platform],
    },
    Screen {
        id: "sites",
        label: "Sites",
        area: Area::Network,
        about: "locais físicos e o que há em cada um — get_platform_status area='networks'",
        groups: &[ToolGroup::Platform],
    },
    Screen {
        id: "networks",
        label: "Redes",
        area: Area::Network,
        about: "redes IP (CIDR), sites e servidores DNS — get_platform_status area='networks'",
        groups: &[ToolGroup::Platform],
    },
    Screen {
        id: "topology",
        label: "Topologia",
        area: Area::Network,
        about: "mapa de enlaces entre equipamentos — get_topology",
        groups: &[ToolGroup::Platform],
    },
    Screen {
        id: "link-tests",
        label: "Testes de velocidade",
        area: Area::Network,
        about: "testes de velocidade de enlace e os agendamentos deles",
        groups: &[ToolGroup::Platform],
    },
    Screen {
        id: "discovery",
        label: "Descoberta",
        area: Area::Network,
        about: "varreduras de rede recentes e em andamento — get_platform_status area='discovery'",
        groups: &[ToolGroup::Platform],
    },
    Screen {
        id: "plugins",
        label: "Plugins",
        area: Area::Network,
        about: "plugins de equipamento e as ações deles — list_device_plugins, get_plugin",
        groups: &[ToolGroup::Devices],
    },
    Screen {
        id: "plugin-app",
        label: "App de plugin",
        area: Area::Network,
        about: "a tela de um app de plugin — list_plugin_apps",
        groups: &[ToolGroup::Devices],
    },
    Screen {
        id: "docker",
        label: "Docker",
        area: Area::Servers,
        about: "containers, imagens, volumes e redes dos hosts Docker — get_docker_containers, get_docker_usage; logs: grep source='docker' container=<nome>",
        groups: &[ToolGroup::Docker],
    },
    Screen {
        id: "deploys",
        label: "Deploy contínuo",
        area: Area::Servers,
        about: "aplicações, deploys e compilações — get_deploys, get_builds; o log: get_deploy_log, get_build_log",
        groups: &[ToolGroup::Docker],
    },
    Screen {
        id: "registry",
        label: "Registry",
        area: Area::Servers,
        about: "imagens do registry próprio e as vulnerabilidades achadas nelas",
        groups: &[ToolGroup::Docker],
    },
    Screen {
        id: "vpn",
        label: "VPN",
        area: Area::Vpn,
        about: "servidor WireGuard e peers: handshake e tráfego — get_platform_status area='vpn'",
        groups: &[ToolGroup::Platform],
    },
];

/// Caracteres do que a tela diz estar filtrado.
pub const MAX_DETAIL_CHARS: usize = 300;

#[must_use]
pub fn screen(id: &str) -> Option<&'static Screen> {
    SCREENS.iter().find(|screen| screen.id == id.trim())
}

/// A tela aberta, validada para o prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenContext {
    pub screen: &'static Screen,
    /// O que está filtrado ou selecionado, numa linha.
    pub detail: Option<String>,
    /// A tela da pergunta anterior, se era outra.
    pub previous: Option<&'static Screen>,
}

impl ScreenContext {
    /// A linha do `<contexto>`.
    #[must_use]
    pub fn line(&self) -> String {
        let detail = self
            .detail
            .as_deref()
            .map(|detail| format!(" Na tela agora: {detail}."))
            .unwrap_or_default();
        let previous = self
            .previous
            .map(|previous| {
                format!(
                    " A pergunta anterior foi feita na tela {}: referência vaga pode ser de qualquer uma das duas.",
                    previous.label
                )
            })
            .unwrap_or_default();
        format!(
            "- {}: {}.{detail}{previous}",
            self.screen.label, self.screen.about
        )
    }

    /// Os grupos que a tela pré-carrega.
    #[must_use]
    pub fn groups(&self) -> ToolGroups {
        self.screen.groups.iter().copied().collect()
    }
}

/// Uma linha só, sem espaços repetidos, cortada no limite.
fn one_line(text: &str) -> Option<String> {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return None;
    }
    if collapsed.chars().count() <= MAX_DETAIL_CHARS {
        return Some(collapsed);
    }
    let mut cut: String = collapsed.chars().take(MAX_DETAIL_CHARS - 1).collect();
    cut.push('…');
    Some(cut)
}

/// A tela que a SPA mandou, se o backend a conhece.
#[must_use]
pub fn sanitize(input: Option<AiScreenContext>) -> Option<ScreenContext> {
    let input = input?;
    let current = screen(&input.id)?;
    Some(ScreenContext {
        screen: current,
        detail: input.detail.as_deref().and_then(one_line),
        previous: input
            .previous
            .as_deref()
            .and_then(screen)
            .filter(|previous| previous.id != current.id),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    fn entrada(id: &str, detail: Option<&str>) -> Option<AiScreenContext> {
        Some(AiScreenContext {
            id: id.into(),
            detail: detail.map(Into::into),
            previous: None,
        })
    }

    #[test]
    fn tela_anterior_diferente_entra_na_linha() {
        let mudou = sanitize(Some(AiScreenContext {
            id: "docker".into(),
            detail: None,
            previous: Some("logs".into()),
        }))
        .unwrap();
        assert!(mudou
            .line()
            .contains("pergunta anterior foi feita na tela Logs"));

        let mesma = sanitize(Some(AiScreenContext {
            id: "docker".into(),
            detail: None,
            previous: Some("docker".into()),
        }))
        .unwrap();
        assert_eq!(mesma.previous, None);
        let inventada = sanitize(Some(AiScreenContext {
            id: "docker".into(),
            detail: None,
            previous: Some("nao-existe".into()),
        }))
        .unwrap();
        assert_eq!(inventada.previous, None);
    }

    #[test]
    fn ids_sao_unicos() {
        let ids: HashSet<&str> = SCREENS.iter().map(|screen| screen.id).collect();
        assert_eq!(ids.len(), SCREENS.len());
    }

    #[test]
    fn tela_conhecida_vira_linha_e_grupos() {
        let logs = sanitize(entrada("logs", Some("  severidade  erro,\n ap-sala "))).unwrap();
        assert_eq!(logs.detail.as_deref(), Some("severidade erro, ap-sala"));
        assert!(logs.line().starts_with("- Logs: syslog"));
        assert!(logs
            .line()
            .ends_with("Na tela agora: severidade erro, ap-sala."));
        assert!(logs.groups().contains(&ToolGroup::Logs));
    }

    #[test]
    fn tela_desconhecida_ou_ausente_e_ignorada() {
        assert!(sanitize(entrada("settings", None)).is_none());
        assert!(sanitize(None).is_none());
        let sem_detalhe = sanitize(entrada("docker", Some("   "))).unwrap();
        assert_eq!(sem_detalhe.detail, None);
    }

    /// As telas que a SPA declara (`frontend/src/areas/*/aiScreens.ts`)
    /// existem aqui: id desconhecido seria ignorado em silêncio, e a IA
    /// ficaria sem saber o que a tela mostra.
    #[test]
    fn telas_declaradas_pela_spa_existem_no_catalogo() {
        let areas =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../frontend/src/areas");
        let id = regex::Regex::new(r"(?m)^\s*id: '([^']+)',").unwrap();
        let mut vistas = 0;
        for area in std::fs::read_dir(&areas).expect("frontend/src/areas") {
            let arquivo = area.unwrap().path().join("aiScreens.ts");
            let Ok(texto) = std::fs::read_to_string(&arquivo) else {
                continue;
            };
            for captura in id.captures_iter(&texto) {
                vistas += 1;
                assert!(
                    screen(&captura[1]).is_some(),
                    "{} declara a tela '{}', que não está em SCREENS",
                    arquivo.display(),
                    &captura[1]
                );
            }
        }
        assert_eq!(
            vistas,
            SCREENS.len(),
            "toda tela do catálogo tem rota na SPA"
        );
    }

    #[test]
    fn detalhe_longo_e_cortado() {
        let longo = "x".repeat(MAX_DETAIL_CHARS * 2);
        let tela = sanitize(entrada("wifi", Some(&longo))).unwrap();
        assert_eq!(tela.detail.unwrap().chars().count(), MAX_DETAIL_CHARS);
    }
}
