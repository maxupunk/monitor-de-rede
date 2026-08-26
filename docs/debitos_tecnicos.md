# Diagnóstico e Catálogo de Débitos Técnicos

Este documento consolida a auditoria de arquitetura, código-fonte, padrões de design, duplicações, segurança e gaps de teste do **NetMonitor**. A última revisão completa foi em **2026-08-21**; o estado real do código prevalece sobre qualquer descrição deste arquivo.

---

## 📊 Sumário Executivo & Métricas da Auditoria (2026-08-20)

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

### 🟢 DB-02: Versionamento de Migrations
- Débito cosmético sem impacto operacional; manter como está para não invalidar `seaql_migrations`.

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

### 🔴 SEC-01: Segredos de Fallback Públicos
- **Arquivos Afetados:** `.env.example:55,63`, `docker-compose.yml:82,87`, `backend/config/{production.yaml:70,development.yaml:117,test.yaml:104}`
- **Descrição:** `JWT_SECRET` e `ENCRYPTION_KEY` têm defaults públicos. Quem não customiza o `.env` roda com autenticação e cifra decorativas.
- **Recomendação:** remover defaults dos três arquivos; em produção, fazer o boot falhar se os segredos não estiverem definidos (já existe o padrão para `ENCRYPTION_KEY` em `crypto.rs`).

### 🔴 SEC-02: Chave Privada WireGuard em Claro no Repositório
- **Arquivos Afetados:** `backend/tmp/wireguard/wg0.conf`, `.dockerignore`
- **Descrição:** o arquivo contém `PrivateKey` de servidor WireGuard em claro, está rastreado pelo Git e não é excluído por `.dockerignore` (que só lista `.env` da raiz, não `**/.env`, e não lista `backend/tmp/`).
- **Recomendação:** remover o arquivo do repositório, rotacionar a chave (considerá-la comprometida), adicionar `backend/tmp/` e `**/.env` ao `.dockerignore`.

### 🔴 SEC-04: Receptor de Resultados de Probe Não Vincula Monitor ao Probe
- **Arquivos Afetados:** `backend/src/services/probes/receiver.rs:40-67`, `backend/src/controllers/probes.rs:275-289`
- **Descrição:** o receptor aceita `monitor_id` e grava sem conferir `monitor.probe_id == probe.id`. Combinado ao token padrão público (`DEFAULT_VPN_PROBE_TOKEN`), permite injetar resultados falsos em qualquer monitor. O endpoint de tarefas também não filtra por `role`.
- **Recomendação:** descartar resultados cujo monitor não pertença ao probe; filtrar tarefas por role; logar `warn!` quando o token efetivo for o padrão.

### 🟡 SEC-05: Injeção de Configuração `wg0.conf` via Nome de Peer
- **Arquivos Afetados:** `backend/src/services/vpn/config_builder.rs:69`, `backend/src/controllers/vpn_peers.rs:183-197,247-254`
- **Descrição:** `controllers/devices.rs` valida nome/tipo de dispositivo contra `\n`/`\r`, mas `vpn_peers.rs` não valida nomes de peer. `config_builder.rs` interpola `peer.name` em comentário sem defesa em profundidade.
- **Recomendação:** rejeitar controles em nomes de peer e sanitizar no gerador final.

### 🔴 SEC-06: `POST /api/probes` Aceita `token_hash` do Cliente
- **Arquivos Afetados:** `backend/src/controllers/probes.rs:163-165`
- **Descrição:** o endpoint permite que o cliente envie o hash que quiser, em vez de receber o token cru e hashear no servidor.
- **Recomendação:** receber token cru, gerar `sha256` no servidor e nunca aceitar `token_hash` na criação.

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

## 7. Roteiro Resumido de Ação

1. **Fase 0 — Segurança & CI (não negociável):**
   - Remover segredos públicos e chave privada do repo; rotacionar chave.
   - Mover CI para `.github/workflows/` e adicionar frontend/auditoria/build Docker.
   - Vincular resultados de probe ao probe; sanitizar nomes de peer; corrigir `POST /probes`.

2. **Fase 1 — Higiene de frontend e infra:**
   - Headers de segurança no servidor estático.
   - Limpar `lastArtifact` ao fechar visualizador VPN; DOMPurify no QR code.
   - Supervisão do watcher e hardening do container.

3. **Fase 2 — Qualidade e performance:**
   - Infraestrutura de testes no frontend.
   - Cachear `reqwest::Client`; timeouts em `lookup_host`.
   - Reduzir N+1 SNMP; resolver `timeout_seconds`.
   - Melhorar buffer offline do probe.

4. **Fase 3 — Evolução de produto (iniciada):**
   - Janelas de manutenção: silenciar notificações por site/dispositivo em intervalo agendado.
   - Rollup/agregação de métricas: tabela `monitor_results_hourly`, job no scheduler, endpoint `GET /api/monitors/:id/uptime` e card de estabilidade no detalhe do dispositivo.

O detalhamento completo das entregas, cronograma e critérios de aceite está em [`docs/roadmap.md`](roadmap.md).
