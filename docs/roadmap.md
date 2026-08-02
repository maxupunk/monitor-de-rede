# Roadmap de Desenvolvimento: Monitor de Rede

Baseado na documentação de arquitetura (`docs/arquitetura.md`) e especificação base (`docs/base.md`), este documento mapeia o status atual do projeto, detalhando o que já foi estruturado e as etapas necessárias para tornar o sistema 100% funcional.

---

## 📊 Visão Geral do Status Atual

| Componente / Módulo | Status Atual | Descrição |
| :--- | :---: | :--- |
| **Documentação Técnica** | 🟢 **Concluído** | Especificação completa da arquitetura (`arquitetura.md`) e requisitos (`base.md`). |
| **Estrutura do Projeto Backend** | 🟢 **Concluído** | Estrutura de diretórios AdonisJS v6, rotas da API, controllers e modelos definidos. |
| **Banco de Dados & Migrations** | 🟢 **Concluído** | Criadas todas as 15+ tabelas de negócio e atualizados os modelos Lucid com relacionamentos. |
| **Motor de Monitoramento (Checkers)** | 🟢 **Concluído** | Checkers reais de Ping (ICMP/RTT), HTTP/HTTPS (Fetch/Status/Latência), TCP (Sockets) e DNS (`node:dns`). |
| **Worker & Queue System** | 🟢 **Concluído** | `ResultProcessor` grava resultados no banco, extrai métricas e atualiza estado dos dispositivos/monitores. |
| **Agendador (Scheduler)** | 🟢 **Concluído** | Comando `scheduler:run` com loop de busca por `next_run_at`, execução e recálculo do próximo ciclo. |
| **Descoberta Automática (Discovery)** | 🟢 **Concluído** | Scanners funcionais (ICMP/Ping sweep, tabela ARP, PortScanner) e fusão com auto-criação de dispositivo/monitor. |
| **Comunicação com Probes** | 🟢 **Concluído** | Autenticação por token Hash (SHA-256), registro via CLI (`probe:register`), heartbeat, despachante de tarefas e buffer offline com reenvio automático. |
| **SNMP & Métricas de Tráfego** | 🟢 **Concluído** | Coleta SNMP (v1/v2c/v3), varredura de interfaces, contadores de octetos (ifHCIn/ifHCOut) e métricas de tráfego (bps). |
| **Topologia de Rede** | 🟢 **Concluído** | Resolutor de links (`DeviceLink`), pontuação de confiança (`ConfidenceCalculator`), inferência de sub-rede e gerador do mapa gráfico. |
| **Alertas & Notificações** | 🟢 **Concluído** | Avaliação de regras em tempo real (`AlertManager`), ciclo de vida (ativo, reconhecido, silenciado, resolvido) e conectores (E-mail, Telegram, Discord, Webhook). |
| **Eventos Tempo Real (SSE)** | 🟢 **Concluído** | Barramento `EventBus` singleton e streaming em `/api/events/stream` via SSE funcional. |
| **Frontend (Vue 3 + Vuetify)** | 🟢 **Concluído** | SPA/PWA completa integrada à API REST AdonisJS v6, com Pinia, gráficos, topologia gráfica interativa e suporte a SSE em tempo real. |
| **Infraestrutura Docker** | 🟢 **Concluído** | `docker-compose.yml` e `Dockerfile` configurados para todos os serviços (API, Worker, Scheduler, Probe, Postgres, Redis, Frontend). |

---

## 🎯 Roadmap Detalhado por Fases

---

### Fase 1: MVP Backend & Persistência de Dados (Concluído 🟢)
> **Objetivo:** Ter a API REST 100% funcional com banco PostgreSQL populado e CRUDs reais.

- [x] **Estrutura base AdonisJS v6** (Controllers, Routes, Service Providers).
- [x] **Migrations Lucid ORM (Crítico)**:
  - [x] `sites` (locais monitorados).
  - [x] `networks` (sub-redes e faixas CIDR).
  - [x] `probes` (agentes locais/remotos).
  - [x] `devices`, `device_addresses`, `device_macs`, `device_interfaces`, `device_links`.
  - [x] `monitors` e `monitor_results`.
  - [x] `metrics` e `discovery_runs` / `discovery_results`.
  - [x] `alert_rules` e `alert_events`.
- [x] **Relacionamentos & Models Lucid**:
  - [x] Tipagem de colunas e relacionamentos (`hasMany`, `belongsTo`) em todos os Models.

---

### Fase 2: Motor de Monitoramento & Workers (Concluído 🟢)
> **Objetivo:** Executar verificações de disponibilidade reais (Ping, HTTP, TCP, DNS) através de Workers e Scheduler.

- [x] **Implementação dos Checkers Funcionais**:
  - [x] `PingChecker`: integração com ICMP nativo / pacotes raw ping com cálculo real de RTT e packet loss.
  - [x] `HttpChecker`: requisições HTTP/HTTPS com validação de status code, tempo de resposta e expiração SSL.
  - [x] `TcpChecker`: conexão socket TCP com medição de timeout e abertura de porta.
  - [x] `DnsChecker`: resolução de registros DNS (A, AAAA, CNAME, MX, TXT) via `node:dns`.
- [x] **Sistema de Filas & Worker Process**:
  - [x] `ResultProcessor` para processar resultados, calcular métricas e atualizar status de `Device` e `Monitor`.
  - [x] Comando `monitor:test` para execução e validação via CLI por ID de monitor.
- [x] **Agendador (`node ace scheduler:run`)**:
  - [x] Consulta otimizada por `next_run_at <= NOW()` com lock para evitar duplicação.
  - [x] Recálculo automático de `next_run_at`.

---

### Fase 3: Descoberta de Dispositivos (Discovery) (Concluído 🟢)
> **Objetivo:** Permitir escannear a rede local e descobrir dispositivos automaticamente.

- [x] **Implementação dos Scanners**:
  - [x] `IcmpScanner`: varredura de faixa IP via ping sweep e resolução de DNS reverso (PTR).
  - [x] `ArpScanner`: leitura da tabela ARP do sistema e extração de pares IP-MAC.
  - [x] `PortScanner`: detecção de portas abertas comuns (80, 443, 22, 445, 8080, 8291).
- [x] **Consolidação & Fusão de Resultados (`DiscoveryMerger`)**:
  - [x] Classificação aprimorada do tipo de equipamento (`DeviceIdentifier`) e cálculo de confiança.
  - [x] Fusão de registros por IP/MAC no `DiscoveryMerger`.
- [x] **Fluxo da API de Descoberta**:
  - [x] Comando CLI `network:scan` para varredura imediata.
  - [x] Controller e rotas para listar execuções, aceitar (gerando dispositivo e monitor de ping automaticamente) ou ignorar/mesclar resultados.

---

### Fase 4: Probes & Arquitetura Distribuída (Concluído 🟢)
> **Objetivo:** Permitir que agentes leves rodem em outras redes ou no próprio container e enviem dados ao servidor central.

- [x] **Autenticação & Registro do Probe**:
  - [x] Fluxo de registro com token temporário `node ace probe:register`.
  - [x] Autenticação persistente com Token Hash (SHA-256).
  - [x] Heartbeat periódico e status online/offline/revoked.
- [x] **Despachante de Tarefas & Buffer Offline**:
  - [x] Despacho de verificações atribuídas a um `probe_id` (`ProbeTaskDispatcher`).
  - [x] Armazenamento temporário de resultados offline no Probe (`ProbeBuffer`) em caso de perda de conexão.
  - [x] Reenvio automático assim que a conexão com o servidor central for restabelecida.

---

### Fase 5: Alertas, Notificações & Eventos SSE em Tempo Real (Concluído 🟢)
> **Objetivo:** Notificar o usuário em tempo real sobre indisponibilidades e mudanças de estado.

- [x] **Serviço de Alertas (`rule_evaluator.ts` & `AlertManager`)**:
  - [x] Regras para dispositivo offline, latência alta, falha em serviço HTTP/TCP.
  - [x] Controle de tempo mínimo, janela de silêncio (`SilenceManager`) e recuperação automática (`RecoveryManager`).
- [x] **Canais de Notificação**:
  - [x] Conector de E-mail (`EmailChannel`).
  - [x] Conector de Telegram Bot API (`TelegramChannel`).
  - [x] Conector de Discord Webhook (`DiscordChannel`).
  - [x] Conector de Webhooks Genéricos (`WebhookChannel`).
- [x] **Server-Sent Events (SSE)**:
  - [x] Barramento em tempo real `EventBus` e transmissão em `/api/events/stream` via SSE funcional.

---

### Fase 6: SNMP, Métricas & Topologia Avançada (Concluído 🟢)
> **Objetivo:** Obter dados detalhados de switches/roteadores e gerar mapas visuais de rede.

- [x] **Coleta SNMP (v1, v2c, v3)**:
  - [x] `system_collector`: sysName, sysDescr, sysUptime.
  - [x] `interface_collector`: listagem de portas, status admin/oper, velocidade.
  - [x] `traffic_collector`: leitura de contadores de octetos (ifHCInOctets / ifHCOutOctets) para cálculo de tráfego (bps).
  - [x] `lldp_collector`: vizinhos LLDP/CDP para descoberta de conexões entre equipamentos.
- [x] **Topologia de Rede**:
  - [x] Resolução de links (`DeviceLink`) manuais, LLDP/CDP e inferidos.
  - [x] API REST (`/api/topology`) com grafo interativo e comandos CLI (`snmp:poll`).

---

### Fase 7: Frontend Vue 3 + Vuetify Funcional (Concluído 🟢)
> **Objetivo:** Conectar a interface visual com a API e disponibilizar uma SPA/PWA completa.

- [x] **Integração HTTP & Stores Pinia**:
  - [x] Serviço HTTP centralizado (`apiService.ts`) e Store de Autenticação (`authStore`) com login/logout.
  - [x] Stores de `sites`, `networks`, `devices`, `deviceDetail`, `monitors`, `discovery`, `topology`, `alerts`, `probes`, `events`.
- [x] **Telas e Componentes Interativos**:
  - [x] **Dashboard**: cards de resumo alimentados por estatísticas em tempo real via SSE.
  - [x] **Gestão de Dispositivos**: listagem com filtros, status visual (online/offline) e modal de criação/edição.
  - [x] **Detalhes do Dispositivo**: abas com histórico de latência, interfaces SNMP, monitores vinculados e eventos.
  - [x] **Central de Descoberta**: tabela para revisar, aceitar ou ignorar novos dispositivos encontrados.
  - [x] **Mapa de Topologia**: tela gráfica interativa para arrastar nós, visualizar ligações e definir links manuais.
- [x] **Monitores com Histórico Estilo Uptime Kuma & Gráficos de Latência**:
  - [x] Componente `MonitorTimelineBar.vue` com barras verticais coloridas (estilo Uptime Kuma) na lista de monitores (`/monitors`).
  - [x] Página de detalhes/gráficos de monitor (`/monitors/:id`) com estatísticas de ping (médio, mín, máx), gráfico SVG de latência e log de verificações.
- [x] **Suporte a PWA**:
  - [x] Manifest file, ícones de aplicativo e Service Worker para experiência instalável.

---

## 📌 Próximos Passos Recomendados (Ordem de Prioridade)

1. **Criar e executar as Migrations das tabelas de negócio no PostgreSQL.**
2. **Implementar a lógica real dos Checkers de Monitoramento (Ping, HTTP, TCP, DNS).**
3. **Conectar os Workers e o Scheduler com a fila Redis para execução assíncrona dos checks.**
4. **Implementar as rotas da API REST conectadas aos dados do banco.**
5. **Conectar as telas do Frontend Vue 3 com a API REST usando Pinia.**
