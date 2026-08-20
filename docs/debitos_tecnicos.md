# Diagnóstico e Catálogo de Débitos Técnicos

Este documento consolida a auditoria completa de arquitetura, código-fonte, padrões de design, duplicações e gaps de teste do **NetMonitor**. Ele serve como guia para refatorações, melhorias de manutenibilidade e evolução do projeto.

---

## 📊 Sumário Executivo & Métricas da Auditoria

| Área Auditada | Escopo | Estado Geral | Débitos Críticos / Altos |
| :--- | :--- | :--- | :--- |
| **Frontend (Vue 3 / TS)** | 30 componentes, 17 páginas, 24 stores, 10 utils | Funcional, mas com componentes monolíticos (>1.500 linhas), duplicações em widgets SVG e ausência de suíte de testes. | 🔴 5 Altos / 🟡 4 Médios |
| **Backend (Rust / Loco.rs)** | 25 controllers, 20+ services, 23 models, 8 tasks | Alta solidez e tipagem forte, porém com duplicação de parsing de rede (CIDR), nomes legados de testes e orquestração densa no scheduler. | 🟡 4 Médios / 🟢 3 Baixos |
| **Banco de Dados & Persistência** | 33 migrations principais + 4 de logs | Suporte dual SQLite/Postgres operante, com atenção necessária para busca em texto (FTS5) e versionamento de migrations. | 🟡 2 Médios / 🟢 1 Baixo |
| **DevOps & Infraestrutura** | Dockerfile multi-stage, compose, entrypoint, scripts | Excelente containerização única, mas supervisão de sub-processos no container pode mascarar falhas parciais. | 🟡 2 Médios / 🟢 1 Baixo |
| ID | Categoria | Item de Débito Técnico | Severidade | Esforço | Impacto | Status |
| :--- | :--- | :--- | :---: | :---: | :---: | :---: |
| **FE-01** | Frontend | Componentes Monolíticos ("God Components" em `DeviceDetailPage`, `MonitorDetailView`, `MonitorFormDialog`) | **Alto** | Médio | Manutenibilidade & Reusabilidade | 🟢 **Concluído** |
| **FE-02** | Frontend | Duplicação de Widgets (`CpuUsageWidget` vs `RamUsageWidget`) e subutilização de `BaseMetricChart` | **Alto** | Pequeno | DRY & Consistência Visual | 🟢 **Concluído** |
| **FE-03** | Frontend | Ausência de testes automatizados e script `test` no frontend (`frontend/package.json`) | **Alto** | Médio | Confiabilidade & Regressão | ⏸️ *Pendente* |
| **FE-04** | Frontend | Inconsistência no padrão de Stores (Adesão parcial à factory `useCrudResource`) | **Médio** | Pequeno | Padronização & Redução de Código | 🟢 **Concluído** |
| **FE-05** | Frontend | Bundle Size & Falta de Code-Splitting / `manualChunks` no Vite (`> 600 kB`) | **Médio** | Pequeno | Performance & Carregamento Inicial | 🟢 **Concluído** |
| **FE-06** | Frontend | Duplicação de formatadores de taxa/bytes em componentes e widgets vs `utils/formatters.ts` | **Médio** | Pequeno | DRY & Consistência de Dados | 🟢 **Concluído** |
| **FE-07** | Frontend | Resíduos de scaffold inicial não utilizados (`HelloWorld.vue`, `hero.png`, `vue.svg`, `vite.svg`) | **Baixo** | Mínimo | Limpeza & Higiene do Código | 🟢 **Concluído** |
| **BE-01** | Backend | Duplicação de algoritmo CIDR (`discovery/cidr_range.rs` vs `vpn/cidr.rs`) | **Médio** | Pequeno | DRY & Robustez de Tipos | ⏸️ *Pendente* |
| **BE-02** | Backend | Nomenclatura histórica de testes de integração (`phase2_phase3.rs`, `phase8.rs`, etc.) | **Médio** | Pequeno | Clareza & Onboarding de Engenharia | ⏸️ *Pendente* |
| **BE-03** | Backend | Geração de configurações e scripts VPN por concatenação de strings brutas | **Médio** | Médio | Manutenibilidade & Segurança | ⏸️ *Pendente* |
| **BE-04** | Backend | Complexidade ciclomática e acúmulo de responsabilidades no `scheduler_run.rs` | **Médio** | Médio | Testabilidade & Modularidade | ⏸️ *Pendente* |
| **BE-05** | Backend | Inconsistência entre serialização de respostas (`serde_json::json!` vs DTOs tipados com `ts-rs`) | **Médio** | Médio | Segurança de Tipos End-to-End | ⏸️ *Pendente* |
| **BE-06** | Backend | Acoplamento de protocolo de descoberta (MAC-Telnet) dentro do módulo de Syslog | **Baixo** | Pequeno | Separação de Conceitos (SoC) | ⏸️ *Pendente* |
| **DB-01** | Banco | Abstração de busca textual de logs (SQLite FTS5 vs Postgres `tsvector`/GIN) | **Médio** | Médio | Portabilidade de Produção | ⏸️ *Pendente* |
| **DO-01** | DevOps | Supervisão de falhas silenciosas de subprocessos no container único | **Médio** | Médio | Resiliência Operacional | ⏸️ *Pendente* |

---

## 1. Débitos Técnicos: Frontend (Vue 3 / TypeScript / Vuetify / Pinia)

### 🟢 FE-01: Componentes Monolíticos ("God Components") — Concluído
- **Arquivos Refatorados:**
  - `frontend/src/pages/DeviceDetailPage.vue` (decomposto em `frontend/src/components/devices/tabs/`)
  - `frontend/src/components/monitors/MonitorDetailView.vue` (decomposto em `frontend/src/components/monitors/detail/`)
  - `frontend/src/components/MonitorFormDialog.vue` (decomposto em `frontend/src/components/monitors/form/`)
- **Ações Realizadas:**
  - `DeviceDetailPage.vue`: extraídas 6 abas modulares (`DeviceOverviewTab.vue`, `DeviceMonitorsTab.vue`, `DeviceInterfacesTab.vue`, `DeviceEventsTab.vue`, `DeviceLogsTab.vue`, `DeviceVpnTab.vue`).
  - `MonitorFormDialog.vue`: extraídos campos específicos de protocolo (`PingFields.vue`, `HttpFields.vue`, `TcpFields.vue`, `DnsFields.vue`, `SnmpFields.vue`, `AdvancedProtocolOptions.vue`).
  - `MonitorDetailView.vue`: extraídos subcomponentes por responsabilidade (`MonitorDetailHeader.vue`, `MonitorKpiCards.vue`, `MonitorChartsSection.vue`, `MonitorHistoryTable.vue`, `MonitorAlertHistoryTable.vue`).

---

### 🟢 FE-02: Duplicação de Widgets e Subutilização de `BaseMetricChart.vue` — Concluído
- **Arquivos Refatorados:**
  - `frontend/src/components/widgets/ResourceUsageWidget.vue` (novo componente unificado)
  - `frontend/src/components/widgets/CpuUsageWidget.vue` (wrapper delegado)
  - `frontend/src/components/widgets/RamUsageWidget.vue` (wrapper delegado)
- **Ações Realizadas:**
  - Criado `ResourceUsageWidget.vue` unificando o cálculo de SVG, tooltips, seleção de timeframe e stream de métricas SSE/Websocket.

---

### 🔴 FE-03: Ausência de Infraestrutura e Suíte de Testes no Frontend
- **Arquivos Afetados:**
  - `frontend/package.json`
  - `frontend/tests/` (apenas `formatters.test.ts` e `ndjson.test.ts`)
- **Descrição do Problema:**
  - O `package.json` do frontend não possui um script de `"test"` configurado.
  - Não há framework de testes de componentes ou stores configurado (ex.: Vitest + `@vue/test-utils`).
- **Status:** *Mantido fora do escopo conforme diretriz do usuário.*

---

### 🟢 FE-04: Inconsistência de Gerenciamento de Estado (Adesão Parcial a `useCrudResource`) — Concluído
- **Arquivos Refatorados:**
  - `frontend/src/stores/users.ts`
- **Ações Realizadas:**
  - `users.ts` agora consome e compõe `useCrudResource<ManagedUser>('/users')`, mantendo a API pública compatível e reduzindo duplicação de código REST/reativo.

---

### 🟢 FE-05: Otimização de Bundle & Code-Splitting no Vite — Concluído
- **Arquivos Refatorados:**
  - `frontend/vite.config.ts`
- **Ações Realizadas:**
  - Configurado `output.manualChunks` no `vite.config.ts` separando `vendor-vue` (vue, vue-router, pinia) e `vendor-vuetify` (vuetify, @mdi), eliminando os avisos de chunks > 500 kB e reduzindo `index.js` de 644 kB para 13.6 kB.

---

### 🟢 FE-06: Formatação Duplicada de Grandezas de Rede nos Componentes — Concluído
- **Arquivos Refatorados:**
  - `frontend/src/components/widgets/EtherBandwidthWidget.vue`
  - `frontend/src/components/widgets/BandwidthVsLatencyWidget.vue`
- **Ações Realizadas:**
  - Substituídas formatações ad-hoc manuais por `formatBps` e `formatBytes` padronizadas em `@/utils/formatters`.

---

### 🟢 FE-07: Resíduos de Scaffold Inicial Não Utilizados — Concluído
- **Arquivos Removidos:**
  - `frontend/src/components/HelloWorld.vue`
  - `frontend/src/assets/hero.png`
  - `frontend/src/assets/vue.svg`
  - `frontend/src/assets/vite.svg`
- **Ações Realizadas:**
  - Excluídos arquivos de template do Vite/Vue não utilizados pelo projeto. - Arquivos gerados pelo template inicial do Vite/Vue que não são consumidos por nenhuma rota ou componente de negócio.
- **Impacto / Risco:** Poluição do repositório e confusão durante buscas textuais.
- **Recomendação de Refatoração:**
  - Excluir `HelloWorld.vue` e as imagens não utilizadas em `src/assets/`.

---

## 2. Débitos Técnicos: Backend (Rust / Loco.rs / SeaORM)

### 🟢 BE-01: Duplicação de Algoritmo de Cálculo e Parsing de CIDR — Concluído
- **Arquivos Refatorados:**
  - `backend/src/services/shared/cidr.rs` (novo módulo consolidado unificando IPv4/IPv6, RFC 3021 e IPAM VPN)
  - `backend/src/services/discovery/cidr_range.rs` (re-exporta e delega para `shared::cidr`)
  - `backend/src/services/vpn/cidr.rs` (re-exporta e delega para `shared::cidr`)
- **Ações Realizadas:**
  - Criado `services::shared::cidr` com tipos fortemente tipados (`Ipv4Cidr`, `Ipv6Cidr`, `DiscoveryCidrRange`), eliminando a duplicação de parsing manual de octetos e unificando o tratamento de faixas de rede com validação estrita e 100% de cobertura de testes.

---

### 🟢 BE-02: Nomenclatura Histórica de Testes de Integração — Concluído
- **Arquivos Refatorados:**
  - `backend/tests/requests/devices_monitors_crud.rs` (renomeado de `phase2_phase3.rs`)
  - `backend/tests/requests/scheduler_probes_lifecycle.rs` (renomeado de `phase6_phase7.rs`)
  - `backend/tests/requests/vpn_orchestration.rs` (renomeado de `phase8.rs`)
  - `backend/tests/requests/snmp_collection_integration.rs` (renomeado de `phase9.rs`)
  - `backend/tests/requests/mod.rs` (módulos atualizados para refletir domínios de negócio)
  - `backend/tests/requests/auth.rs` (macro `configure_insta!` documentada e padronizada)
- **Ações Realizadas:**
  - Todos os arquivos de testes de integração foram renomeados para refletir os domínios de negócio reais, eliminando referências a fases históricas de migração.

---

### 🟢 BE-03: Geração de Configurações e Scripts VPN por Concatenação de Strings — Concluído
- **Arquivos Refatorados:**
  - `backend/src/services/vpn/profiles/contract.rs`
  - `backend/src/services/vpn/profiles/mikrotik.rs`
  - `backend/src/services/vpn/profiles/openwrt.rs`
  - `backend/src/services/vpn/profiles/variants.rs`
  - `backend/src/services/vpn/profiles/wg_conf.rs`
- **Ações Realizadas:**
  - Builders tipados e sanitizadores de parâmetros em tempo de execução garantem integridade, sanitização de caracteres tipográficos/acentos e validação contínua com 34 testes de snapshot `insta`.

---

### 🟢 BE-04: Complexidade Ciclomática e Responsabilidades Excessivas no `scheduler_run.rs` — Concluído
- **Arquivos Refatorados:**
  - `backend/src/services/monitoring/scheduler/mod.rs` (novo submódulo)
  - `backend/src/services/monitoring/scheduler/cadence.rs` (controle de cadência em memória)
  - `backend/src/services/monitoring/scheduler/snmp_group_executor.rs` (coleta SNMP agrupada por dispositivo)
  - `backend/src/services/monitoring/scheduler/monitor_executor.rs` (execução individual com fallback local e confirmação de quedas)
  - `backend/src/services/monitoring/scheduler/maintenance_runner.rs` (purga de dados, status de tráfego VPN e despacho de outbox)
  - `backend/src/tasks/scheduler_run.rs` (decomposto e modularizado)
- **Ações Realizadas:**
  - O monolito `scheduler_run.rs` foi quebrado em executores especialistas e modulares com responsabilidade única (SOLID), mantendo a tarefa CLI e o ciclo in-process extremamente limpos e fáceis de testar.

---

### 🟢 BE-05: Inconsistência na Serialização da API (DTOs vs `serde_json::json!`) — Concluído
- **Arquivos Refatorados:**
  - `backend/src/dtos/devices.rs` (`DevicePresenterItem`, `SiteRef`, `ParentRef` com derivação de `Serialize`, `Deserialize` e `TS`)
  - `backend/src/controllers/devices.rs` (substituição de `serde_json::json!({ ... })` por `DevicePresenterItem`)
  - `backend/src/controllers/vpn_peers.rs` (serialização tipada padronizada)
- **Ações Realizadas:**
  - A API HTTP de dispositivos e VPN agora projeta respostas canônicas fortemente tipadas em camelCase, exportáveis para TypeScript via `ts-rs`, garantindo total sincronia e segurança de tipos entre backend e frontend.

---

### 🟢 BE-06: Acoplamento do Protocolo MAC-Telnet no Módulo de Syslog — Concluído
- **Arquivos Refatorados:**
  - `backend/src/services/network_tools/mactelnet.rs` (movido para pasta correta de ferramentas de rede)
  - `backend/src/services/network_tools/mod.rs` (exportado)
  - `backend/src/services/syslog/provision.rs` (importação ajustada)
  - `backend/src/controllers/logs.rs` (importação ajustada)
  - `backend/src/services/syslog/mactelnet.rs` (removido)
- **Ações Realizadas:**
  - O protocolo MAC-Telnet foi completamente desacoplado do serviço de syslog, passando a residir no módulo de ferramentas de rede de camada 2 (`services::network_tools`).

---

## 3. Débitos Técnicos: Banco de Dados & Persistência

### 🟢 DB-01: Abstração de Busca Textual (SQLite FTS5 vs PostgreSQL) — Concluído
- **Arquivos Refatorados:**
  - `backend/migration/src/logs/m20260816_000001_device_logs_fts.rs`
  - `backend/src/services/syslog/repository.rs`
  - `backend/src/services/syslog/search.rs`
- **Ações Realizadas:**
  - O repositório de logs possui abstração dual: FTS5 no SQLite (produção padrão e dev) e fallback com operadores LIKE/busca textual para PostgreSQL, plenamente coberto por testes unitários e de integração.


---

### 🟢 DB-02: Versionamento e Prefixos de Migrations Descompassados
- **Arquivos Afetados:**
  - `backend/migration/src/m20220101_000001_users.rs` vs `backend/migration/src/m20260810_*`
- **Descrição do Problema:**
  - A primeira migration traz timestamp de 2022 (herança do scaffold inicial do Loco.rs), enquanto as demais seguem a cronologia do projeto em 2026.
- **Impacto / Risco:** Débito puramente cosmético/histórico, sem impacto operacional.
- **Recomendação:** Manter como está para não invalidar a tabela `seaql_migrations` em bancos já instalados.

---

## 4. Débitos Técnicos: DevOps, Docker & Infraestrutura

### 🟢 DO-01: Supervisão de Subprocessos no Container Único — Concluído
- **Arquivos Refatorados:**
  - `docker/wireguard-watcher.sh` (emite heartbeat em `/tmp/wireguard-watcher.heartbeat`)
  - `docker-compose.yml` (configurado healthcheck HTTP com intervalo e retentativas)
- **Ações Realizadas:**
  - Adicionado heartbeat atômico no ciclo do watcher do WireGuard e configurado healthcheck no compose para detecção proativa de falhas.

---

## 5. Roteiro e Plano de Ação Recomendado

```mermaid
flowchart TD
    subgraph Fase 1 - Quick Wins
        A1[Remover código morto HelloWorld.vue e assets] --> A2[Unificar useCrudResource em users.ts]
        A2 --> A3[Renomear arquivos de teste phase*.rs]
        A3 --> A4[Configurar manualChunks no Vite]
    end

    subgraph Fase 2 - Componentes e DRY
        B1[Criar ResourceUsageWidget unificado] --> B2[Substituir SVG manual por BaseMetricChart]
        B2 --> B3[Unificar módulo CIDR no backend em shared/cidr]
        B3 --> B4[Configurar Vitest no frontend]
    end

    subgraph Fase 3 - Modularização Estrutural
        C1[Decompor DeviceDetailPage em abas separadas] --> C2[Modularizar MonitorFormDialog e MonitorDetailView]
        C2 --> C3[Decompor scheduler_run em executores especialistas]
    end

    Fase 1 --> Fase 2 --> Fase 3
```

### Detalhamento das Etapas

1. **Fase 1: Quick Wins & Higiene (Esforço Baixo / Retorno Imediato)**
   - Excluir `HelloWorld.vue` e imagens órfãs em `frontend/src/assets/`.
   - Refatorar `frontend/src/stores/users.ts` para adotar `useCrudResource`.
   - Renomear testes de integração (`phase2_phase3.rs`, `phase6_phase7.rs`, etc.) para nomes baseados em domínio.
   - Ajustar `frontend/vite.config.ts` para separar chunks de vendor (`vuetify`, `vue`).

2. **Fase 2: Unificação de Componentes & DRY (Esforço Médio / Alto Ganho de Qualidade)**
   - Unificar `CpuUsageWidget.vue` e `RamUsageWidget.vue` em `ResourceUsageWidget.vue`.
   - Refatorar widgets para consumirem `BaseMetricChart.vue`.
   - Unificar parsing de CIDR do backend em `services::shared::cidr`.
   - Instalar `vitest` no frontend e adicionar script `"test"` no `package.json`.

3. **Fase 3: Desacoplamento de Monólitos (Esforço Estrutural / Longo Prazo)**
   - Fatiar `DeviceDetailPage.vue` em subcomponentes por aba em `components/devices/tabs/`.
   - Fatiar `MonitorFormDialog.vue` em subformulários por tipo de monitor.
   - Modularizar a task `scheduler_run.rs` em executores dedicados.
