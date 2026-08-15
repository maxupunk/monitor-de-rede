//! Catálogo de regras pré-configuradas (§8.7).
//!
//! É a única fonte de verdade das políticas de alerta que antes viviam
//! espalhadas no código (ex.: downgrade de negociação de interface). Cada
//! template é apenas *dado*: quem aplica, avalia e dispara não precisa ser
//! alterado para nascer uma política nova — basta acrescentar um item aqui.

use serde::Serialize;
use serde_json::{json, Value};

use crate::services::alerts::fields::{
    self, interface_speed_transition, interface_status_transition, vpn_status_transition,
};

/// As seis categorias exibidas na tela, com os rótulos em português.
pub const CATEGORY_LABELS: [(&str, &str); 6] = [
    ("disponibilidade", "Disponibilidade"),
    ("desempenho", "Desempenho"),
    ("servicos", "Serviços e aplicações"),
    ("interfaces", "Interfaces de rede (SNMP)"),
    ("equipamento", "Equipamento (SNMP)"),
    ("vpn", "Túneis VPN (WireGuard)"),
];

/// Um item do catálogo. Os campos são exatamente os que o frontend lê em
/// `AlertRuleTemplate` (`stores/alerts.ts`), mais `applied`/`ruleId`, que a
/// camada de serviço acrescenta.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlertRuleTemplate {
    /// Chave de idempotência: uma regra por template.
    pub key: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub category: &'static str,
    #[serde(rename = "type")]
    pub rule_type: &'static str,
    pub condition: Value,
    pub severity: &'static str,
    pub duration_seconds: i32,
    /// Janela de estabilidade antes de resolver (Fase 1), revisada por tipo de
    /// problema na Fase 2: as regras de degradação sustentada (perda, latência)
    /// ganham janela igual à tolerância de disparo (300 s); as transições de
    /// interface/túnel ganham janela curta (120 s), para que uma recaída
    /// imediata reabra o episódio em vez de gerar um novo par de alertas.
    ///
    /// Idempotência: a assinatura de dedup (`catalog/service.rs`) cobre só
    /// condição + escopo — mudar a janela aqui **não** atualiza regras já
    /// aplicadas em instalações existentes; só instalações novas (ou quem
    /// ainda não aplicou o template) recebem o valor novo.
    pub recovery_window_seconds: i32,
    /// Recaídas dentro de [`Self::flap_window_seconds`] que declaram o alvo
    /// oscilando (Fase 3). `0` desliga a detecção.
    ///
    /// O default do catálogo é `5` em tudo que tem janela de estabilidade e `0`
    /// no resto — e não é arbitrário: a detecção acontece **sobre o episódio**,
    /// que só sobrevive à oscilação quando há janela. Template com
    /// `recovery_window_seconds: 0` fecha o evento na primeira checagem ok e
    /// nunca chega a recair, então limiar ali seria letra morta.
    pub flap_threshold: i32,
    /// Largura da janela deslizante da detecção de flapping. 900 s (15 min) em
    /// todo o catálogo: é o horizonte em que "caiu de novo" ainda descreve o
    /// mesmo problema.
    pub flap_window_seconds: i32,
    /// Intervalo mínimo entre notificações de problema do par (regra, alvo),
    /// mesmo quando o episódio fecha e um novo abre (Fase 4).
    ///
    /// 900 s em tudo que descreve **problema** (`warning`/`critical`) e `0` nos
    /// registros informativos. O critério é o que a mensagem custa ao operador:
    /// a janela de estabilização já cobre a oscilação *dentro* do episódio, mas
    /// nada impedia um episódio de fechar e outro abrir três minutos depois,
    /// com o par 🚨+✅ inteiro de novo. Registro informativo não precisa de
    /// freio — ele já é raro por construção.
    pub notification_cooldown_seconds: i32,
    /// Suprimir a notificação quando o pai declarado do dispositivo já está em
    /// alerta (Fase 4).
    ///
    /// Ligado nas categorias que medem "consigo falar com o alvo"
    /// (disponibilidade, desempenho, serviços) — são exatamente as que um
    /// roteador caído derruba em massa. Desligado nas que descrevem o estado do
    /// próprio equipamento (interfaces, equipamento, VPN): ali o pai não explica
    /// o filho, e calar seria esconder.
    pub inhibit_when_parent_down: bool,
    /// Faz parte do conjunto básico provisionado por padrão.
    pub recommended: bool,
}

/// Os 18 templates portados literalmente do `alert_rule_templates.ts`.
///
/// É função e não `const` porque `condition` é um `serde_json::Value`, que não
/// pode ser construído em contexto constante. A ordem é a do original — ela
/// define a ordem de exibição na tela.
#[must_use]
pub fn all() -> Vec<AlertRuleTemplate> {
    vec![
        AlertRuleTemplate {
            key: "device_offline",
            name: "Dispositivo sem resposta",
            description: "Dispara assim que uma verificação retorna \"sem resposta\". É a regra base de indisponibilidade.",
            category: "disponibilidade",
            rule_type: "device_offline",
            condition: json!({ "field": fields::STATUS, "operator": "eq", "value": "down" }),
            severity: "critical",
            duration_seconds: 0,
            // Revisto na Fase 3: era 0. A indisponibilidade que cai e volta é
            // *o* caso de flapping, e sem janela o evento fecha na primeira
            // checagem ok — nunca chega a recair, nunca é detectado. 120 s
            // também entrega o critério de aceite da Fase 1 (link caindo a cada
            // 30 s gera um par de notificações, não vinte).
            recovery_window_seconds: 120,
            flap_threshold: 5,
            flap_window_seconds: 900,
            notification_cooldown_seconds: 900,
            inhibit_when_parent_down: true,
            recommended: true,
        },
        AlertRuleTemplate {
            key: "packet_loss_high",
            name: "Perda de pacotes acima de 10%",
            description: "Link instável: parte dos pacotes ICMP não volta. Só dispara se a perda persistir por 5 minutos.",
            category: "disponibilidade",
            rule_type: "custom",
            condition: json!({ "field": fields::PACKET_LOSS, "operator": "gt", "value": 10 }),
            severity: "warning",
            duration_seconds: 300,
            recovery_window_seconds: 300,
            flap_threshold: 5,
            flap_window_seconds: 900,
            notification_cooldown_seconds: 900,
            inhibit_when_parent_down: true,
            recommended: true,
        },
        AlertRuleTemplate {
            key: "latency_high",
            name: "Latência acima de 200 ms",
            description: "Tempo de resposta degradado de forma sustentada (5 minutos), evitando alarme por oscilação momentânea.",
            category: "desempenho",
            rule_type: "latency_high",
            condition: json!({ "field": fields::LATENCY_MS, "operator": "gt", "value": 200 }),
            severity: "warning",
            duration_seconds: 300,
            recovery_window_seconds: 300,
            flap_threshold: 5,
            flap_window_seconds: 900,
            notification_cooldown_seconds: 900,
            inhibit_when_parent_down: true,
            recommended: true,
        },
        AlertRuleTemplate {
            key: "latency_critical",
            name: "Latência acima de 500 ms",
            description: "Degradação severa de latência: acima deste patamar aplicações interativas travam.",
            category: "desempenho",
            rule_type: "latency_high",
            condition: json!({ "field": fields::LATENCY_MS, "operator": "gt", "value": 500 }),
            severity: "critical",
            duration_seconds: 300,
            recovery_window_seconds: 300,
            flap_threshold: 5,
            flap_window_seconds: 900,
            notification_cooldown_seconds: 900,
            inhibit_when_parent_down: true,
            recommended: false,
        },
        AlertRuleTemplate {
            key: "check_duration_high",
            name: "Checagem demorando mais de 5 segundos",
            description: "Verificação lenta como um todo — útil para monitores HTTP de páginas pesadas.",
            category: "desempenho",
            rule_type: "custom",
            condition: json!({ "field": fields::DURATION_MS, "operator": "gt", "value": 5000 }),
            severity: "info",
            duration_seconds: 300,
            recovery_window_seconds: 300,
            flap_threshold: 5,
            flap_window_seconds: 900,
            notification_cooldown_seconds: 0,
            inhibit_when_parent_down: true,
            recommended: false,
        },
        AlertRuleTemplate {
            key: "http_error_response",
            name: "Serviço HTTP respondendo com erro (4xx/5xx)",
            description: "O site/serviço responde, mas devolve código de erro em vez de conteúdo válido.",
            category: "servicos",
            rule_type: "http_failure",
            condition: json!({ "field": fields::STATUS_CODE, "operator": "gte", "value": 400 }),
            severity: "critical",
            duration_seconds: 0,
            recovery_window_seconds: 0,
            flap_threshold: 0,
            flap_window_seconds: 900,
            notification_cooldown_seconds: 900,
            inhibit_when_parent_down: true,
            recommended: true,
        },
        AlertRuleTemplate {
            key: "tcp_connect_slow",
            name: "Conexão TCP acima de 1 segundo",
            description: "A porta monitorada aceita conexão, mas o handshake está lento.",
            category: "servicos",
            rule_type: "tcp_failure",
            condition: json!({ "field": fields::CONNECT_TIME_MS, "operator": "gt", "value": 1000 }),
            severity: "warning",
            duration_seconds: 300,
            recovery_window_seconds: 300,
            flap_threshold: 5,
            flap_window_seconds: 900,
            notification_cooldown_seconds: 900,
            inhibit_when_parent_down: true,
            recommended: false,
        },
        AlertRuleTemplate {
            key: "dns_resolution_slow",
            name: "Resolução DNS acima de 800 ms",
            description: "O servidor DNS responde, porém devagar o bastante para atrasar toda a navegação.",
            category: "servicos",
            rule_type: "custom",
            condition: json!({ "field": fields::RESOLUTION_TIME_MS, "operator": "gt", "value": 800 }),
            severity: "warning",
            duration_seconds: 300,
            recovery_window_seconds: 300,
            flap_threshold: 5,
            flap_window_seconds: 900,
            notification_cooldown_seconds: 900,
            inhibit_when_parent_down: true,
            recommended: false,
        },
        AlertRuleTemplate {
            key: "interface_link_down",
            name: "Interface de rede caiu (UP ➔ DOWN)",
            description: "Queda de link detectada na coleta SNMP das interfaces administrativamente habilitadas.",
            category: "interfaces",
            rule_type: "custom",
            condition: json!({
                "field": fields::INTERFACE_STATUS_TRANSITION,
                "operator": "eq",
                "value": interface_status_transition::WENT_DOWN,
            }),
            severity: "warning",
            duration_seconds: 0,
            recovery_window_seconds: 120,
            flap_threshold: 5,
            flap_window_seconds: 900,
            notification_cooldown_seconds: 900,
            inhibit_when_parent_down: false,
            recommended: true,
        },
        AlertRuleTemplate {
            key: "interface_speed_downgrade",
            name: "Downgrade na negociação da interface",
            description: "A interface renegociou para uma velocidade menor (ex.: 1 Gbps ➔ 100 Mbps) — sintoma clássico de cabo ou porta com defeito.",
            category: "interfaces",
            rule_type: "custom",
            condition: json!({
                "field": fields::INTERFACE_SPEED_TRANSITION,
                "operator": "eq",
                "value": interface_speed_transition::DOWNGRADE,
            }),
            severity: "warning",
            duration_seconds: 0,
            recovery_window_seconds: 120,
            flap_threshold: 5,
            flap_window_seconds: 900,
            notification_cooldown_seconds: 900,
            inhibit_when_parent_down: false,
            recommended: true,
        },
        AlertRuleTemplate {
            key: "interface_speed_upgrade",
            name: "Interface renegociou velocidade para cima",
            description: "Registro informativo de renegociação para uma velocidade maior (ex.: 100 Mbps ➔ 1 Gbps).",
            category: "interfaces",
            rule_type: "custom",
            condition: json!({
                "field": fields::INTERFACE_SPEED_TRANSITION,
                "operator": "eq",
                "value": interface_speed_transition::UPGRADE,
            }),
            severity: "info",
            duration_seconds: 0,
            recovery_window_seconds: 0,
            flap_threshold: 0,
            flap_window_seconds: 900,
            notification_cooldown_seconds: 0,
            inhibit_when_parent_down: false,
            recommended: false,
        },
        AlertRuleTemplate {
            key: "interface_link_recovered",
            name: "Interface voltou a operar (DOWN ➔ UP)",
            description: "Registro informativo do retorno do link após uma queda.",
            category: "interfaces",
            rule_type: "custom",
            condition: json!({
                "field": fields::INTERFACE_STATUS_TRANSITION,
                "operator": "eq",
                "value": interface_status_transition::CAME_BACK,
            }),
            severity: "info",
            duration_seconds: 0,
            recovery_window_seconds: 0,
            flap_threshold: 0,
            flap_window_seconds: 900,
            notification_cooldown_seconds: 0,
            inhibit_when_parent_down: false,
            recommended: false,
        },
        AlertRuleTemplate {
            key: "interface_below_gigabit",
            name: "Interface negociada abaixo de 1 Gbps",
            description: "Vigia links que deveriam operar em gigabit e estão negociando abaixo disso de forma contínua.",
            category: "interfaces",
            rule_type: "custom",
            condition: json!({
                "field": fields::INTERFACE_SPEED_BPS,
                "operator": "lt",
                "value": 1_000_000_000_i64,
            }),
            severity: "info",
            duration_seconds: 0,
            recovery_window_seconds: 0,
            flap_threshold: 0,
            flap_window_seconds: 900,
            notification_cooldown_seconds: 0,
            inhibit_when_parent_down: false,
            recommended: false,
        },
        AlertRuleTemplate {
            key: "snmp_interface_oper_down",
            name: "Interface monitorada via SNMP inativa",
            description: "Para monitores do tipo SNMP que leem ifOperStatus diretamente: 2 significa interface inativa.",
            category: "equipamento",
            rule_type: "custom",
            // O `"2"` é string de propósito: `eq` compara sem coerção (§8.7).
            condition: json!({ "field": fields::IF_OPER_STATUS, "operator": "eq", "value": "2" }),
            severity: "warning",
            duration_seconds: 0,
            recovery_window_seconds: 0,
            flap_threshold: 0,
            flap_window_seconds: 900,
            notification_cooldown_seconds: 900,
            inhibit_when_parent_down: false,
            recommended: false,
        },
        AlertRuleTemplate {
            key: "snmp_device_restarted",
            name: "Equipamento reiniciado recentemente",
            description: "Uptime SNMP abaixo de 10 minutos indica que o equipamento reiniciou (queda de energia, travamento ou reboot).",
            category: "equipamento",
            rule_type: "custom",
            // sysUpTime é reportado em centésimos de segundo: 60.000 = 10 minutos.
            condition: json!({ "field": fields::SNMP_UPTIME, "operator": "lt", "value": 60_000 }),
            severity: "warning",
            duration_seconds: 0,
            recovery_window_seconds: 0,
            flap_threshold: 0,
            flap_window_seconds: 900,
            notification_cooldown_seconds: 900,
            inhibit_when_parent_down: false,
            recommended: false,
        },
        AlertRuleTemplate {
            key: "vpn_peer_disconnected",
            name: "Túnel VPN caiu",
            description: "O equipamento remoto parou de responder pelo túnel WireGuard. Enquanto o túnel estiver fora, o monitoramento por trás dele fica cego.",
            category: "vpn",
            rule_type: "custom",
            condition: json!({
                "field": fields::VPN_STATUS_TRANSITION,
                "operator": "eq",
                "value": vpn_status_transition::DISCONNECTED,
            }),
            severity: "critical",
            duration_seconds: 0,
            recovery_window_seconds: 120,
            flap_threshold: 5,
            flap_window_seconds: 900,
            notification_cooldown_seconds: 900,
            inhibit_when_parent_down: false,
            recommended: true,
        },
        AlertRuleTemplate {
            key: "vpn_peer_unstable",
            name: "Túnel VPN instável",
            description: "O túnel ainda responde, mas os keepalives estão falhando — sintoma de link ruim ou NAT reciclando a porta do peer.",
            category: "vpn",
            rule_type: "custom",
            condition: json!({
                "field": fields::VPN_STATUS_TRANSITION,
                "operator": "eq",
                "value": vpn_status_transition::DESTABILIZED,
            }),
            severity: "warning",
            duration_seconds: 0,
            recovery_window_seconds: 120,
            flap_threshold: 5,
            flap_window_seconds: 900,
            notification_cooldown_seconds: 900,
            inhibit_when_parent_down: false,
            recommended: false,
        },
        AlertRuleTemplate {
            key: "vpn_peer_reconnected",
            name: "Túnel VPN restabelecido",
            description: "Registro informativo do retorno do túnel após uma queda.",
            category: "vpn",
            rule_type: "custom",
            condition: json!({
                "field": fields::VPN_STATUS_TRANSITION,
                "operator": "eq",
                "value": vpn_status_transition::RECONNECTED,
            }),
            severity: "info",
            duration_seconds: 0,
            recovery_window_seconds: 0,
            flap_threshold: 0,
            flap_window_seconds: 900,
            notification_cooldown_seconds: 0,
            inhibit_when_parent_down: false,
            recommended: false,
        },
    ]
}

/// Um template pela chave.
#[must_use]
pub fn find(key: &str) -> Option<AlertRuleTemplate> {
    all().into_iter().find(|template| template.key == key)
}

/// Os templates do conjunto básico (`recommended`).
#[must_use]
pub fn recommended() -> Vec<AlertRuleTemplate> {
    all().into_iter().filter(|item| item.recommended).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn o_catalogo_tem_os_dezoito_templates_do_roadmap() {
        assert_eq!(all().len(), 18);
    }

    #[test]
    fn sete_templates_compoem_o_conjunto_basico() {
        assert_eq!(recommended().len(), 7);
    }

    #[test]
    fn as_chaves_sao_unicas() {
        let keys: HashSet<&str> = all().iter().map(|template| template.key).collect();
        assert_eq!(keys.len(), 18);
    }

    #[test]
    fn toda_categoria_declarada_tem_rotulo() {
        let labels: HashSet<&str> = CATEGORY_LABELS.iter().map(|(key, _)| *key).collect();
        for template in all() {
            assert!(
                labels.contains(template.category),
                "categoria sem rótulo: {}",
                template.category
            );
        }
        assert_eq!(labels.len(), 6);
    }

    #[test]
    fn toda_condicao_e_legivel_pelo_avaliador() {
        use crate::services::alerts::evaluator::AlertRuleCondition;
        for template in all() {
            let condition = AlertRuleCondition::from_json(&template.condition)
                .unwrap_or_else(|| panic!("condição ilegível em {}", template.key));
            assert!(
                condition.operator.is_some(),
                "operador desconhecido em {}",
                template.key
            );
        }
    }

    #[test]
    fn o_template_de_snmp_compara_com_string() {
        // Matriz de paridade #28: trocar por `2` numérico silenciaria a regra.
        let template = find("snmp_interface_oper_down").expect("template existe");
        assert_eq!(template.condition["value"], json!("2"));
    }

    #[test]
    fn janelas_revisadas_na_fase_2() {
        let window = |key: &str| find(key).expect("template existe").recovery_window_seconds;
        // Degradação sustentada: janela casa com a tolerância de disparo.
        assert_eq!(window("packet_loss_high"), 300);
        assert_eq!(window("latency_high"), 300);
        assert_eq!(window("latency_critical"), 300);
        assert_eq!(window("dns_resolution_slow"), 300);
        // Transições de interface/túnel: janela curta.
        assert_eq!(window("interface_link_down"), 120);
        assert_eq!(window("interface_speed_downgrade"), 120);
        assert_eq!(window("vpn_peer_disconnected"), 120);
        assert_eq!(window("vpn_peer_unstable"), 120);
        // Indisponibilidade: revista na Fase 3 (ver o comentário do template).
        assert_eq!(window("device_offline"), 120);
        // Registros informativos e de estado: resolvem na hora.
        assert_eq!(window("http_error_response"), 0);
        assert_eq!(window("vpn_peer_reconnected"), 0);
    }

    #[test]
    fn a_deteccao_de_flapping_acompanha_a_janela_de_estabilidade() {
        // A detecção acontece sobre o episódio, que só sobrevive à oscilação
        // quando há janela: limiar sem janela seria letra morta.
        for template in all() {
            assert_eq!(
                template.flap_window_seconds, 900,
                "{} deve usar a janela padrão de 15 min",
                template.key
            );
            let esperado = i32::from(template.recovery_window_seconds > 0) * 5;
            assert_eq!(
                template.flap_threshold, esperado,
                "{} tem janela {} e limiar {}",
                template.key, template.recovery_window_seconds, template.flap_threshold
            );
        }
    }

    #[test]
    fn o_cooldown_freia_problema_e_deixa_o_informativo_em_paz() {
        for template in all() {
            let esperado = if template.severity == "info" { 0 } else { 900 };
            assert_eq!(
                template.notification_cooldown_seconds, esperado,
                "{} é {} e deveria ter cooldown {esperado}",
                template.key, template.severity
            );
        }
    }

    #[test]
    fn a_inibicao_vale_para_quem_mede_alcance_ao_alvo() {
        // Um roteador caído derruba ping, HTTP, TCP e DNS de tudo que está
        // atrás dele. Já o estado das interfaces e dos túneis descreve o
        // próprio equipamento: ali o pai não explica o filho.
        const ALCANCE: [&str; 3] = ["disponibilidade", "desempenho", "servicos"];
        for template in all() {
            assert_eq!(
                template.inhibit_when_parent_down,
                ALCANCE.contains(&template.category),
                "{} está na categoria {} e tem inibição {}",
                template.key,
                template.category,
                template.inhibit_when_parent_down
            );
        }
    }

    #[test]
    fn serializa_em_camel_case_para_o_frontend() {
        let template = find("device_offline").expect("template existe");
        let json = serde_json::to_value(&template).unwrap();
        assert_eq!(json["durationSeconds"], 0);
        assert_eq!(json["recoveryWindowSeconds"], 120);
        assert_eq!(json["flapThreshold"], 5);
        assert_eq!(json["flapWindowSeconds"], 900);
        assert_eq!(json["notificationCooldownSeconds"], 900);
        assert_eq!(json["inhibitWhenParentDown"], true);
        assert_eq!(json["type"], "device_offline");
        assert!(json.get("duration_seconds").is_none());
        assert!(json.get("flap_threshold").is_none());
        assert!(json.get("notification_cooldown_seconds").is_none());
        assert!(json.get("ruleType").is_none());
    }
}
