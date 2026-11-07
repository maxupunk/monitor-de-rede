//! Base de Conhecimento embutida sobre o NetMonitor.
//!
//! Fornece documentação técnica, manuais de operação e guias de troubleshooting
//! que são indexados e injetados nas consultas da IA para respostas precisas sobre o sistema.

/// Guia de regras de alerta: funcionamento, vocabulário e receitas. Um só
/// texto para a busca na documentação e para `get_alert_rules_guide`.
pub const ALERT_RULES_GUIDE: &str = include_str!("regras_de_alerta.md");

/// Skill de autoria de plugins de dispositivo: como a IA cria, testa e usa um
/// "driver" de equipamento. Servida por `get_plugin_authoring_guide`. É só da
/// IA — a documentação de **uso** de cada plugin viaja no próprio pacote.
pub const PLUGIN_AUTHORING_GUIDE: &str = include_str!("plugins.md");

/// O prompt do revisor de segurança dos plugins (`plugins::review`),
/// versionado junto com o código.
pub const PLUGIN_SECURITY_REVIEW: &str = include_str!("plugin_security_review.md");

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
        id: "dashboard_layout",
        title: "Layout do Dashboard (cards)",
        keywords: &["dashboard", "painel", "card", "cards", "widget", "widgets", "layout", "tela inicial", "organizar", "reorganizar"],
        content: "O dashboard é montado com cards (widgets) num layout compartilhado, gravado no servidor e sincronizado em tempo real entre as abas:
- Cards únicos (resumo estatístico, saúde global, alertas ativos, latência, distribuição de eventos, alvos instáveis, feed de eventos, monitores de rede, resumo Docker, serviços SaaS) e cards que aceitam várias instâncias, cada uma com a sua configuração (banda de interface, banda vs latência, CPU, RAM, status binário, mapa de calor SaaS, latência DNS).
- Tamanhos: um terço, metade, dois terços ou largura total.
- Na tela: botão de edição do dashboard (mover, remover, catálogo de cards). Pelo chat: a IA lê o layout com get_dashboard_layout e propõe mudanças com update_dashboard_layout (adicionar, remover, mover, redimensionar, configurar ou restaurar o padrão) — gravadas só depois da confirmação do usuário, a cada vez.
- Remover um card único só o oculta (volta pelo catálogo); remover uma instância a apaga.",
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
        keywords: &["alerta", "alertas", "regra", "regras", "condicao", "limiar", "notificacao", "notificacoes", "webpush", "telegram", "discord", "webhook", "flap", "flapping", "severidade"],
        content: ALERT_RULES_GUIDE,
    },
    DocTopic {
        id: "device_plugins",
        title: "Plugins de Dispositivo (drivers por SSH/HTTP)",
        keywords: &["plugin", "plugins", "driver", "drivers", "script", "ssh", "telnet", "http", "porta 80", "openwrt", "opkg", "instalar pacote", "credencial", "extensao", "rhai"],
        content: "Plugins de dispositivo agem sobre o equipamento por SSH, HTTP (interface web) ou Telnet:
- Ficam na aba Plugins de /devices/{id}. Um plugin de modelo serve a todos os equipamentos compatíveis (sistema, modelo, firmware); um exclusivo serve só a um.
- Cada plugin traz manifesto (ações com efeito leitura/escrita), script Rhai em sandbox, documentação de uso, lista de compatibilidade e testes (unitários com respostas gravadas e funcionais no equipamento real).
- Ciclo: rascunho → testado (testes unitários aprovados) → ativo (o operador ativa). Plugin importado entra em quarentena e passa por revisão de segurança (análise estática + IA) antes de ser instalado; risco crítico bloqueia.
- Credenciais SSH/HTTP/Telnet ficam cifradas (ENCRYPTION_KEY) ou são pedidas a cada sessão; a senha nunca volta para a tela nem para a IA.
- Plugin que não está ativo pede aprovação a cada acesso. A IA sempre pede confirmação a cada acesso, exceto com o modo \"Aceitar automaticamente\" ligado para aquela conversa e equipamento, com termo de ciência.
- Equipamento atrás de agente remoto: o agente precisa liberar device_io no AGENT_ALLOW.",
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
- Servidor Syslog nativo escutando em UDP/TCP 514 (ou 5514 no container) com detecção automática de conexões NAT e vinculação a dispositivos.
- Monitoramento de Docker Engine: monitora containers (CPU, memória, status de saúde, OOMKilled exit 137), imagens e volumes em tempo real. Todo host Docker é atendido por um agente NetMonitor: o desta central é o agente local (serviço netmonitor-agent, protegido pela label com.netmonitor.protected=true), os demais são agentes remotos.",
    },
    DocTopic {
        id: "backups_storage",
        title: "Sistema de Backup, Bancos de Dados e Destinos de Storage",
        keywords: &[
            "backup", "backups", "restore", "restauracao", "postgres", "postgresql", "mysql",
            "mariadb", "dump", "sql.gz", "storage", "s3", "minio", "r2", "gcs", "azure", "sftp",
            "retencao", "sha256", "checksum",
        ],
        content: "O NetMonitor possui dois subsistemas de backup e uma camada unificada de Storage:
- Backup de Configuração do NetMonitor (services::backup): exporta e restaura o cadastro do sistema (dispositivos, monitores, credenciais cifradas, regras de alerta, topologia, destinos de storage) com simulação prévia (plan), agendamento e retenção automática (retention_count). Durante o restore, o coletor de monitores é pausado automaticamente.
- Backup de Bancos de Dados Externos (services::databases): realiza dumps lógicos nativos em Rust comprimidos em .sql.gz com hash SHA-256 sem depender de binários externos (não usa pg_dump nem mysqldump):
  * PostgreSQL (porta 5432): usa protocolo nativo e COPY TO STDOUT.
  * MySQL e MariaDB (porta 3306): usa SHOW CREATE TABLE e streaming binário de linhas.
  * Ponte via Agente Remoto (ADR 013, campo 'via'): quando o banco fica em rede privada sem rota direta do servidor central, o agente remoto abre o túnel TCP até o banco enquanto o servidor central executa o dump e grava no storage.
  * Modos de Restore: NewDatabase (cria banco novo) ou Replace (substitui o banco existente).
- Destinos de Storage (services::storage): suporta 'local' (diretório controlado por STORAGE_LOCAL_ROOT, padrão /data/storage no container), 'aws_s3', 'minio' (requer force_path_style), 'cloudflare_r2', 's3_compatible', 'google_gcs', 'azure_blob' e 'sftp' (SSH nativo via russh na porta 22).",
    },
    DocTopic {
        id: "wifi_signals_80211",
        title: "Monitoramento de Sinais Wi-Fi e Padrões IEEE 802.11",
        keywords: &[
            "wifi", "wi-fi", "wireless", "802.11", "802.11n", "802.11ac", "802.11ax", "802.11be",
            "rssi", "snr", "ruido", "retransmissao", "retries", "openwrt-wifi", "power_save",
            "ofdma", "mu-mimo", "canal", "dfs",
        ],
        content: "Monitoramento de Sinais Wi-Fi (docs/monitoramento_wifi_sinais.md) e Padrões IEEE 802.11:
- Coleta passiva a cada 60s via plugin 'openwrt-wifi' (papéis 'clients' com iwinfo/ubus/ARP e 'ping' sob demanda) ou SNMP/SSH.
- Classificação de Sinal (RSSI em dBm): Excelente (>= -60 dBm), Bom (-61 a -70 dBm), Atenção (-71 a -78 dBm), Ruim (-79 a -84 dBm) e Crítico (<= -85 dBm).
- Relação Sinal-Ruído (SNR = signal_dbm - noise_dbm): Excelente (>= 30 dB), Bom (25 a 29 dB), Atenção (15 a 24 dB), Ruim (10 a 14 dB) e Crítico (< 10 dB).
- Taxa de Retransmissão (tx_retries / (tx_packets + tx_retries) * 100): Excelente (< 10%), Aceitável (10% a 25%), Degradado (> 25%). Quando o sinal é bom (>= -70 dBm) mas a retransmissão passa de 25%, o diagnóstico é 'high_retries' (interferência co-canal, canal 2.4 GHz congestionado fora do 1/6/11 ou colisão de pacotes).
- Diagnósticos automáticos (WifiHealth): 'healthy', 'power_save' (inativo > 10s com sinal >= -72 dBm, economia de bateria normal de smartphones), 'weak_signal', 'high_retries' e 'stale' (inativo > 60s).
- Clientes Vigiados (watched): geram alertas 'wifi_watched_offline' (ausente >= 600s / 10 min) e 'wifi_watched_poor_signal' (sinal ruim/crítico sustentado por 15 min).
- Padrões IEEE 802.11: 802.11n (Wi-Fi 4, 2.4/5 GHz, MIMO 40 MHz), 802.11ac (Wi-Fi 5, 5 GHz, 80/160 MHz, 256-QAM, DL MU-MIMO), 802.11ax (Wi-Fi 6/6E, 2.4/5/6 GHz, OFDMA, UL/DL MU-MIMO, 1024-QAM, BSS Coloring, TWT) e 802.11be (Wi-Fi 7, 320 MHz, 4096-QAM, MLO).",
    },
    DocTopic {
        id: "optical_fiber_gpon",
        title: "Sinais de Fibra Óptica, GPON/EPON e Transceivers SFP/SFP+",
        keywords: &[
            "fibra", "optica", "gpon", "epon", "xgs-pon", "olt", "onu", "ont", "dbm", "sfp",
            "atenuacao", "splitter", "los", "dying gasp", "1310", "1490", "1550", "pon",
        ],
        content: "Referência de Sinais Ópticos (GPON / EPON / enlaces PtP SFP+) no monitoramento de OLTs, switches e roteadores:
- Unidades: 'dBm' mede potência óptica absoluta referenciada a 1 mW; 'dB' mede perda/atenuação relativa (orçamento óptico).
- Comprimentos de onda GPON: 1490 nm no downstream (OLT -> ONU, atenuação ~0,22 dB/km), 1310 nm no upstream (ONU -> OLT, atenuação ~0,35 dB/km) e 1550 nm (RF/DWDM, muito sensível a macrocurvatura/dobras no cordão óptico).
- Perda típica de inserção de splitters ópticos PLC balanceados: 1:2 (~3,5 dB), 1:4 (~7,2 dB), 1:8 (~10,5 dB), 1:16 (~13,8 dB), 1:32 (~17,2 dB) e 1:64 (~20,5 dB). Fusão: ~0,05 a 0,1 dB; conector SC/APC ou SC/UPC: ~0,3 a 0,5 dB.
- Faixas de Potência RX na ONU (Classe B+ / C+):
  * Normal / Ideal: -15 dBm a -24 dBm.
  * Atenção / Limiar de degradação: -25 dBm a -27 dBm (sujeito a erros de FEC/CRC e quedas térmicas).
  * Crítico / Queda de link (LOS): <= -28 dBm.
  * Saturação / Sobrecarga do fotodiodo: > -8 dBm (exige atenuador óptico).
- Diagnóstico de Alarme: 'Dying Gasp' indica falta de energia elétrica na ONU do cliente (desligamento gracioso anunciado antes de apagar), enquanto 'LOS' (Loss of Signal) ou 'LOFi' sem Dying Gasp indica rompimento de fibra, conector sujo/desconectado ou atenuação extrema.",
    },
    DocTopic {
        id: "link_tests_btest",
        title: "Testes de Capacidade de Link e BTest (ADR 015)",
        keywords: &["btest", "bandwidth test", "link", "capacidade", "throughput", "porta 2000", "mikrotik"],
        content: "Testes de Capacidade de Enlace (ADR 015):
- O NetMonitor suporta testes de banda ativos entre roteadores MikroTik (Bandwidth Test / btest na porta TCP/UDP 2000) e testes WAN/LAN.
- Como testes ativos de throughput podem saturar links em produção e elevar latência/perda temporariamente, eles exigem janela curta controlada e verificação prévia do tráfego das interfaces.",
    },
    DocTopic {
        id: "web_search",
        title: "Pesquisa Web da IA (web_search)",
        keywords: &["web_search", "pesquisa web", "internet", "cve", "firmware", "bug", "fabricante", "forum", "searxng"],
        content: "Ferramenta 'web_search' (docs/pesquisa_web_ia_frontend.md):
- Quando o diagnóstico encontra um código de erro de fabricante, bug de firmware (RouterOS, OpenWrt, Huawei VRP, Cisco IOS-XE) ou CVE que não consta na base interna (search_system_docs), a IA aciona 'web_search(query, domain)'.
- A própria central pesquisa, no chat e nas rotinas automáticas (resumo de incidente, resumo periódico, tarefas agendadas): DuckDuckGo por padrão, ou um SearXNG próprio em AI_WEB_SEARCH_URL; CVEs citados vêm também do NVD (descrição e nota CVSS).
- Antes de sair, o termo perde IPs internos, MACs, credenciais, e-mails e chaves; os resultados voltam como texto puro (até 5, com título, url e trecho) e ficam 1 h em memória.
- Sem internet, a ferramenta avisa na hora e a IA segue com os dados do NetMonitor. AI_WEB_SEARCH=off desliga a pesquisa.",
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
