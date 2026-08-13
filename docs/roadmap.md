# Roadmap de Desenvolvimento: Monitor de Rede

Baseado na documentação de arquitetura (`docs/arquitetura.md`) e especificação base (`docs/base.md`), este documento mapeia o status atual do projeto, detalhando o que já foi estruturado e as etapas necessárias para tornar o sistema 100% funcional.

> **O backend é Rust (Loco.rs), em `backend-rust/`.** As fases abaixo registram
> o que foi entregue — os nomes de arquivo e comando citados nelas são os da
> época em que cada fase foi escrita. Para o estado atual, leia
> [arquitetura.md](arquitetura.md); para o registro da reescrita,
> [historico/](historico/).

---

## 📊 Visão Geral do Status Atual

| Componente / Módulo | Status Atual | Descrição |
| :--- | :---: | :--- |
| **Documentação Técnica** | 🟢 **Concluído** | Especificação completa da arquitetura (`arquitetura.md`) e requisitos (`base.md`). |
| **Backend em Rust (Loco.rs)** | 🟢 **Concluído** | Reescrita completa em `backend-rust/`: 22 migrations, ~90 endpoints, 5 checkers, discovery, SNMP, topologia, alertas, probes e VPN. Ver [historico/roadmap_backend_rust.md](historico/roadmap_backend_rust.md). |
| **Estrutura do Projeto Backend** | 🟢 **Concluído** | Controllers em `src/controllers/`, domínio em `src/services/`, entidades `sea-orm` em `src/models/_entities/`. |
| **Banco de Dados & Migrations** | 🟢 **Concluído** | 21 tabelas, uma migration por tabela em `backend-rust/migration/`, entidades `sea-orm` com os relacionamentos. |
| **Backup & Restauração** | 🟢 **Concluído** | `GET /api/backup/export` baixa as 12 tabelas de configuração em JSON; `POST /api/backup/preview` mostra o que o arquivo contém e `POST /api/backup/restore` o aplica em uma transação, preservando os ids. Telemetria e `users` ficam de fora. Card na tela `/settings`. |
| **Motor de Monitoramento (Checkers)** | 🟢 **Concluído** | Checkers reais de Ping (ICMP/RTT), HTTP/HTTPS (Fetch/Status/Latência), TCP (Sockets), SNMP (uptime/CPU/memória/interface) e DNS com medição de latência de resolução via UDP, TCP e DoH. |
| **Processamento de Resultados** | 🟢 **Concluído** | `ResultProcessor` grava resultados no banco, extrai métricas e atualiza estado dos dispositivos/monitores. |
| **Worker & Queue System** | 🔴 **Não implementado** | A fila do §4.2 da arquitetura nunca saiu do papel: o `scheduler` executa os monitores inline e os probes cuidam do resto. Ver a dívida de backpressure na Fase 2. |
| **Agendador (Scheduler)** | 🟢 **Concluído** | Processo `scheduler` (`task scheduler_loop`) repete o ciclo a cada 5 s: busca por `next_run_at`, execução e recálculo do próximo ciclo ([ADR 007](adr/007-scheduler-processo-unico.md)). |
| **Descoberta Automática (Discovery)** | 🟢 **Concluído** | Scanners funcionais (ICMP/Ping sweep, tabela ARP, PortScanner) e fusão com auto-criação de dispositivo/monitor. |
| **Comunicação com Probes** | 🟢 **Concluído** | Autenticação por token Hash (SHA-256), registro via CLI (`task probe_register`), heartbeat, despachante de tarefas e buffer offline com reenvio automático. |
| **SNMP & Métricas de Tráfego** | 🟢 **Concluído** | Coleta SNMP (v1/v2c/v3), varredura de interfaces, contadores de octetos (ifHCIn/ifHCOut) e métricas de tráfego (bps). |
| **Topologia de Rede** | 🟢 **Concluído** | Resolutor de links (`DeviceLink`), pontuação de confiança (`ConfidenceCalculator`), inferência de sub-rede e gerador do mapa gráfico. |
| **Alertas & Notificações** | 🟢 **Concluído** | Avaliação de regras em tempo real (`AlertManager`), catálogo de regras pré-configuradas com aplicação idempotente (`AlertRuleCatalogService`), ciclo de vida (ativo, reconhecido, silenciado, resolvido) e conectores (E-mail, Telegram, Discord, Webhook). |
| **Eventos Tempo Real (SSE)** | 🟢 **Concluído** | Barramento `EventBus` singleton e streaming em `/api/events/stream` via SSE funcional. |
| **Frontend (Vue 3 + Vuetify)** | 🟢 **Concluído** | SPA/PWA completa integrada à API REST, com Pinia, gráficos, topologia gráfica interativa e suporte a SSE em tempo real. |
| **Infraestrutura Docker** | 🟢 **Concluído** | **Um container**: API, SPA, ciclo de monitores, SQLite em WAL e WireGuard. Oito serviços viraram um — nginx, Postgres, migration, scheduler, probe e vpn-probe saíram ([arquitetura.md §3](arquitetura.md#3-topologia-de-processos)). |
| **Módulo WireGuard (VPN)** | 🟢 **Concluído (Fases 1–4)** | Modelo de dados, geração nativa de chaves X25519, IPAM transacional, scripts por perfil (MikroTik/OpenWrt/Linux/Windows/Mobile), container WireGuard com hot-reload por `syncconf`, `vpn-probe` dedicado e telas de servidor/dispositivos/wizard. Falta apenas a validação E2E com hardware real ([roadmap_vpn.md](roadmap_vpn.md)). |
| **Dashboard Grafana Customizável** | 🟢 **Concluído** | Dashboard modular com reordenação de widgets, adicionar/remover cards, limitação de altura e scroll no card de monitores e novos gráficos temporais ([roadmap_dashboard_grafana.md](roadmap_dashboard_grafana.md)). |

---

## 🎯 Roadmap Detalhado por Fases

---

### Fase 1: MVP Backend & Persistência de Dados (Concluído 🟢)
> **Objetivo:** Ter a API REST 100% funcional com banco PostgreSQL populado e CRUDs reais.

- [x] **Estrutura base do backend** (controllers, rotas, serviços de domínio).
- [x] **Migrations do esquema (Crítico)**:
  - [x] `sites` (locais monitorados).
  - [x] `networks` (sub-redes e faixas CIDR).
  - [x] `probes` (agentes locais/remotos).
  - [x] `devices`, `device_interfaces`, `device_links`.
  - [x] `monitors` e `monitor_results`.
  - [x] `metrics` e `discovery_runs` / `discovery_results`.
  - [x] `alert_rules` e `alert_events`.
- [x] **Relacionamentos & entidades**:
  - [x] Tipagem de colunas e relacionamentos em todas as entidades `sea-orm`.

---

### Fase 2: Motor de Monitoramento & Workers (Concluído 🟢)
> **Objetivo:** Executar verificações de disponibilidade reais (Ping, HTTP, TCP, DNS) através de Workers e Scheduler.

- [x] **Implementação dos Checkers Funcionais**:
  - [x] `PingChecker`: integração com ICMP nativo / pacotes raw ping com cálculo real de RTT e packet loss.
  - [x] `HttpChecker`: requisições HTTP/HTTPS com validação de status code, tempo de resposta e expiração SSL.
  - [x] `TcpChecker`: conexão socket TCP com medição de timeout e abertura de porta.
  - [x] `DnsChecker`: resolução de registros DNS (A, AAAA, CNAME, MX, TXT, NS) com medição de latência.
  - [x] **Latência de resolução DNS**: consultas em wire format próprio (RFC 1035) sobre UDP, TCP e DoH (RFC 8484), cronometradas com `performance.now()`; medição de múltiplos hostnames por checagem, limiar de latência configurável e ranking dos servidores mais rápidos no dashboard (`/api/dns/performance`, `/api/dns/benchmark`, `/api/dns/lookup`).
  - [x] **Cadastro de servidores DNS** (`/api/dns/servers`): CRUD com semeadura dos resolvedores públicos no primeiro acesso, autocomplete no formulário de monitores e seleção de quais participam da comparação do dashboard.
- [x] **Vínculo com dispositivo opcional**: checagens externas (DNS público, sites de terceiros) deixam de exigir um equipamento cadastrado; o SNMP segue exigindo. A origem da execução (servidor da aplicação ou probe) passou a ser escolhida no próprio formulário.
- [x] **Processamento de resultados**:
  - [x] `ResultProcessor` para processar resultados, calcular métricas e atualizar status de `Device` e `Monitor`.
  - [x] Execução e validação de um monitor por ID via `POST /api/monitors/{id}/run`.
- [x] **Agendador (tarefa `scheduler_run`)**:
  - [x] Consulta otimizada por `next_run_at <= NOW()` com lock para evitar duplicação.
  - [x] Recálculo automático de `next_run_at`.
- [ ] **Sistema de filas & worker process** — 🔴 **não implementado**. Nunca saiu
  do papel, e as dependências que existiam para ele (Redis e biblioteca de fila)
  foram removidas por nunca terem sido importadas. Hoje o `scheduler` executa os
  monitores inline e a fila dos probes é a tabela `probe_tasks`. Ver
  [§12 da arquitetura](arquitetura.md#12-o-que-não-existe).

  > **Dívida — backpressure no scheduler.** O ciclo
  > (`backend-rust/src/tasks/scheduler_run.rs`) busca os monitores vencidos a
  > cada tique de 5 s e dispara as execuções sem controle de concorrência: se
  > elas demorarem mais que o tique, o ciclo seguinte abre outra leva sem saber
  > que as anteriores ainda rodam. Com o volume atual isso não aparece — é um
  > teto que chega sem aviso. **Gatilho para reviver a fila:** execuções
  > acumulando além de um tique, ou o total de monitores vencidos encostando no
  > limite por ciclo de forma recorrente.

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
  - [x] `POST /api/discovery/scan-stream` transmite progresso e hosts encontrados em tempo real via NDJSON.
  - [x] `discovery_results` funciona apenas como cache do último scan; "já adicionado" é determinado verificando se o IP existe em `devices`.
  - [x] Controller e rotas para listar execuções e criar dispositivo a partir do resultado (sem botão ignorar/mesclar).

---

### Fase 4: Probes & Arquitetura Distribuída (Concluído 🟢)
> **Objetivo:** Permitir que agentes leves rodem em outras redes ou no próprio container e enviem dados ao servidor central.

- [x] **Autenticação & Registro do Probe**:
  - [x] Fluxo de registro com token temporário (`backend_rust-cli task probe_register`).
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
  - [x] Avaliação genérica por contexto (`AlertEvaluationContext`): qualquer produtor de fatos — resultado de monitor ou coleta SNMP de interfaces — é avaliado pelo mesmo motor, com deduplicação e normalização por `scopeKey`.
- [x] **Catálogo de Regras Pré-configuradas (`AlertRuleCatalogService`)**:
  - [x] Templates versionados em código (`alert_rule_templates.ts`) cobrindo disponibilidade, desempenho, serviços, interfaces e equipamento.
  - [x] Provisionamento automático do conjunto básico em instalações novas (`start/alert_rules.ts`), sem ressuscitar regras removidas de propósito.
  - [x] Aplicação idempotente via `GET/POST /api/alert-rules/catalog` e diálogo de seleção na Central de Alertas — regra já existente (por template ou por condição equivalente) nunca é duplicada.
  - [x] Políticas antes fixas no código (queda de interface e downgrade de negociação Ethernet) migradas para o catálogo, ajustáveis pelo operador.
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
  - [x] **Cadastro do primeiro acesso** (`/setup`): em banco sem usuário o roteador leva à tela de cadastro (nome, e-mail, senha e token de instalação) em vez do login, e a sessão já vem pronta na resposta. Tela de login com validação por campo, mensagem de erro do backend e retorno ao destino original (`?redirect`).
  - [x] Stores de `sites`, `networks`, `devices`, `deviceDetail`, `monitors`, `discovery`, `topology`, `alerts`, `probes`, `events`.
- [x] **Telas e Componentes Interativos**:
  - [x] **Dashboard**: cards de resumo em tempo real, lista de monitores com barras verticais coloridas (estilo Uptime Kuma) e feed SSE de eventos simplificado e amigável.
  - [x] **Gestão de Dispositivos**: listagem com filtros, status visual (online/offline), site opcional, modal reusável de site (`SiteDialog.vue`), campo "Está atrás de" (hierarquia/topologia) e opção de monitorar dispositivo (`isMonitored`).
  - [x] **Detalhes do Dispositivo**: abas com histórico de latência, interfaces SNMP, monitores vinculados e eventos.
  - [x] **Central de Descoberta**: listagem do último scan com progresso em tempo real e botão para adicionar dispositivo; cache do último scan é limpo a cada nova varredura.
  - [x] **Mapa de Topologia**: tela gráfica interativa para arrastar nós, visualizar ligações e definir links manuais.
- [x] **Layout Mobile/Desktop otimizado**:
  - [x] Tabelas usam `ResponsiveDataTable` com visual de cards no mobile, sem bordas arredondadas e com margens reduzidas (~5px) para aparência de app portable.
- [x] **Monitores com Histórico Estilo Uptime Kuma & Gráficos de Latência**:
  - [x] Componente `MonitorTimelineBar.vue` com barras verticais coloridas (estilo Uptime Kuma) na lista de monitores (`/monitors`).
  - [x] Página de detalhes/gráficos de monitor (`/monitors/:id`) com estatísticas de ping (médio, mín, máx), gráfico SVG de latência e log de verificações.
- [x] **Suporte a PWA**:
  - [x] Manifest file, ícones de aplicativo e Service Worker para experiência instalável.
- [x] **Backup e Restauração das Configurações** (`/settings`):
  - [x] Exportação em JSON das 12 tabelas de configuração (sites, redes, dispositivos, interfaces, enlaces, monitores, regras de alerta, servidores DNS, servidor VPN e peers, preferências); telemetria e contas de acesso ficam de fora.
  - [x] Prévia do arquivo antes de aplicar (contagem por tabela) e restauração transacional com os ids preservados — o que mantém intactas as FKs de monitores, regras e enlaces.

---

### Fase 8: Módulo WireGuard para Roteadores Remotos (Fases 1–4 Concluídas 🟢)
> **Objetivo:** Monitorar roteadores MikroTik (RouterOS v7+) e OpenWrt fora da rede local via túnel WireGuard, com gestão de chaves e configuração 100% pela interface do sistema.

Especificação completa em [roadmap_vpn.md](roadmap_vpn.md).

- [x] **Modelo de Dados**: tabelas `vpn_servers` e `vpn_peers`, índice UNIQUE `(network_id, ip_address)` em `devices` para o IPAM, com chaves privadas e PSKs cifradas via `ENCRYPTION_KEY`.
- [x] **Core Backend**: geração de chaves X25519 em Rust puro, via `x25519-dalek` (sem binário `wg`), alocador de IP transacional com retry, geradores de configuração por perfil (MikroTik, OpenWrt, Linux, Windows, Mobile), parser de `wg show dump` e API `/api/vpn/...`.
- [x] **Provisionamento automático**: ao concluir o wizard, o sistema cria `Device`, `VpnPeer`, monitor de Ping e (opcional) monitor SNMP em uma única transação. Com o túnel no mesmo container, os monitores rodam locais; com `VPN_PROBE_EXTERNAL=true`, ficam com o `vpn-probe`.
- [x] **Docker**: hot-reload via `wg syncconf` (sem `docker.sock`) por um watcher root dentro do container único. O processo da API segue sem capability alguma (`CapEff` zerado) — a fronteira do §7 do AGENTS.md mudou de container para processo, não de conteúdo.
- [x] **Frontend**: painel do servidor VPN com teste de pré-voo (detecção de CGNAT), lista de peers com status de handshake e diagnóstico de firewall, e wizard de adição por perfil de equipamento com "Copiar tudo".
- [ ] **Validação E2E**: conexão real de MikroTik e OpenWrt com ICMP e SNMP pelo túnel (exige host Linux com IP público ou port-forward UDP).

---

## 📌 Próximos Passos Recomendados (Ordem de Prioridade)

1. **Provisionar ambiente de validação com IP público** (host Linux ou VPS) e executar a Fase 5 do `docs/roadmap_vpn.md` com um MikroTik e um OpenWrt reais.
2. ~~**Aplicar o middleware `auth` ao grupo `/api`**~~ — **resolvido 🟢**: o grupo de negócio passa por `controllers::auth_guard::require_jwt` (`src/app.rs`), e a autenticação deixou de ser stub. O primeiro usuário nasce em `POST /api/auth/setup`, autorizado pelo token de instalação (`SETUP_TOKEN` ou sorteado no boot); `POST /api/auth/register` passou a exigir sessão, para o token de instalação não virar formalidade.
3. ~~Verificar o parsing de latência do `PingChecker` nos dois formatos de saída~~ — **resolvido**: o checker aceita os formatos do iputils (`rtt min/avg/max/mdev`) e do BusyBox (`round-trip min/avg/max`).
