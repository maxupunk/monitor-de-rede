# Diagnóstico e Catálogo de Débitos Técnicos

Este documento consolida a auditoria de arquitetura, código-fonte, padrões de design, duplicações, segurança e gaps de teste do **NetMonitor**. A última revisão completa foi em **2026-08-20**; o estado real do código prevalece sobre qualquer descrição deste arquivo.

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
| **FE-03** | Frontend | Ausência de testes automatizados e script `test` no frontend | Alto | Médio | 🔴 Pendente |
| **FE-04** | Frontend | Inconsistência no padrão de stores (`useCrudResource`) | Médio | Pequeno | 🟢 Concluído |
| **FE-05** | Frontend | Bundle size e code-splitting no Vite | Médio | Pequeno | 🟢 Concluído |
| **FE-06** | Frontend | Formatação duplicada de taxa/bytes | Médio | Pequeno | 🟢 Concluído |
| **FE-07** | Frontend | Resíduos de scaffold inicial | Baixo | Mínimo | 🟢 Concluído |
| **BE-01** | Backend | Duplicação de algoritmo CIDR | Médio | Pequeno | 🟢 Concluído |
| **BE-02** | Backend | Nomenclatura histórica de testes (`phase*.rs`) | Médio | Pequeno | 🟢 Concluído |
| **BE-03** | Backend | Geração de configurações/scripts VPN por concatenação de strings | Médio | Médio | 🟡 Parcial |
| **BE-04** | Backend | Complexidade do `scheduler_run.rs` | Médio | Médio | 🟢 Concluído |
| **BE-05** | Backend | Inconsistência entre `serde_json::json!` e DTOs tipados | Médio | Médio | 🟡 Parcial |
| **BE-06** | Backend | Acoplamento MAC-Telnet no módulo Syslog | Baixo | Pequeno | 🟢 Concluído |
| **DB-01** | Banco | Abstração FTS5 vs `tsvector`/GIN | Médio | Médio | 🟢 Concluído |
| **DO-01** | DevOps | Supervisão de subprocessos no container | Médio | Médio | 🟢 Concluído |
| **SEC-01** | Segurança | Segredos de fallback públicos (`JWT_SECRET`, `ENCRYPTION_KEY`) | Crítico | Pequeno | 🟢 Concluído |
| **SEC-02** | Segurança | Chave privada WireGuard em claro no repositório (`backend/tmp/wireguard/wg0.conf`) | Crítico | Pequeno | 🟢 Concluído |
| **SEC-03** | Segurança | CI em diretório ignorado pelo GitHub Actions (`backend/.github/`) | Crítico | Pequeno | 🔴 Pendente |
| **SEC-04** | Segurança | Receptor de resultados de probe não vincula monitor ao probe autenticado | Alto | Médio | 🟢 Concluído |
| **SEC-05** | Segurança | Injeção de configuração `wg0.conf` via nome de peer não sanitizado | Alto | Pequeno | 🟡 Parcial |
| **SEC-06** | Segurança | `POST /api/probes` aceita `token_hash` arbitrário do cliente | Alto | Pequeno | 🟢 Concluído |
| **SEC-07** | Segurança | JWT armazenado em `localStorage` no frontend | Alto | Médio | 🔴 Pendente |
| **SEC-08** | Segurança | Headers de segurança ausentes no servidor estático | Baixo | Pequeno | 🔴 Pendente |
| **SEC-09** | Segurança | Magic link restrito a `@example.com`/`@gmail.com` (resíduo de scaffold) | Médio | Pequeno | 🔴 Pendente |
| **SEC-10** | Segurança | DTOs de credencial derivam `Debug` expondo senha | Médio | Pequeno | 🔴 Pendente |
| **QUA-01** | Qualidade | Novos componentes/páginas monolíticas cresceram (`DashboardPage`, `AlertsPage`, `SettingsPage`) | Médio | Médio | 🔴 Pendente |
| **QUA-02** | Qualidade | `reqwest::Client` recriado a cada checagem HTTP | Médio | Pequeno | 🔴 Pendente |
| **QUA-03** | Qualidade | `lookup_host` sem timeout em ping e SNMP | Médio | Pequeno | 🔴 Pendente |
| **QUA-04** | Qualidade | N+1 por interface na coleta SNMP | Médio | Médio | 🔴 Pendente |
| **QUA-05** | Qualidade | Coluna `monitors.timeout_seconds` escrita e nunca lida | Médio | Pequeno | 🔴 Pendente |
| **QUA-06** | Qualidade | Buffer offline do probe sem teto e com reescrita O(n²) | Médio | Médio | 🔴 Pendente |
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

### 🔴 FE-03: Ausência de Infraestrutura e Suíte de Testes no Frontend
- **Arquivos Afetados:** `frontend/package.json`, `frontend/tests/`
- **Descrição:** o `package.json` não possui script `"test"`. Não há `vitest`, `@vue/test-utils`, `jsdom`, `cypress` ou `playwright`. Os dois arquivos em `frontend/tests/` (`formatters.test.ts`, `ndjson.test.ts`) não são executáveis.
- **Impacto:** regressões no frontend só são pegas manualmente; o critério do `AGENTS.md` não cobre testes de frontend, mas a ausência deles é risco operacional.
- **Recomendação:** adicionar `vitest` + `@vue/test-utils` + `jsdom`, script `"test"`, e cobrir pelo menos `utils/formatters.ts`, `composables/` e stores puras.

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

### 🟡 BE-03: Geração de Configurações e Scripts VPN — Parcial
- **Arquivos Afetados:** `backend/src/services/vpn/profiles/{mikrotik.rs,openwrt.rs,variants.rs,wg_conf.rs}`
- **Descrição:** existem builders tipados e sanitização de `community`/`dns`, mas ainda há interpolação direta de `peer_name`, `client_private_key`, `server_public_key`, `endpoint_host` e `vpn_cidr`. A sanitização não cobre todos os campos nem garante defesa em profundidade no gerador final.
- **Recomendação:** centralizar escape/sanitização de todos os valores interpolados; adicionar testes de snapshot para strings maliciosas.

### 🟢 BE-04: Complexidade do `scheduler_run.rs` — Concluído
- Quebrado em `services/monitoring/scheduler/{monitor_executor.rs,snmp_group_executor.rs,maintenance_runner.rs,cadence.rs}`.

### 🟡 BE-05: Inconsistência na Serialização da API — Parcial
- **Arquivos Afetados:** `backend/src/controllers/{devices.rs,monitors.rs,logs.rs,vpn_peers.rs}`
- **Descrição:** `DevicePresenterItem` e `VpnPeerResponse` introduziram DTOs tipados, mas ainda há dezenas de `serde_json::json!` nos mesmos controllers e em outros. Isso mantém risco de drift de contrato e dificulta a geração automática de bindings TypeScript.
- **Recomendação:** mapear todos os endpoints para DTOs `ts-rs`; eliminar `serde_json::json!` de respostas de negócio.

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

### 🔴 INF-01: CI em Diretório Ignorado pelo GitHub Actions
- **Arquivos Afetados:** `backend/.github/workflows/ci.yaml`
- **Descrição:** o GitHub Actions só lê `.github/workflows/` na raiz. O workflow atual nunca roda; nenhum gate de qualidade é executado na prática.
- **Recomendação:** mover para `.github/workflows/ci.yaml` com `defaults.run.working-directory: backend`; adicionar jobs de frontend, auditoria (`cargo audit`, `npm audit`), build Docker e pin de actions por SHA.

### 🟡 INF-02: Supervisão do Watcher e Hardening do Container
- **Arquivos Afetados:** `docker/entrypoint.sh:54`, `Dockerfile`, `docker-compose.yml`
- **Descrição:** o watcher inicia com `&` e vira órfão; não há `tini`/supervisão. Imagens base não estão pinadas por digest. Faltam `security_opt: [no-new-privileges:true]`, `read_only` com `tmpfs` e rotação de logs no compose.
- **Recomendação:** adotar `tini` como init ou supervisão mínima; pinar imagens por digest; endurecer compose.

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

### 🔴 SEC-07: JWT Armazenado em `localStorage`
- **Arquivos Afetados:** `frontend/src/services/apiService.ts:28-30`
- **Descrição:** o token JWT continua em `localStorage`, vulnerável a XSS e leitura por extensões. O `roadmap_auditoria_seguranca.md` marca este item como concluído, mas o código não reflete isso.
- **Recomendação:** migrar para cookie `HttpOnly` com CSRF adequado, ou — se manter localStorage — implementar CSP estrito e validar a decisão documental.

### 🔴 SEC-08: Headers de Segurança Ausentes no Servidor Estático
- **Arquivos Afetados:** `backend/src/spa.rs:72-86`
- **Descrição:** só há `Cache-Control`. Faltam CSP, `X-Content-Type-Options`, `Referrer-Policy`, `frame-ancestors`.
- **Recomendação:** adicionar headers mínimos de segurança; CSP pode começar restritivo e ser relaxado conforme necessário.

### 🟡 SEC-09: Magic Link com Allowlist de Scaffold
- **Arquivos Afetados:** `backend/src/controllers/auth.rs:20-25`
- **Descrição:** magic link só permite `@example.com` e `@gmail.com`, resíduo de scaffold que impede uso real.
- **Recomendação:** remover allowlist ou remover o fluxo de magic link.

### 🟡 SEC-10: DTOs de Credencial Derivam `Debug`
- **Arquivos Afetados:** `backend/src/models/users.rs:30-33,35-44`
- **Descrição:** `LoginParams` e `RegisterParams` derivam `Debug`, expondo senha em logs e erros.
- **Recomendação:** implementar `Debug` manual redigindo a senha.

---

## 6. Débitos de Qualidade e Manutenibilidade Identificados

### 🟡 QUA-01: Novos Componentes/Páginas Monolíticas
- **Arquivos Afetados:** `frontend/src/pages/DashboardPage.vue` (~937 linhas), `SettingsPage.vue` (~573 linhas), `AlertsPage.vue` (~859 linhas)
- **Descrição:** após a refatoração FE-01, novas páginas cresceram sem decomposição equivalente.
- **Recomendação:** extrair widgets/tabelas/filtros para componentes menores; seguir o mesmo padrão de `DeviceDetailPage`.

### 🟡 QUA-02: `reqwest::Client` Recriado a Cada Checagem HTTP
- **Arquivos Afetados:** `backend/src/services/monitoring/checkers/http.rs:51-55`
- **Descrição:** desperdiça conexões e repete TLS handshakes.
- **Recomendação:** cachear um ou dois clientes via `OnceLock` ou estado do checker.

### 🟡 QUA-03: `lookup_host` sem Timeout
- **Arquivos Afetados:** `backend/src/services/monitoring/checkers/ping.rs:111`, `backend/src/services/snmp/client.rs:359`
- **Descrição:** resolução DNS pode travar o monitor por tempo indefinido.
- **Recomendação:** envolver em `tokio::time::timeout`.

### 🟡 QUA-04: N+1 por Interface na Coleta SNMP
- **Arquivos Afetados:** `backend/src/services/snmp/service.rs:265-301`
- **Descrição:** loop sobre interfaces faz uma query/escrita por interface.
- **Recomendação:** buscar métricas anteriores em uma query por grupo; separar coleta, persistência e efeitos para manter transações curtas.

### 🟡 QUA-05: Coluna `monitors.timeout_seconds` Não Lida
- **Arquivos Afetados:** `backend/src/models/monitors.rs:111`, `backend/src/controllers/monitors.rs:217-234`
- **Descrição:** o scheduler usa `calculate_smart_timeout_seconds` baseado apenas no intervalo.
- **Recomendação:** honrar a coluna ou removê-la do contrato.

### 🟡 QUA-06: Buffer Offline do Probe sem Teto e com Reescrita O(n²)
- **Arquivos Afetados:** `backend/src/services/probes/buffer.rs:73-90`
- **Descrição:** arquivo cresce sem limite; cada resultado lê e regrava o arquivo inteiro; escrita não é atômica.
- **Recomendação:** adicionar tamanho máximo; usar tmp+rename; considerar SQLite ou segmentação.

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

O detalhamento completo das entregas, cronograma e critérios de aceite está em [`docs/roadmap.md`](roadmap.md).
