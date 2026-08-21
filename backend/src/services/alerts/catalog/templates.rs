//! Catálogo de regras pré-configuradas (§8.7).
//!
//! É a única fonte de verdade das políticas de alerta que antes viviam
//! espalhadas no código (ex.: downgrade de negociação de interface). Cada
//! template é apenas *dado*: quem aplica, avalia e dispara não precisa ser
//! alterado para nascer uma política nova — basta acrescentar um item aqui.

use serde::Serialize;
use serde_json::{json, Value};

use crate::services::{
    alerts::fields::{
        self, interface_speed_transition, interface_status_transition, vpn_status_transition,
    },
    syslog::matcher::RULE_TYPE as LOG_PATTERN_TYPE,
};

/// Monta um template de padrão de log.
///
/// A `condition` carrega duas coisas ao mesmo tempo: `field`/`operator`/`value`
/// para o avaliador e `pattern`/`minSeverity`/`windowSeconds` para o matcher.
/// O `AlertRuleCondition::from_json` ignora o que não conhece, então as duas
/// convivem sem coluna nova no banco.
///
/// A regex é deliberadamente **case-insensitive** (`(?i)`): fabricante nenhum
/// combina a caixa das mensagens, e um padrão que só casa em minúsculas passa
/// despercebido no equipamento do vizinho.
struct LogPatternSpec {
    key: &'static str,
    name: &'static str,
    description: &'static str,
    /// Regex sem o `(?i)`, que a função acrescenta.
    pattern: &'static str,
    /// Quantas ocorrências na janela para a regra bater.
    ocorrencias: i64,
    janela: i64,
    severity: &'static str,
    recommended: bool,
}

fn log_pattern(spec: LogPatternSpec) -> AlertRuleTemplate {
    let LogPatternSpec {
        key,
        name,
        description,
        pattern,
        ocorrencias,
        janela,
        severity,
        recommended,
    } = spec;
    AlertRuleTemplate {
        key,
        name,
        description,
        category: "logs",
        rule_type: LOG_PATTERN_TYPE,
        condition: json!({
            "field": fields::LOG_MATCH_COUNT,
            "operator": "gte",
            "value": ocorrencias,
            "pattern": format!("(?i){pattern}"),
            "windowSeconds": janela,
        }),
        severity,
        // O sustento já vem da janela deslizante ("N vezes em M minutos"):
        // exigir também `duration_seconds` pediria que a condição se
        // mantivesse por mais um período depois de já ter se sustentado.
        duration_seconds: 0,
        // A janela do matcher já é a memória do episódio: o alerta só resolve
        // quando a contagem zera, o que exige M minutos sem nenhuma ocorrência.
        recovery_window_seconds: 0,
        flap_threshold: 0,
        flap_window_seconds: 900,
        // Log repetido é o caso clássico de fadiga de notificação: um roteador
        // sob ataque de força bruta gera centenas de linhas por minuto. O
        // informativo fica de fora, como no resto do catálogo: ele não freia
        // nada porque não interrompe ninguém.
        notification_cooldown_seconds: if severity == "info" { 0 } else { 900 },
        inhibit_when_parent_down: false,
        recommended,
    }
}

/// As seis categorias exibidas na tela, com os rótulos em português.
pub const CATEGORY_LABELS: [(&str, &str); 8] = [
    ("disponibilidade", "Disponibilidade"),
    ("desempenho", "Desempenho"),
    ("servicos", "Serviços e aplicações"),
    ("interfaces", "Interfaces de rede (SNMP)"),
    ("equipamento", "Equipamento (SNMP)"),
    ("saude", "Saúde do equipamento"),
    ("vpn", "Túneis VPN (WireGuard)"),
    ("logs", "Padrões no log (syslog)"),
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
            key: "latency_baseline_deviation",
            name: "Latência 50% acima da baseline",
            description: "A latência atual ultrapassou em 50% a média histórica de 7 dias — indicativo de degradação progressiva do link.",
            category: "desempenho",
            rule_type: "custom",
            condition: json!({ "field": fields::LATENCY_DEVIATION_PERCENT, "operator": "gt", "value": 50 }),
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
            key: "packet_loss_baseline_deviation",
            name: "Perda de pacotes acima da baseline",
            description: "A perda de pacotes atual superou em 10 pontos percentuais a média histórica de 7 dias.",
            category: "disponibilidade",
            rule_type: "custom",
            condition: json!({ "field": fields::PACKET_LOSS_DEVIATION_PERCENT, "operator": "gt", "value": 10 }),
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
            key: "uptime_baseline_deviation",
            name: "Uptime abaixo da baseline",
            description: "O uptime de 24 horas caiu 2 pontos percentuais abaixo da média histórica de 7 dias — link menos estável que o habitual.",
            category: "disponibilidade",
            rule_type: "custom",
            condition: json!({ "field": fields::UPTIME_DEVIATION_PERCENT, "operator": "gt", "value": 2 }),
            severity: "critical",
            duration_seconds: 300,
            recovery_window_seconds: 300,
            flap_threshold: 5,
            flap_window_seconds: 900,
            notification_cooldown_seconds: 900,
            inhibit_when_parent_down: true,
            recommended: true,
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
        // --- Padrões no log (Fase 6 do roadmap de syslog) -------------------
        log_pattern(LogPatternSpec {
            key: "log_login_failure",
            name: "Falhas de login no equipamento",
            description: "Cinco tentativas de autenticação recusadas em cinco minutos — força bruta ou credencial desatualizada em algum sistema.",
            pattern: r"login failure|failed password|authentication fail|invalid user",
            ocorrencias: 5,
            janela: 300,
            severity: "warning",
            recommended: true,
        }),
        log_pattern(LogPatternSpec {
            key: "log_system_started",
            name: "Equipamento reiniciou",
            description: "O roteador anunciou que subiu. Reinício não programado costuma ser queda de energia, travamento ou atualização automática.",
            pattern: r"system started|router (re)?booted|starting up",
            ocorrencias: 1,
            janela: 300,
            severity: "critical",
            recommended: true,
        }),
        log_pattern(LogPatternSpec {
            key: "log_routing_down",
            name: "Vizinhança de roteamento caiu",
            description: "OSPF ou BGP perdeu adjacência. Costuma preceder a perda de rota para uma faixa inteira da rede.",
            pattern: r"ospf.*(down|lost|expired)|bgp.*(down|closing|reset)|neighbor.*down",
            ocorrencias: 1,
            janela: 300,
            severity: "critical",
            recommended: true,
        }),
        log_pattern(LogPatternSpec {
            key: "log_pppoe_flapping",
            name: "PPPoE caindo repetidamente",
            description: "Três quedas de sessão PPPoE em dez minutos. Link do provedor instável, e não queda limpa.",
            pattern: r"pppoe.*(terminat|disconnect|down)|ppp.*link.*(terminated|down)",
            ocorrencias: 3,
            janela: 600,
            severity: "warning",
            recommended: true,
        }),
        log_pattern(LogPatternSpec {
            key: "log_dhcp_pool_exhausted",
            name: "Pool DHCP esgotado",
            description: "Não há mais endereço para entregar. Cliente novo na rede simplesmente não conecta, sem erro visível para o usuário.",
            pattern: r"no more (free )?addresses|pool.*(exhaust|full)|dhcp.*no free",
            ocorrencias: 1,
            janela: 600,
            severity: "critical",
            recommended: true,
        }),
        log_pattern(LogPatternSpec {
            key: "log_out_of_memory",
            name: "Equipamento sem memória",
            description: "O roteador relatou falta de memória. É o aviso que antecede travamento e reinício.",
            pattern: r"out of memory|no memory|memory (low|exhaust)|cannot allocate",
            ocorrencias: 1,
            janela: 600,
            severity: "critical",
            recommended: true,
        }),
        log_pattern(LogPatternSpec {
            key: "log_config_changed",
            name: "Configuração alterada",
            description: "Alguém mudou a configuração do equipamento. Não é falha: é rastro de auditoria, útil para correlacionar com um problema que começou logo depois.",
            pattern: r"config(uration)? (changed|saved|written)|commit.*complete",
            ocorrencias: 1,
            janela: 300,
            severity: "info",
            recommended: false,
        }),
        // --- Saúde do equipamento -------------------------------------------
        //
        // Não são "regras do servidor". São regras de **dispositivo**: valem
        // para qualquer equipamento que publique os campos — o NetMonitor pela
        // coleta local, um roteador pelo dataset do SNMP. `recommended: false`
        // de propósito: o conjunto básico é global, e uma regra de CPU sem
        // escopo dispararia para o parque inteiro. Quem as aplica é o catálogo
        // por dispositivo, que sabe se o campo existe ali.
        AlertRuleTemplate {
            key: "cpu_usage_high",
            name: "CPU acima de 85%",
            description: "Uso de CPU sustentado por 5 minutos. Pico curto não alerta — só a carga que persiste.",
            category: "saude",
            rule_type: "custom",
            condition: json!({ "field": fields::CPU_USAGE_PERCENT, "operator": "gt", "value": 85 }),
            severity: "warning",
            duration_seconds: 300,
            recovery_window_seconds: 300,
            flap_threshold: 5,
            flap_window_seconds: 900,
            notification_cooldown_seconds: 900,
            // A saúde descreve o estado do próprio equipamento: o pai caído não
            // explica a CPU do filho, e calar aqui seria esconder.
            inhibit_when_parent_down: false,
            recommended: false,
        },
        AlertRuleTemplate {
            key: "memory_usage_high",
            name: "Memória usada acima de 90%",
            description: "Memória sustentada acima de 90% por 5 minutos — a faixa em que o sistema começa a recuperar página à força.",
            category: "saude",
            rule_type: "custom",
            condition: json!({ "field": fields::MEMORY_USED_PERCENT, "operator": "gt", "value": 90 }),
            severity: "warning",
            duration_seconds: 300,
            recovery_window_seconds: 300,
            flap_threshold: 5,
            flap_window_seconds: 900,
            notification_cooldown_seconds: 900,
            inhibit_when_parent_down: false,
            recommended: false,
        },
        AlertRuleTemplate {
            key: "storage_usage_high",
            name: "Armazenamento usado acima de 85%",
            description: "Disco enchendo de forma sustentada por 10 minutos. A janela é maior porque armazenamento não oscila: quando sobe, é para ficar.",
            category: "saude",
            rule_type: "custom",
            condition: json!({ "field": fields::STORAGE_USED_PERCENT, "operator": "gt", "value": 85 }),
            severity: "warning",
            duration_seconds: 600,
            recovery_window_seconds: 600,
            flap_threshold: 5,
            flap_window_seconds: 900,
            notification_cooldown_seconds: 900,
            inhibit_when_parent_down: false,
            recommended: false,
        },
    ]
}

/// As chaves de saúde, que o catálogo por dispositivo aplica ao Servidor
/// NetMonitor no primeiro boot.
///
/// Ficam aqui, e não numa lista solta no serviço, porque é aqui que os
/// templates existem: acrescentar um quarto template de saúde e esquecer de
/// listá-lo seria um erro invisível.
pub const HEALTH_KEYS: [&str; 3] = ["cpu_usage_high", "memory_usage_high", "storage_usage_high"];

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

    /// 18 do roadmap de alertas + 7 padrões de log (Fase 6 do roadmap de
    /// syslog) + 3 de saúde de equipamento (Fase 3 do roadmap do servidor como
    /// dispositivo) + 3 de baseline móvel (Fase 3 do roadmap mestre).
    const TOTAL_TEMPLATES: usize = 31;

    #[test]
    fn o_catalogo_tem_os_templates_dos_dois_roadmaps() {
        assert_eq!(all().len(), TOTAL_TEMPLATES);
    }

    /// 7 do conjunto original + 6 dos padrões de log + 3 de baseline móvel.
    /// Só `log_config_changed` fica de fora: é rastro de auditoria, não problema,
    /// e ligá-lo por padrão encheria a Central de alerta informativo.
    #[test]
    fn dezesseis_templates_compoem_o_conjunto_basico() {
        assert_eq!(recommended().len(), 16);
    }

    #[test]
    fn as_chaves_sao_unicas() {
        let keys: HashSet<&str> = all().iter().map(|template| template.key).collect();
        assert_eq!(keys.len(), TOTAL_TEMPLATES);
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
        assert_eq!(labels.len(), 8);
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
