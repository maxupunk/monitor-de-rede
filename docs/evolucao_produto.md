# Evolução de Produto — NetMonitor

> Este documento reúne oportunidades de crescimento do NetMonitor inspiradas em produtos do mesmo ramo (monitoramento de infraestrutura e redes) e em soluções de outros mercados que resolvem problemas similares.  
> **Última revisão:** 2026-08-21.

---

## 1. Contexto

O NetMonitor hoje monitora redes residenciais e de pequenas empresas através de:

- Descoberta e identificação de dispositivos na LAN/VPN.
- Checagens de disponibilidade (ICMP, HTTP, TCP, DNS, SNMP).
- Métricas, alertas inteligentes, notificações e syslog.
- Túnel WireGuard para acesso remoto e medição de redes externas.
- Probes distribuídos que reportam resultados via HTTP autenticado.

Isso cobre o básico bem. As próximas oportunidades de produto partem de três movimentos observados no mercado:

1. **Convergência de rede + IoT + automação** — quem cuida da rede também quer saber se a câmera travou, se a lâmpada responde e se o sensor de porta está offline.
2. **Observabilidade unificada** — métricas, logs, traces e eventos no mesmo painel, com correlação automática.
3. **Experiência mobile-first e assistida por IA** — alertas contextuais, diagnósticos automáticos e ações sugeridas no celular.

---

## 2. Oportunidades de produto

### 2.1 Integração com dispositivos IoT residenciais e industriais

#### 2.1.1 Descoberta e monitoramento de dispositivos Tuya / Smart Life

**Referências:** Tuya Smart Life, Home Assistant (integração Tuya), Hubitat.

| Aspecto | Proposta |
| :--- | :--- |
| O quê | Detectar lâmpadas, interruptores, tomadas, sensores e câmeras que usam a plataforma Tuya (incluindo marcas-branca). |
| Como | Descoberta via mDNS/Bonjour (`_tuya._tcp.local`) e, opcionalmente, integração com a API de desenvolvedor Tuya Cloud quando o usuário fornecer `client_id`, `secret` e `device_id`. |
| Métricas | Estado online/offline, RSSI Wi-Fi, temperatura de operação, consumo energético (tomadas inteligentes), status de bateria (sensores). |
| Alertas | "Tomada da sala de estar não responde há 5 min", "Sensor de porta com bateria abaixo de 20%", "Câmera do quintal desconectou". |
| Ações | Reinicialização remota de tomadas inteligentes, acionar cenas (ex.: piscar luzes quando um host crítico cair). |

**Por que agregaria valor:** muitas instalações residenciais já têm dezenas de dispositivos Tuya e não sabem quando param de responder. O NetMonitor poderia virar o "status da casa" além do "status da rede".

#### 2.1.2 Monitoramento de dispositivos Sonoff / eWeLink

**Referências:** eWeLink app, Sonoff LAN mode (DIY), Home Assistant integration.

| Aspecto | Proposta |
| :--- | :--- |
| O quê | Suportar dispositivos Sonoff em modo LAN (DIY mode) e, com credenciais opcionais, via API eWeLink. |
| Como | Modo LAN: descoberta via mDNS e controle REST local (`http://<ip>:8081/zeroconf/`). Modo cloud: API eWeLink com token de refresh. |
| Métricas | Estado do relé, temperatura, umidade, consumo (Sonoff POW), sinal Wi-Fi. |
| Alertas | "Sonoff POW do ar-condicionado não reporta consumo", "Interruptor do quarto não responde a ping". |
| Ações | Ligar/desligar relé como ação de alerta (ex.: reiniciar modem roteador via tomada inteligente quando a internet cair). |

#### 2.1.3 Dispositivos ESPHome e firmware aberto

**Referências:** ESPHome, Home Assistant, OpenMQTTGateway.

| Aspecto | Proposta |
| :--- | :--- |
| O quê | Detectar e monitorar dispositivos baseados em ESP8266/ESP32 rodando ESPHome, Tasmota, WLED ou firmware próprio. |
| Como | Descoberta via mDNS (`esphome.local.`, `tasmota-*.local.`) e API nativa REST/MQTT do ESPHome. |
| Métricas | RSSI, uptime, estado dos GPIOs, sensores conectados (temperatura, umidade, CO2, energia). |
| Alertas | "Sensor de CO2 do escritório parou de enviar leituras", "ESP32 da garagem reiniciou 3 vezes hoje". |
| Extensão | Permitir que o usuário cadastre templates YAML de sensores para novos modelos de ESP. |

#### 2.1.4 Shelly e dispositivos baseados em REST/MQTT

**Referências:** Shelly Cloud app, Shelly Gen2/Gen3 API.

| Aspecto | Proposta |
| :--- | :--- | 
| O quê | Suporte nativo a dispositivos Shelly (reles, dimmers, medidores de energia, sensores). |
| Como | API HTTP local (`/rpc/Switch.GetStatus`, `/rpc/Sys.GetStatus`) ou MQTT quando o broker estiver disponível. |
| Métricas | Potência ativa (W), energia acumulada (kWh), temperatura do dispositivo, estado do relé, tensão. |
| Casos de uso | Monitorar se geladeira/Freezer parou de consumir energia; alertar se ar-condicionado ligou fora do horário. |

#### 2.1.5 Zigbee / Z-Wave via gateway local

**Referências:** Zigbee2MQTT, ZHA (Home Assistant), Z-Wave JS.

| Aspecto | Proposta |
| :--- | :--- |
| O quê | Integrar com gateways Zigbee/Z-Wave locais (Sonoff ZBBridge, ConBee, Aeotec Z-Stick) através do MQTT. |
| Como | Assinar tópicos MQTT do Zigbee2MQTT ou Z-Wave JS UI e mapear `availability`, `linkquality` e `battery`. |
| Métricas | Qualidade do link (LQI), bateria, última vez visto, estado do sensor. |
| Alertas | "Sensor de fumaça da cozinha está offline", "Fechadura da porta da frente com bateria crítica". |

---

### 2.2 Monitoramento de ISP e qualidade de experiência (QoE)

#### 2.2.1 Testes de velocidade agendados e históricos

**Referências:** Speedtest by Ookla, Fast.com, LibreSpeed.

| Aspecto | Proposta |
| :--- | :--- |
| O quê | Rodar testes de download/upload/latência/jitter periodicamente e manter histórico. |
| Como | Integrar com librespeed/speedtest-cli ou implementar medição via iperf3 para servidores próprios. |
| Alertas | "Download caiu abaixo de 50% do contrato", "Latência para o gateway do ISP está acima de 100 ms há 10 min". |
| Valor | Provar para o ISP que a conexão está abaixo do contrato. |

#### 2.2.2 Monitoramento de latência para serviços SaaS 🟢 Concluído

**Referências:** ThousandEyes, Pingdom, UptimeRobot.

| Aspecto | Proposta |
| :--- | :--- |
| O quê | Targets pré-cadastrados (Google, Cloudflare, Netflix, Microsoft 365, GitHub, AWS, Zoom, WhatsApp) com thresholds sugeridos. |
| Como | ICMP + HTTP HEAD para endpoints estáveis. |
| Painel | Mapa de calor por hora do dia mostrando quando a rede fica lenta. |
| Estado | 🟢 **Concluído:** Catálogo curado de presets SaaS com endpoints ICMP e HTTP HEAD e thresholds automáticos, provisionamento 1-clique ou em lote (`SaasPresetsDialog`), agregação horária no backend via `monitor_results_hourly` (`GET /api/monitors/hourly-heatmap`) e novo widget interativo de Heatmap de Latência no Dashboard (`SaasLatencyHeatmapWidget`) e no detalhe dos monitores. |

#### 2.2.3 Detecção de shaping/tráfego priorizado

**Referências:** Glasnost ( projeto antigo do M-Lab), Wehe.

| Aspecto | Proposta |
| :--- | :--- |
| O quê | Comparar latência e vazão entre diferentes portas/protocolos (ex.: 443 vs 6881, UDP vs TCP). |
| Valor | Detectar neutralidade de rede violada ou priorização de tráfego. |

---

### 2.3 Observabilidade avançada e análise de causa raiz

#### 2.3.1 Coleta de métricas via Telegraf / Prometheus remote write

**Referências:** Prometheus, Grafana, InfluxDB, Telegraf.

| Aspecto | Proposta |
| :--- | :--- |
| O quê | Tornar o NetMonitor um destino de métricas: receber `remote_write` do Prometheus ou linhas InfluxDB Line Protocol. |
| Por quê | Dispositivos como routers MikroTik, OPNsense, Unifi e servidores já exportam métricas; o NetMonitor poderia consolidá-las sem agente próprio. |
| Armazenamento | SQLite/Postgres com retenção configurável + agregações diárias. |

#### 2.3.2 Correlação de eventos e causa raiz automática `🟢 Concluído`

**Referências:** BigPanda, Moogsoft, PagerDuty AIOps, Datadog Watchdog.

| Aspecto | Proposta |
| :--- | :--- |
| O quê | Quando vários hosts caem simultaneamente, inferir automaticamente se a causa é o roteador, o switch, o link ISP, o gateway, a VPN ou falha de site. |
| Como | Grafo de dependências construído a partir da topologia (`devices.parent_id`, `device_links` de LLDP/CDP/manuais e inferência de sub-redes) + scoring topológico e temporal com BFS. |
| Saída | "17 dispositivos ficaram inacessíveis após `192.168.1.1` (Gateway Principal) parar de responder — causa provável: Gateway da Rede". |
| Entrega | ✅ Motor de Causa Raiz e Grafo de Dependências (`DependencyGraph`) em `backend/src/services/alerts/correlation.rs` com BFS downstream/upstream, cálculo de raio de impacto e caminho de dependência.<br>✅ Classificação em 8 categorias causais (`Gateway`, `Router`, `Switch`, `Firewall`, `VpnTunnel`, `IspLink`, `SiteOutage`, `IsolatedDevice`) com pontuação de confiança (0 a 100%).<br>✅ Síntese diagnóstica em linguagem natural e endpoint global `GET /api/alerts/root-cause-analysis` com agrupamento de clusters ativos (`IncidentCluster`).<br>✅ Endpoint pontual `GET /api/alerts/:id/correlation` com cadeia de nós (`dependencyChain`) e lista de equipamentos impactados (`impactedDevices`).<br>✅ Frontend com Banner Inteligente de RCA na Central de Alertas (`AlertsPage.vue`) e modal expandido `AlertCorrelationDialog.vue` com chips de confiança, citação diagnóstica formatada, visualização de cadeia de dependência e raio de impacto. |

#### 2.3.3 Anomalias com linha de base estatística `🟢 Concluído`

**Referências:** Datadog anomaly detection, AWS CloudWatch Anomaly Detection.

| Aspecto | Proposta |
| :--- | :--- |
| O quê | Detectar desvios em métricas sem thresholds fixos (ex.: latência, perda de pacotes, volume de syslog, tráfego de interfaces). |
| Como | Modelo de média móvel histórica de 7 dias ($\mu$) + desvio padrão amostral ($\sigma$) com pisos numéricos de variância ($\epsilon$), cálculo de Z-Score ($z = \frac{\text{current} - \mu}{\max(\sigma, \epsilon)}$) e bandas de confiança de 3 sigmas ($\mu \pm 3\sigma$). |
| Entrega | ✅ Motor estatístico em `baseline.rs` com cálculo amostral de $\sigma$, Z-Scores e bandas normais ($3\sigma$).<br>✅ 13 novos campos tipados de alerta (`latencyZScore`, `latencyUpperBandMs`, `packetLossZScore`, `uptimeZScore`, `trafficInZScore`, etc.).<br>✅ Templates no catálogo (`latency_statistical_anomaly`, `packet_loss_statistical_anomaly`, `traffic_statistical_anomaly`).<br>✅ Endpoint `GET /api/monitors/:id/baseline` e componente visual `MonitorBaselineCard.vue` com badges de estado, faixas de confiança e criação de regras. |

---

### 2.4 Automação, ações e orquestração

#### 2.4.1 Ações corretivas automáticas (self-healing)

**Referências:** Rundeck, Ansible, Datadog Workflows, Home Assistant automations.

| Aspecto | Proposta |
| :--- | :--- |
| O quê | Permitir que um alerta dispare uma ação: ligar/desligar tomada inteligente, reiniciar POE de switch, chamar webhook, executar script remoto via SSH. |
| Como | Motor de automação simples: `SE alerta X E condição Y ENTÃO ação Z`, com cooldown e confirmação para ações destrutivas. |
| Exemplo | "Se o gateway não responder por 2 min, desligar a tomada do roteador por 10 s e ligar novamente". |

#### 2.4.2 Integração com Home Assistant

**Referências:** Home Assistant, Node-RED.

| Aspecto | Proposta |
| :--- | :--- |
| O quê | Expor dispositivos e alertas do NetMonitor no Home Assistant e vice-versa. |
| Como | Webhook no Home Assistant que recebe eventos do NetMonitor; MQTT para publicar estado dos dispositivos. |
| Valor | O usuário pode criar automações no HA usando a saúde da rede como gatilho. |

#### 2.4.3 Playbooks de diagnóstico

**Referências:** Datadog Notebooks, Grafana Incident, PagerDuty Runbooks.

| Aspecto | Proposta |
| :--- | :--- |
| O quê | Para cada tipo de alerta, apresentar um checklist automatizado de verificação. |
| Exemplo | Alerta "internet lenta" → executar traceroute, teste de velocidade, verificar uso de banda por dispositivo. |

---

### 2.5 Segurança e conformidade

#### 2.5.1 Detecção de dispositivos desconhecidos e mudanças de topologia

**Referências:** Fing, GlassWire, UniFi Network.

| Aspecto | Proposta |
| :--- | :--- |
| O quê | Alertar quando um novo MAC aparece na rede ou quando um dispositivo muda de porta/VLAN. |
| Como | Comparar snapshots de ARP/descoberta. |
| Extensão | Lista de permitidos/bloqueados e integração com notificações. |

#### 2.5.2 Scan de vulnerabilidades leve

**Referências:** OpenVAS, Nessus Essentials, Shodan.

| Aspecto | Proposta |
| :--- | :--- |
| O quê | Verificar portas abertas, serviços com versões conhecidas e credenciais padrão em dispositivos locais. |
| Como | Integrar com o scanner de portas existente e banco de CVEs leve (ex.: consulta a serviços identificados). |
| Cuidado | Manter opcional e educativo — não substitui pentest. |

#### 2.5.3 Backup e versionamento de configurações de rede

**Referências:** RANCID, Oxidized, Git.

| Aspecto | Proposta |
| :--- | :--- |
| O quê | Coletar periodicamente configurações de switches/roteadores via SSH/Telnet e detectar mudanças. |
| Como | Tarefa agendada que roda `show running-config` e guarda no banco com diff. |

---

### 2.6 Experiência do usuário e mobilidade

#### 2.6.1 Aplicativo mobile nativo ou PWA aprimorado 🟢 Concluído

**Referências:** UniFi Network app, Fing app, UptimeRobot app.

| Aspecto | Proposta |
| :--- | :--- |
| O quê | Notificações push, visualização rápida de status, ações de silêncio de alerta. |
| Como | PWA com push via service worker + Web Push, ou app Flutter/React Native para recursos nativos. |
| Estado | 🟢 **Concluído:** PWA completo com Service Worker customizado (`injectManifest`), Web Push (RFC 8030/8291/8292/VAPID), atalhos rápidos e botão de instalação no menu lateral com suporte a iOS e Android. |

#### 2.6.2 Mapa de calor de Wi-Fi

**Referências:** UniFi Design Center, NetSpot, Ekahau.

| Aspecto | Proposta |
| :--- | :--- |
| O quê | Permitir upload de planta baixa e anotação de RSSI medido em diferentes pontos. |
| Valor | Identificar zonas mortas e decidir onde colocar repetidores. |

#### 2.6.3 Assistente de configuração guiada (onboarding)

**Referências:** Google Nest setup, Eero TrueMesh, Ubiquiti UniFi.

| Aspecto | Proposta |
| :--- | :--- |
| O quê | Wizard que descobre a rede, sugere monitores para gateway/DNS, configura alertas padrão e integra notificações. |
| Valor | Reduzir o tempo do primeiro valor (time-to-value). |

---

### 2.7 Modelo de negócio e multi-tenancy

#### 2.7.1 Suporte a múltiplos sites/clientes

**Referências:** Datadog, PRTG, Auvik.

| Aspecto | Proposta |
| :--- | :--- |
| O quê | Um mesmo servidor NetMonitor gerenciar vários sites (casa, escritório, cliente A, cliente B). |
| Como | Entidade `Site` + probes vinculados a sites; dashboard com alternância de contexto. |
| Extensão | Papéis e permissões por site (multi-tenancy leve). |

#### 2.7.2 Marketplace de plugins de integração

**Referências:** Home Assistant HACS, Grafana plugins, Nagios Exchange.

| Aspecto | Proposta |
| :--- | :--- |
| O quê | Permitir que a comunidade escreva plugins para novos dispositivos/protocolos. |
| Como | API estável de "device adapter" em WebAssembly ou script com interface REST/MQTT definida. |

---

## 3. Matriz de priorização sugerida

A priorização abaixo considera **impacto para o usuário final** × **esforço técnico** × **aderência à arquitetura atual** (container único, SQLite/Postgres, sem broker externo).

| Rank | Oportunidade | Impacto | Esforço | Justificativa |
| :--- | :--- | :--- | :--- | :--- |
| 1 | Integração com dispositivos IoT (Tuya, Sonoff, ESPHome, Shelly) | 🟢 Alto | 🟡 Médio | Diferencial competitivo imediato; aproveita a descoberta e o motor de alertas existentes. |
| 2 | Testes de velocidade agendados + histórico | 🟢 Alto | 🟢 Pequeno | Demanda recorrente; pode reaproveitar o scheduler. |
| 3 | Mapa de dependências e causa raiz automática | 🟢 Alto | 🔴 Alto | Reduz drasticamente o tempo de diagnóstico; requer modelagem de grafo. |
| 4 | Ações corretivas automáticas (self-healing) | 🟢 Alto | 🟡 Médio | Natural depois de IoT; precisa de motor de automação e auditoria. |
| 5 | Detecção de dispositivos desconhecidos | 🟡 Médio | 🟢 Pequeno | Reforça a segurança; usa dados já coletados. |
| 6 | Integração Home Assistant / MQTT | 🟢 Alto | 🟡 Médio | Amplia o ecossistema sem reescrever o produto. |
| 7 | Recepção de métricas (Prometheus remote_write / Influx Line Protocol) | 🟡 Médio | 🟡 Médio | Torna o NetMonitor um hub de observabilidade. |
| 8 | PWA com notificações push | 🟡 Médio | 🟡 Médio | Melhora a experiência mobile sem custo de app nativo. |
| 9 | Scan de vulnerabilidades leve | 🟡 Médio | 🟡 Médio | Valor de segurança; exige cuidado para não alarmar falsamente. |
| 10 | Multi-tenancy por sites | 🟡 Médio | 🔴 Alto | Habilita MSPs (provedores de serviço gerenciado); muda o modelo de dados. |
| 11 | Backup de configurações de equipamentos | 🟡 Médio | 🟡 Médio | Útil para redes profissionais; requer credenciais SSH. |
| 12 | Mapa de calor de Wi-Fi | 🔵 Baixo | 🟡 Médio | Feature visual; prioridade menor para o core. |
| 13 | Plugin marketplace | 🟢 Alto | 🔴 Alto | Escala o ecossistema, mas exige API estável e comunidade. |

---

## 4. Recomendação de roteiro curto/médio prazo

### Fase A — Ganho rápido (próximas 4–6 semanas)

1. **Testes de velocidade agendados** — adicionar monitor do tipo `speedtest` (download/upload/latência/jitter) com histórico.
2. **Detecção de dispositivos desconhecidos** — snapshot diário de MACs e alerta de novidade.
3. **Métricas de ISP pré-cadastradas** — targets padrão (gateway, DNS, serviços populares) no primeiro acesso.

### Fase B — Expansão para IoT (próximas 8–12 semanas)

1. **Descoberta mDNS aprimorada** — identificar fabricantes via hostname (`tasmota-`, `shelly-`, `esphome-`).
2. **Adapter de dispositivos IoT** — abstração genérica (`IotAdapter`) com implementações para:
   - Tuya Cloud API (opcional).
   - Sonoff DIY / eWeLink (opcional).
   - ESPHome API local.
   - Shelly RPC local.
3. **Novos tipos de monitor IoT** — `iot_state`, `iot_sensor`, `iot_power`, `iot_battery`.
4. **Ações de alerta em dispositivos IoT** — ligar/desligar tomada inteligente como ação corretiva.

### Fase C — Inteligência e automação (próximas 12–24 semanas)

1. **Grafo de dependências** — modelar relações entre dispositivos, switch, gateway e ISP.
2. **Causa raiz automática** — correlacionar falhas em cascata.
3. **Motor de automação** — regras `SE...ENTÃO...` com auditoria.
4. **Integração Home Assistant** — webhook/MQTT bidirecional.

---

## 5. Riscos e cuidados

| Risco | Mitigação |
| :--- | :--- |
| Fragmentação de protocolos IoT | Criar abstração `IotAdapter` para isolar protocolos específicos. |
| Dependência de cloud (Tuya/eWeLink) | Sempre preferir API local; cloud como fallback opcional. |
| Segurança de credenciais de terceiros | Usar o cofre existente; nunca logar tokens. |
| Falsos positivos em automações | Exigir confirmação ou modo "dry-run" para ações destrutivas. |
| Performance do SQLite com muitas métricas | Retenção agressiva, agregações diárias e recomendação de Postgres para instalações grandes. |
| Privacidade de dados de rede | Manter tudo local por padrão; cloud apenas quando o usuário optar. |

---

## 6. Métricas de sucesso

Para avaliar se uma evolução deu certo:

- **Adoção:** % de instalações que ativaram ao menos uma integração IoT.
- **Tempo médio de resolução (MTTR):** redução após causa raiz automática e automações.
- **Falsos positivos:** taxa de alertas silenciados ou descartados.
- **Engajamento:** frequência de acesso ao app/painel.
- **Churn inverso:** usuários que voltam a usar após feature nova.

---

## 7. Próximos passos imediatos

1. Validar com usuários reais quais integrações IoT são mais desejadas (Tuya, Sonoff, ESPHome, Shelly, outras).
2. Prototipar um `IotAdapter` para um único protocolo (sugestão: **Shelly** ou **ESPHome**, por serem totalmente locais).
3. Adicionar o tipo de monitor `speedtest` como prova de conceito de QoE.
4. Atualizar este documento após cada experimento, descartando ou elevando ideias conforme feedback.
