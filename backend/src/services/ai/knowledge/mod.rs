//! Base de Conhecimento embutida sobre o NetMonitor.
//!
//! Fornece documentação técnica, manuais de operação e guias de troubleshooting
//! que são indexados e injetados nas consultas da IA para respostas precisas sobre o sistema.

#[derive(Debug, Clone)]
pub struct DocTopic {
    pub id: &'static str,
    pub title: &'static str,
    pub keywords: &'static [&'static str],
    pub content: &'static str,
}

pub static SYSTEM_DOCS: &[DocTopic] = &[
    DocTopic {
        id: "overview",
        title: "Visão Geral do NetMonitor",
        keywords: &["netmonitor", "sistema", "inicio", "dashboard", "visao geral", "como funciona"],
        content: "O NetMonitor é um sistema corporativo de monitoramento de redes e infraestrutura desenvolvido em Rust (backend Loco.rs) e Vue 3/Vuetify (frontend). Ele oferece coleta de métricas em tempo real via SSE (Server-Sent Events), monitoramento contínuo de conectividade (ICMP/TCP/UDP/SNMP), descoberta automatizada de dispositivos, topologia de rede visual, gestão de alertas com detecção de flapping, servidor WireGuard VPN integrado e diagnósticos avançados com playbooks automatizados.",
    },
    DocTopic {
        id: "monitors",
        title: "Monitores e Coleta de Métricas",
        keywords: &["monitor", "monitores", "icmp", "ping", "tcp", "udp", "snmp", "intervalo", "threshold", "perda"],
        content: "Monitores são sondas periódicas associadas a dispositivos:
- Tipos de monitor:
  * ICMP Ping: mede latência (RTT em ms) e perda de pacotes (packet loss %). Utiliza socket nativo SOCK_DGRAM sem necessidade de root.
  * TCP Port: verifica se portas de serviço estão aceitando conexões (ex: 80, 443, 22, 3389).
  * UDP: envia pacotes UDP específicos e avalia resposta (ex: DNS porta 53).
  * SNMP: consulta OIDs e perfis SNMP (v1, v2c, v3) para ler tráfego de interfaces, CPU, memória e status.
- Intervalos: configuráveis por monitor (mínimo 10s, padrão 60s, máximo 86400s).
- Status possíveis: 'up' (operacional), 'down' (parado/inacessível), 'warning' (latência ou perda elevada), 'unknown' (sem coleta recente) e 'paused' (silenciado).",
    },
    DocTopic {
        id: "devices_discovery",
        title: "Dispositivos e Descoberta de Redes",
        keywords: &["dispositivo", "dispositivos", "descoberta", "discovery", "rede", "scan", "ip", "mac", "mikrotik", "mactelnet"],
        content: "O módulo de Descoberta mapeia novos equipamentos na rede:
- Varredura de sub-redes (CIDR) usando ICMP sweep e cache de vizinhos ARP.
- Identificação de fabricantes através da tabela OUI pelo endereço MAC.
- Detecção nativa de roteadores MikroTik através de broadcast MAC-Telnet.
- Cadastro manual ou adição com um clique a partir dos resultados de descoberta.",
    },
    DocTopic {
        id: "alerts_rules",
        title: "Regras de Alerta e Notificações",
        keywords: &["alerta", "alertas", "notificacao", "notificacoes", "webpush", "telegram", "discord", "webhook", "flap", "flapping"],
        content: "Central de Alertas e Notificações:
- Regras de Alerta definem condições para disparo de incidentes:
  * Severidades: 'critical', 'warning', 'info'.
  * Critérios: host inacessível ('down'), latência acima do limite por N amostras consecutivas, perda de pacotes acima da porcentagem estipulada.
  * Flap Detection: evita tempestade de notificações quando um link instável oscila rapidamente entre UP e DOWN.
- Canais de notificação:
  * Web Push (notificações push no navegador/PWA mesmo com app fechado).
  * Telegram Bot (envio para chats ou grupos).
  * Discord Webhook.
  * Webhook genérico HTTP POST com payload JSON assinado.",
    },
    DocTopic {
        id: "diagnostics",
        title: "Ferramentas de Diagnóstico de Rede",
        keywords: &["diagnostico", "traceroute", "speedtest", "velocidade", "playbook", "port scan", "portas", "dns"],
        content: "Recursos nativos de diagnóstico disponíveis na interface:
- Traceroute ICMP: traça a rota salto a salto até o destino com latência individual de cada salto e detecção de timeouts.
- Teste de Velocidade WAN: mede Download, Upload, Ping e Jitter contra a infraestrutura global da Cloudflare.
- Teste de Velocidade LAN: teste local de throughput de download e upload entre o navegador e o servidor NetMonitor.
- Playbooks Automatizados:
  * 'internet_health': checklist que valida DNS, ping externo (1.1.1.1), traceroute e teste de velocidade.
  * 'device_reachability': checklist direcionado a um IP que testa ICMP, portas TCP essenciais e traceroute.",
    },
    DocTopic {
        id: "vpn_wireguard",
        title: "VPN WireGuard e Probes Remotos",
        keywords: &["vpn", "wireguard", "peer", "servidor vpn", "probe", "agente", "10.8.0"],
        content: "Integração WireGuard e Monitoramento Distribuído:
- Servidor VPN integrado criando interface `wg0` na faixa `10.8.0.0/24`.
- Cadastro de peers (clientes) com geração de chaves criptográficas seguras (as chaves privadas nunca são salvas em banco, vivendo apenas em cofre volátil até o download da configuração).
- Scripts de provisionamento prontos para Linux, MikroTik RouterOS, Windows e macOS.
- VPN Probe (`vpn-probe`): agente remoto dedicado que executa coletas dentro de redes isoladas e reporta métricas para o servidor central através do túnel WireGuard.",
    },
    DocTopic {
        id: "maintenance",
        title: "Janelas de Manutenção",
        keywords: &["manutencao", "janela", "silenciar", "pausa", "agendamento"],
        content: "Janelas de Manutenção permitem programar períodos de intervenção técnica:
- Durante uma janela ativa, os alertas dos dispositivos ou monitores vinculados são automaticamente silenciados, impedindo notificações falsas para a equipe.
- Suporta janelas pontuais ou recorrentes (diárias, semanais, mensais).",
    },
    DocTopic {
        id: "syslog_docker",
        title: "Syslog e Monitoramento Docker",
        keywords: &["syslog", "log", "docker", "nat", "container", "containers", "imagem", "volume"],
        content: "Syslog e Docker:
- Servidor Syslog nativo escutando em UDP/TCP 514 com detecção automática de conexões NAT e vinculação a dispositivos.
- Monitoramento de Docker Engine: monitora containers (CPU, memória, status de saúde), imagens e volumes em tempo real via socket Docker.",
    },
];

/// Pesquisa na base de conhecimento e retorna os tópicos mais relevantes.
pub fn search_docs(query: &str) -> Vec<&'static DocTopic> {
    let q = query.to_lowercase();
    let words: Vec<&str> = q.split_whitespace().collect();

    if words.is_empty() {
        return SYSTEM_DOCS.iter().take(3).collect();
    }

    let mut scored: Vec<(&'static DocTopic, usize)> = SYSTEM_DOCS
        .iter()
        .map(|doc| {
            let mut score = 0;
            for word in &words {
                if doc.title.to_lowercase().contains(word) {
                    score += 5;
                }
                for kw in doc.keywords {
                    if kw.contains(word) || word.contains(kw) {
                        score += 3;
                    }
                }
                if doc.content.to_lowercase().contains(word) {
                    score += 1;
                }
            }
            (doc, score)
        })
        .filter(|(_, score)| *score > 0)
        .collect();

    scored.sort_by_key(|b| std::cmp::Reverse(b.1));
    scored.into_iter().map(|(doc, _)| doc).take(3).collect()
}
