# Diagnóstico e Catálogo de Débitos Técnicos

Este documento consolida a auditoria de arquitetura, código-fonte, padrões de design, duplicações, segurança e gaps de teste do **NetMonitor**. A última revisão completa foi em **2026-08-21**; a seção 7 (arquitetura modular) foi acrescentada em **2026-11-05**. O estado real do código prevalece sobre qualquer descrição deste arquivo.

---

## 📊 Sumário Executivo & Métricas da Auditoria (2026-08-20 — retrato histórico)

| Área Auditada | Escopo | Estado Geral | Débitos Críticos / Altos |
| :--- | :--- | :--- | :--- |
| **Segurança & Privacidade** | JWT, criptografia, VPN, probes, container | Bases sólidas, mas com falhas operacionais graves de deploy | 🔴 4 Críticos / 🟠 4 Altos |
| **DevOps & CI/CD** | Dockerfile, compose, entrypoint, GitHub Actions | Containerização madura, CI inexistente na prática | 🔴 2 Críticos / 🟠 1 Alto / 🟡 2 Médios |
| **Frontend (Vue 3 / TS)** | 30 componentes, 17 páginas, 24 stores, 10 utils | Refatorado, mas sem suíte de testes e com gaps de segurança | 🔴 1 Alto / 🟡 4 Médios / 🔵 3 Baixos |
| **Backend (Rust / Loco.rs)** | 25 controllers, 20+ services, 23 models, 8 tasks | Alta solidez; débitos históricos resolvidos, sobram pontos de qualidade | 🟡 6 Médios / 🔵 4 Baixos |
| **Banco de Dados & Persistência** | 33 migrations principais + 4 de logs | Dual SQLite/Postgres operante; busca textual abstraída | 🟢 1 Baixo |

> **Nota sobre a revisão anterior:** a tabela de status da versão anterior deste documento (linhas 15–31) estava inconsistente com o corpo do texto e com o código. A tabela abaixo reflete o estado real verificado no repositório em 2026-08-20.

| ID | Categoria | Item de Débito Técnico | Severidade | Esforço | Status |
| :--- | :--- | :--- | :---: | :---: | :---: |
| **FE-01** | Frontend | Componentes monolíticos (`DeviceDetailPage`, `MonitorDetailView`, `MonitorFormDialog`) | Alto | Médio | 🟢 Concluído |
| **FE-02** | Frontend | Duplicação de widgets CPU/RAM e subutilização de `BaseMetricChart` | Alto | Pequeno | 🟢 Concluído |
| **FE-03** | Frontend | Ausência de testes automatizados e script `test` no frontend | Alto | Médio | 🟢 Concluído |
| **FE-04** | Frontend | Inconsistência no padrão de stores (`useCrudResource`) | Médio | Pequeno | 🟢 Concluído |
| **FE-05** | Frontend | Bundle size e code-splitting no Vite | Médio | Pequeno | 🟢 Concluído |
| **FE-06** | Frontend | Formatação duplicada de taxa/bytes | Médio | Pequeno | 🟢 Concluído |
| **FE-07** | Frontend | Resíduos de scaffold inicial | Baixo | Mínimo | 🟢 Concluído |
| **BE-01** | Backend | Duplicação de algoritmo CIDR | Médio | Pequeno | 🟢 Concluído |
| **BE-02** | Backend | Nomenclatura histórica de testes (`phase*.rs`) | Médio | Pequeno | 🟢 Concluído |
| **BE-03** | Backend | Geração de configurações/scripts VPN por concatenação de strings | Médio | Médio | 🟢 Concluído |
| **BE-04** | Backend | Complexidade do `scheduler_run.rs` | Médio | Médio | 🟢 Concluído |
| **BE-05** | Backend | Inconsistência entre `serde_json::json!` e DTOs tipados | Médio | Médio | 🟢 Concluído |
| **BE-06** | Backend | Acoplamento MAC-Telnet no módulo Syslog | Baixo | Pequeno | 🟢 Concluído |
| **DB-01** | Banco | Abstração FTS5 vs `tsvector`/GIN | Médio | Médio | 🟢 Concluído |
| **DO-01** | DevOps | Supervisão de subprocessos no container | Médio | Médio | 🟢 Concluído |
| **SEC-01** | Segurança | Segredos de fallback públicos (`JWT_SECRET`, `ENCRYPTION_KEY`) | Crítico | Pequeno | 🟢 Concluído |
| **SEC-02** | Segurança | Chave privada WireGuard em claro no repositório (`backend/tmp/wireguard/wg0.conf`) | Crítico | Pequeno | 🟢 Concluído |
| **SEC-03** | Segurança | CI em diretório ignorado pelo GitHub Actions (`backend/.github/`) | Crítico | Pequeno | 🟢 Concluído |
| **SEC-04** | Segurança | Receptor de resultados de probe não vincula monitor ao probe autenticado | Alto | Médio | 🟢 Concluído |
| **SEC-05** | Segurança | Injeção de configuração `wg0.conf` via nome de peer não sanitizado | Alto | Pequeno | 🟢 Concluído |
| **SEC-06** | Segurança | `POST /api/probes` aceita `token_hash` arbitrário do cliente | Alto | Pequeno | 🟢 Concluído |
| **SEC-07** | Segurança | JWT armazenado em `localStorage` no frontend | Alto | Médio | 🟢 Concluído (mitigado via CSP) |
| **SEC-08** | Segurança | Headers de segurança ausentes no servidor estático | Baixo | Pequeno | 🟢 Concluído |
| **SEC-09** | Segurança | Magic link restrito a `@example.com`/`@gmail.com` (resíduo de scaffold) | Médio | Pequeno | 🟢 Concluído |
| **SEC-10** | Segurança | DTOs de credencial derivam `Debug` expondo senha | Médio | Pequeno | 🟢 Concluído |
| **QUA-01** | Qualidade | Novos componentes/páginas monolíticas cresceram (`DashboardPage`, `AlertsPage`, `SettingsPage`) | Médio | Médio | 🟢 Concluído |
| **QUA-02** | Qualidade | `reqwest::Client` recriado a cada checagem HTTP | Médio | Pequeno | 🟢 Concluído |
| **QUA-03** | Qualidade | `lookup_host` sem timeout em ping e SNMP | Médio | Pequeno | 🟢 Concluído |
| **QUA-04** | Qualidade | N+1 por interface na coleta SNMP | Médio | Médio | 🟢 Concluído |
| **QUA-05** | Qualidade | Coluna `monitors.timeout_seconds` escrita e nunca lida | Médio | Pequeno | 🟢 Concluído |
| **QUA-06** | Qualidade | Buffer offline do probe sem teto e com reescrita O(n²) | Médio | Médio | 🟢 Concluído |
| **DOC-01** | Documentação | Inconsistência entre tabela resumo e corpo deste documento | Médio | Pequeno | 🟢 Corrigido |
| **DB-02** | Banco | 64 migrations históricas num crate só | Baixo | Médio | 🗓️ Planejado (ADR 017) |
| **ARQ-01** | Arquitetura | Composição estática: rotas, rotinas, menu e rotas da SPA ligados para toda instalação | Alto | Grande | 🗓️ Planejado (ADR 017) |
| **ARQ-02** | Arquitetura | Dependências na direção errada entre domínios no backend (A1–A5) | Médio | Grande | 🗓️ Planejado (ADR 017) |
| **ARQ-03** | Arquitetura | Barramento SSE central no frontend e Dashboard acoplado ao Docker (A6–A7) | Médio | Médio | 🗓️ Planejado (ADR 017) |
| **ARQ-04** | Arquitetura | Catálogo de widgets duplicado entre backend e frontend | Médio | Pequeno | 🗓️ Planejado (ADR 017) |
| **ARQ-05** | Performance | Snapshot Docker pedido com qualquer aba aberta | Médio | Pequeno | 🗓️ Planejado (ADR 017) |
| **ARQ-06** | Arquitetura | Crate único de ~185 mil linhas; fronteira só por convenção | Médio | Grande | 🗓️ Planejado (ADR 017) |

---

## 1. Débitos Técnicos: Frontend (Vue 3 / TypeScript / Vuetify / Pinia)

### 🟢 FE-01: Componentes Monolíticos — Concluído
- **Arquivos Refatorados:**
  - `frontend/src/pages/DeviceDetailPage.vue` → `frontend/src/components/devices/tabs/`
  - `frontend/src/components/monitors/MonitorDetailView.vue` → `frontend/src/components/monitors/detail/`
  - `frontend/src/components/MonitorFormDialog.vue` → `frontend/src/components/monitors/form/`

### 🟢 FE-02: Duplicação de Widgets — Concluído
- Criado `ResourceUsageWidget.vue`; `CpuUsageWidget.vue` e `RamUsageWidget.vue` viraram wrappers.

### 🟢 FE-03: Ausência de Infraestrutura e Suíte de Testes no Frontend — Concluído
- **Arquivos Afetados:** `frontend/package.json`, `frontend/vitest.config.ts`, `frontend/tests/`
- **Implementado:**
  - Adicionados `vitest`, `@vue/test-utils`, `jsdom` e `@types/jsdom`.
  - Criado `frontend/vitest.config.ts` com ambiente `jsdom` e alias `@/`.
  - Script `"test": "vitest run"` no `package.json`; o script `"format"` passou a incluir `tests/`.
  - Testes existentes migrados para a sintaxe do Vitest.
  - Cobertura de primeira onda: `utils/formatters.ts`, `composables/useMonitorDetail.ts`, `composables/useInfiniteList.ts`, `composables/useInfiniteCursor.ts` e as stores `preferences`, `alerts` e `dashboard`.

### 🟢 FE-04: Inconsistência de Gerenciamento de Estado — Concluído
- `users.ts`, `devices.ts`, `networks.ts`, `sites.ts`, `probes.ts`, `dnsServers.ts` usam `useCrudResource`.

### 🟢 FE-05: Otimização de Bundle — Concluído
- `vite.config.ts` configura `output.manualChunks` separando `vendor-vue` e `vendor-vuetify`.

### 🟢 FE-06: Formatação Duplicada — Concluído
- `utils/formatters.ts` centraliza `formatBytes`, `formatBps`, `formatLatency` etc.

### 🟢 FE-07: Resíduos de Scaffold — Concluído
- `HelloWorld.vue` e imagens de template removidos.

---

## 2. Débitos Técnicos: Backend (Rust / Loco.rs / SeaORM)

### 🟢 BE-01: Duplicação de Algoritmo CIDR — Concluído
- `services::shared::cidr` unifica IPv4/IPv6; `discovery/cidr_range.rs` e `vpn/cidr.rs` re-exportam.

### 🟢 BE-02: Nomenclatura Histórica de Testes — Concluído
- Arquivos `phase*.rs` renomeados para nomes de domínio (`devices_monitors_crud.rs`, `vpn_orchestration.rs`, etc.).

### 🟢 BE-03: Geração de Configurações e Scripts VPN — Concluído
- **Arquivos Afetados:** `backend/src/services/vpn/profiles/{mikrotik.rs,openwrt.rs,variants.rs,wg_conf.rs}`, `backend/src/services/vpn/config_builder.rs`, `backend/src/services/vpn/contract.rs`
- **Implementado:**
  - Criado `backend/src/services/vpn/shell_escape.rs` com `strip_controls`, `escape_wg_value`, `escape_uci`, `escape_routeros` e `sanitize_file_name`.
  - Todos os valores interpolados em `.conf`, scripts UCI/OpenWrt e RouterOS passam pelas funções de escape centralizadas.
  - `PeerConfigContext::prefix_length()` filtra apenas dígitos, evitando que sujeira no CIDR vaze para endereços formatados.
  - Adicionados testes de snapshot para strings maliciosas em `wg_conf` e `mikrotik`.

### 🟢 BE-04: Complexidade do `scheduler_run.rs` — Concluído
- Quebrado em `services/monitoring/scheduler/{monitor_executor.rs,snmp_group_executor.rs,maintenance_runner.rs,cadence.rs}`.

### 🟢 BE-05: Inconsistência na Serialização da API — Concluído
- **Arquivos Afetados:** `backend/src/controllers/{devices.rs,monitors.rs,logs.rs,vpn_peers.rs}`
- **Implementado:**
  - Criados DTOs `ts-rs` em `backend/src/views/vpn.rs` (`VpnNextIpResponse`, `VpnPeerCreatedResponse`, `VpnQrCodeResponse`, `VpnKeyRotationResponse`, `VpnFirewallHintsResponse`, `VpnPeerRevokedResponse`), `backend/src/dtos/devices.rs` (`DeviceMetricItem`, `DeviceEventItem`), `backend/src/dtos/monitors.rs` (`MonitorStats`, `MonitorRunResponse`, `MonitorSnmpRunResponse`, `MonitorWithStats`) e `backend/src/dtos/logs.rs` (`BindSourceResponse`).
  - Controllers de VPN, dispositivos, monitores e logs migrados dos respectivos `serde_json::json!` para os novos DTOs.
  - Bindings TypeScript regenerados e formatados no frontend.

### 🟢 BE-06: Acoplamento MAC-Telnet no Syslog — Concluído
- `mactelnet.rs` movido para `services/network_tools/`.

---

## 3. Débitos Técnicos: Banco de Dados & Persistência

### 🟢 DB-01: Abstração de Busca Textual — Concluído
- FTS5 no SQLite, `tsvector` + GIN no PostgreSQL, `LIKE` como fallback, escolha em runtime por densidade.

### 🗓️ DB-02: Versionamento de Migrations — Planejado (ADR 017)
- **Descrição:** 64 migrations históricas, mais o `migration::purge_removed_migrations` para bancos que aplicaram migrations já removidas.
- **Mudança de premissa (2026-11-05):** o sistema nunca foi para produção, então não há `seaql_migrations` real a preservar. A orientação anterior ("manter como está") caiu.
- **Plano:** consolidar numa base por crate e editar no lugar até a primeira instalação real — Fase 1 do [roadmap da arquitetura modular](roadmap_arquitetura_modular.md).

---

## 4. Débitos Técnicos: DevOps, Docker & Infraestrutura

### 🟢 DO-01: Supervisão de Subprocessos — Concluído
- Heartbeat atômico no `wireguard-watcher.sh` e healthcheck HTTP no `docker-compose.yml`.

### 🟢 INF-01: CI em Diretório Ignorado pelo GitHub Actions — Concluído
- **Arquivos Afetados:** `.github/workflows/ci.yaml` (workflow movido da raiz de `backend/`)
- **Implementado:** workflow na raiz com `defaults.run.working-directory: backend`; jobs de `fmt`, `clippy`, testes SQLite/Postgres, `cargo audit`, frontend (`typecheck`, `lint`, `build`), `npm audit` e build Docker; todas as actions pinadas por SHA.

### 🟢 INF-02: Supervisão do Watcher e Hardening do Container — Concluído
- **Arquivos Afetados:** `docker/entrypoint.sh`, `docker/wireguard-watcher.sh`, `docker/healthcheck.sh`, `Dockerfile`, `docker-compose.yml`
- **Implementado:**
  - `tini` como PID 1; entrypoint supervisiona e reinicia o watcher se ele morrer.
  - Healthcheck considera vida do watcher via `/tmp/wireguard-watcher.heartbeat`.
  - Imagens base (`node`, `rust`, `debian`, `postgres`) pinadas por digest.
  - Compose com `read_only: true`, `tmpfs`, `security_opt: [no-new-privileges:true]` e rotação de logs.

---

## 5. Débitos de Segurança Identificados na Auditoria de 2026-08-20

### 🟢 SEC-01: Segredos de Fallback Públicos — Concluído
- **Descrição:** `JWT_SECRET` e `ENCRYPTION_KEY` tinham defaults públicos.
- **Implementado:** `.env.example` sem valor padrão para `JWT_SECRET`; `require_jwt_secret_in_production` (`backend/src/app.rs`) falha o boot em produção sem o segredo.

### 🟢 SEC-02: Chave Privada WireGuard em Claro no Repositório — Concluído
- **Descrição:** `backend/tmp/wireguard/wg0.conf` com `PrivateKey` estava rastreado e entrava na imagem.
- **Implementado:** arquivo fora do Git; `.dockerignore` lista `**/.env` e `backend/tmp/`.

### 🟢 SEC-04: Receptor de Resultados de Probe Não Vincula Monitor ao Probe — Concluído
- **Descrição:** o receptor gravava resultado de qualquer `monitor_id`.
- **Implementado:** `services/probes/receiver.rs` descarta resultado e execução cujo `probe_id` não seja o do probe autenticado.

### 🟢 SEC-05: Injeção de Configuração `wg0.conf` via Nome de Peer — Concluído
- **Descrição:** nomes de peer sem validação eram interpolados no `wg0.conf`.
- **Implementado:** validação centralizada em `services/vpn/peer_name`, usada por `controllers/vpn_peers.rs`.

### 🟢 SEC-06: `POST /api/probes` Aceita `token_hash` do Cliente — Concluído
- **Descrição:** o cliente podia enviar o hash que quisesse.
- **Implementado:** `controllers/probes.rs` recebe o token cru e grava `sha256_hex(&token)`.

### 🟢 SEC-07: JWT Armazenado em `localStorage` — Concluído (mitigado)
- **Arquivos Afetados:** `frontend/src/services/apiService.ts:28-30`, `backend/src/spa.rs`
- **Decisão:** manter o token em `localStorage` e aplicar CSP restritivo (`script-src 'self'`, `frame-ancestors 'none'`, headers de segurança).
- **Racional:** a migração para cookie `HttpOnly` exigiria reescrita do fluxo de autenticação e proteção CSRF para mutações. O CSP endurecido elimina o vetor XSS mais comum (scripts inline/injetados), mantendo a arquitetura atual. A decisão está documentada no `roadmap.md` e no próprio `apiService.ts`.

### 🟢 SEC-08: Headers de Segurança no Servidor Estático — Concluído
- **Arquivos Afetados:** `backend/src/spa.rs`
- **Implementado:** `Content-Security-Policy`, `X-Content-Type-Options: nosniff`, `Referrer-Policy: strict-origin-when-cross-origin`, `X-Frame-Options: DENY` em todos os arquivos estáticos.

### 🟢 SEC-09: Magic Link com Allowlist de Scaffold — Concluído
- **Arquivos Afetados:** `backend/src/controllers/auth.rs`
- **Implementado:** allowlist `@example.com`/`@gmail.com` removida; magic link aceita qualquer e-mail válido sem revelar se o usuário existe.

### 🟢 SEC-10: DTOs de Credencial Derivam `Debug` — Concluído
- **Arquivos Afetados:** `backend/src/models/users.rs`
- **Implementado:** `LoginParams` e `RegisterParams` implementam `Debug` manualmente, redigindo a senha com `[REDACTED]`.

---

## 6. Débitos de Qualidade e Manutenibilidade Identificados

### 🟢 QUA-01: Novos Componentes/Páginas Monolíticas — Concluído
- **Arquivos Afetados:** `frontend/src/pages/DashboardPage.vue`, `SettingsPage.vue`, `AlertsPage.vue`
- **Implementado:**
  - `DashboardPage.vue`: extraídos `StatCardsWidget`, `ActiveAlertsWidget`, `EventsFeedWidget` e `NetworkMonitorsWidget` para `frontend/src/components/dashboard/`.
  - `AlertsPage.vue`: extraídas as quatro abas (`ActiveAlertsTab`, `ResolvedAlertsTab`, `AlertRulesTab`, `AlertHistoryTab`) para `frontend/src/components/alerts/`.
  - `SettingsPage.vue`: extraídos `PreferencesCard`, `ServerAddressesCard`, `DashboardSyncCard`, `NotificationsCard`, `OnboardingCard` e `BackupCard` para `frontend/src/components/settings/`.
  - Páginas mantêm apenas orquestração, diálogos e feedback.

### 🟢 QUA-02: `reqwest::Client` Recriado a Cada Checagem HTTP — Concluído
- **Arquivos Afetados:** `backend/src/services/monitoring/checkers/http.rs:41-90`
- **Descrição:** desperdiça conexões e repete TLS handshakes.
- **Implementado:**
  - Dois clientes cacheados via `OnceLock<Result<Client, reqwest::Error>>` (`DEFAULT_CLIENT` e `DANGEROUS_CLIENT`).
  - O cliente padrão mantém a verificação TLS; o segundo aceita certificados inválidos quando `validateCertificate: false`.
  - O timeout é aplicado por requisição, sem recriar o cliente.
  - Testes unitários cobrem cache e checagens contra servidor TCP local.

### 🟢 QUA-03: `lookup_host` sem Timeout — Concluído
- **Arquivos Afetados:** `backend/src/services/monitoring/checkers/ping.rs`, `backend/src/services/snmp/client.rs`, `backend/src/services/monitoring/checkers/snmp.rs`
- **Descrição:** resolução DNS podia travar o monitor por tempo indefinido.
- **Implementado:**
  - `resolve_host` no ping e `resolve_target` no SNMP usam `tokio::time::timeout` de 5 s.
  - Timeout de DNS no ping é traduzido para `CheckResult` com `status: unknown` e mensagem clara.
  - Timeout de DNS no SNMP é propagado como `SnmpError::Timeout` e convertido para `CheckResult` `unknown` no checker SNMP.
  - Testes unitários cobrem IP literal, mapeamento de timeout para `unknown` e erros DNS/rede como `down`.

### 🟢 QUA-04: N+1 por Interface na Coleta SNMP — Concluído
- **Arquivos Afetados:** `backend/src/services/snmp/service.rs`, `backend/tests/requests/snmp_collection_integration.rs`
- **Descrição:** loop sobre interfaces fazia uma query/escrita por interface.
- **Implementado:**
  - `poll_device` carrega todas as interfaces conhecidas do dispositivo em uma única query antes do loop de sincronização.
  - `sync_interface` recebe `Option<&device_interfaces::Model>` para evitar SELECT individual por interface.
  - `latest_metrics_for_interfaces` busca as métricas anteriores de todas as interfaces em uma única query.
  - `build_traffic_metrics` e `build_system_metrics` acumulam `PendingMetric`; `record_metrics_bulk` insere tudo com `metrics::Entity::insert_many`.
  - Testes de SNMP (unitários e integração) continuam passando.

### 🟢 QUA-05: Coluna `monitors.timeout_seconds` Honrada — Concluído
- **Arquivos Afetados:** `backend/src/services/monitoring/execution_guard.rs`, `backend/src/services/monitoring/scheduler/monitor_executor.rs`, `backend/src/services/monitoring/presenter.rs`, `frontend/src/stores/monitors.ts`, `frontend/src/components/monitors/MonitorDetailView.vue`
- **Descrição:** o scheduler usava `calculate_smart_timeout_seconds` baseado apenas no intervalo, ignorando a coluna.
- **Implementado:**
  - Adicionado `effective_timeout_seconds(timeout_seconds, interval_seconds)` em `execution_guard.rs`, aplicando mínimo de 1 s e máximo de `interval - 1`.
  - `monitor_executor.rs` converte `effective_timeout_seconds(monitor.timeout_seconds, monitor.interval_seconds)` para milissegundos e aplica no `RunOptions`.
  - `MonitorPresentation` expõe `timeout_seconds` para a UI; o frontend mantém o campo no tipo `Monitor` e no objeto `emptyMonitor`.
  - Decisão: honrar a coluna, mantendo-a no contrato.

### 🟢 QUA-06: Buffer Offline do Probe com Limite e Escrita Atômica — Concluído
- **Arquivos Afetados:** `backend/src/services/probes/buffer.rs`
- **Descrição:** arquivo crescia sem limite; cada resultado lia e regrava o arquivo inteiro; escrita não era atômica.
- **Implementado:**
  - Limites configuráveis: `PROBE_BUFFER_MAX_RESULTS` (padrão 10.000 itens) e `PROBE_BUFFER_MAX_BYTES` (padrão 50 MB).
  - Gravação atômica via arquivo temporário + `tokio::fs::rename`; tmp órfão de crash anterior é removido antes de nova escrita.
  - Ao atingir o teto de bytes, deduplica por `monitor_id` mantendo o resultado mais recente de cada monitor; se ainda exceder, trunca os itens mais antigos.
  - Leitura tolerante a arquivo ausente, vazio ou corrompido.
  - Testes cobrem acúmulo/limpeza, arquivo corrompido, buffer separado de discovery, limite de itens, limite de bytes, deduplicação, truncamento por idade e escrita atômica com recuperação de tmp órfão.

---

## 7. Débitos de Arquitetura Modular (ADR 017)

Levantados em 2026-11-05. A decisão está no [ADR 017](adr/017-arquitetura-modular.md); o plano, no [roadmap da arquitetura modular](roadmap_arquitetura_modular.md), que cita estes IDs em cada fase.

### 🗓️ ARQ-01: Composição Estática — Planejado
- **Arquivos Afetados:** `backend/src/app.rs` (38 `add_route`), `backend/src/initializers/monitoring.rs`, `frontend/src/layouts/DefaultLayout.vue` (`navSections`), `frontend/src/router/index.ts`, `frontend/src/components/setup/InitialSetupDialog.vue`
- **Descrição:** toda instalação recebe os dois domínios: ~36 entradas de menu, onboarding só de rede, ~60 ferramentas de IA e rotinas próprias de áreas que não usa.
- **Plano:** áreas `on`/`off` com registros declarativos (Fases 0, 2, 3, 4 e 5).

### 🗓️ ARQ-02: Dependências na Direção Errada no Backend — Planejado
- **Arquivos Afetados:** `services/monitoring/runner.rs` (A1), `services/{devices/identify.rs,discovery/decider_identity.rs,snmp/decider_interfaces.rs,syslog/categorizer.rs}` (A2), `services/ai/harness/tools/{mod,docker,platform}.rs` e `ai/mentions.rs` (A3), `services/dashboard/catalog.rs` (A4), `services/agents/background.rs` e `services/agent_runtime/` (A5)
- **Descrição:** o núcleo conhece Docker, a Rede chama o Decisor diretamente e a IA conhece todos os domínios.
- **Plano:** pontos de extensão no `core` (`MonitorChecker`, `Classifier`, `AiToolHandler::area`, `WidgetSpec`, `AgentCommandHandler`) — Fases 1 e 6 a 9. Até lá, o teste de arquitetura congela a lista e impede novas.

### 🗓️ ARQ-03: Barramento SSE Central e Dashboard Acoplado ao Docker — Planejado
- **Arquivos Afetados:** `frontend/src/stores/events.ts`, `frontend/src/pages/DashboardPage.vue`
- **Descrição:** `events.ts` importa 26 stores e roteia ~58 tópicos num `switch`: área nova exige editar o centro (fere o Aberto/Fechado). O `case 'device:updated'` aparece duas vezes (o segundo nunca executa). `DashboardPage` sempre importa a store Docker.
- **Plano:** registro de handlers por prefixo; o widget Docker carrega a própria store (Fase 3).

### 🗓️ ARQ-04: Catálogo de Widgets Duplicado — Planejado
- **Arquivos Afetados:** `backend/src/services/dashboard/catalog.rs`, `frontend/src/stores/dashboard.ts` (`CARD_TEMPLATES`, `DEFAULT_WIDGETS`, `parseSavedList`)
- **Descrição:** os tipos de widget existem nos dois lados (fere o DRY), e `parseSavedList` devolve ao layout todo widget padrão ausente — com as áreas, traria o `docker_summary` de volta sozinho.
- **Plano:** catálogo único no backend, lido pelo frontend via `ts-rs`, com `area()` por widget (Fase 2).

### 🗓️ ARQ-05: Snapshot Docker com Qualquer Aba Aberta — Planejado
- **Arquivos Afetados:** `backend/src/services/agents/background.rs` (`desired_live_interval(has_subscribers)`)
- **Descrição:** basta um assinante SSE, em qualquer página, para a central pedir `SetLive` a todos os agentes e distribuir snapshots Docker.
- **Plano:** considerar a área Servidores e a presença de quem usa dados Docker (Fase 5).

### 🗓️ ARQ-06: Crate Único e Fronteira por Convenção — Planejado
- **Arquivos Afetados:** `backend/` (crate único, ~185 mil linhas)
- **Descrição:** nada impede uma dependência nova entre domínios além da revisão humana; a compilação do crate inteiro estoura a paginação nesta máquina (erro 1455).
- **Plano:** workspace com crates por área e reconstrução no lugar (Fases 1 e 6 a 10).

---

## 8. Onde está o plano

Entregas, ordem e critérios de aceite: [`roadmap.md`](roadmap.md) (roadmap mestre) e [`roadmap_arquitetura_modular.md`](roadmap_arquitetura_modular.md) (ADR 017).
