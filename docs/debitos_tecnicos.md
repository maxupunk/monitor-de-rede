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

### 🟡 BE-01: Duplicação de Algoritmo de Cálculo e Parsing de CIDR
- **Arquivos Afetados:**
  - `backend/src/services/discovery/cidr_range.rs` (302 linhas, 10.9 KB)
  - `backend/src/services/vpn/cidr.rs` (165 linhas, 5.8 KB)
- **Descrição do Problema:**
  - Em `services/discovery/cidr_range.rs`, o parsing de endereços IPv4 é feito manualmente via split de strings (`part in ip.split('.')`), contagem de octetos e aritmética manual de bits (`to_number`), com uma estrutura `CidrRange` que armazena `network_address: String`.
  - Em `services/vpn/cidr.rs`, a mesma operação é resolvida de forma idiomática utilizando os tipos da biblioteca padrão `std::net::Ipv4Addr`, com `CidrRange` contendo campos fortemente tipados (`network_address: Ipv4Addr`, `broadcast_address: Ipv4Addr`, etc.).
- **Impacto / Risco:** Dois parsers de rede paralelos no mesmo backend Rust. O parser manual do discovery é mais propenso a bugs de borda e não aproveita a validação nativa de `Ipv4Addr`.
- **Recomendação de Refatoração:**
  - Mover o cálculo de CIDR para um módulo compartilhado único (`backend/src/services/shared/cidr.rs` ou `backend/src/services/network_tools/cidr.rs`), unificando o uso em torno de `std::net::Ipv4Addr`.

---

### 🟡 BE-02: Nomenclatura Histórica de Testes de Integração
- **Arquivos Afetados:**
  - `backend/tests/requests/phase2_phase3.rs` (30 KB)
  - `backend/tests/requests/phase6_phase7.rs` (30 KB)
  - `backend/tests/requests/phase8.rs` (26 KB)
  - `backend/tests/requests/phase9.rs` (28 KB)
  - `backend/tests/requests/auth.rs` (L10: `// TODO: see how to dedup / extract this to app-local test utils`)
- **Descrição do Problema:**
  - Diversos arquivos de teste de integração ainda refletem os nomes das fases do roadmap de migração de AdonisJS para Rust, em vez de refletirem os domínios da aplicação (ex.: `phase2_phase3.rs` testa CRUD de dispositivos e configurações; `phase6_phase7.rs` testa scheduler, probes e alertas).
  - Em `auth.rs:10`, há um TODO explícito indicando necessidade de extração de utilitários comuns de teste para autenticação e seed de usuários.
- **Impacto / Risco:** Dificulta a localização de testes para novos desenvolvedores e agentes IA, além de gerar retrabalho por duplicação de setup de dados em testes.
- **Recomendação de Refatoração:**
  1. Renomear os arquivos para refletir os domínios:
     - `phase2_phase3.rs` ➔ `devices_monitors_crud.rs`
     - `phase6_phase7.rs` ➔ `scheduler_probes_lifecycle.rs`
     - `phase8.rs` ➔ `vpn_orchestration.rs`
     - `phase9.rs` ➔ `snmp_collection_integration.rs`
  2. Extrair helpers comuns de autenticação e contexto de requisição para `tests/requests/prepare_data.rs`.

---

### 🟡 BE-03: Geração de Configurações e Scripts VPN por Concatenação de Strings
- **Arquivos Afetados:**
  - `backend/src/services/vpn/profiles/mikrotik.rs`
  - `backend/src/services/vpn/profiles/openwrt.rs`
  - `backend/src/services/vpn/profiles/variants.rs`
  - `backend/src/services/vpn/profiles/wg_conf.rs`
- **Descrição do Problema:**
  - A geração de scripts de configuração para RouterOS (MikroTik), OpenWrt, Windows, Linux e Android é feita utilizando concatenações manuais extensas de strings com a macro `format!`.
- **Impacto / Risco:** Fragilidade na formatação de comandos de terminal, dificuldade de leitura/revisão dos templates e risco de erros de escape em parâmetros de usuário.
- **Recomendação de Refatoração:**
  - Adotar uma engine de templates leve em tempo de compilação ou templates desacoplados com validação de sintaxe.

---

### 🟡 BE-04: Complexidade Ciclomática e Responsabilidades Excessivas no `scheduler_run.rs`
- **Arquivos Afetados:**
  - `backend/src/tasks/scheduler_run.rs` (700+ linhas, 24.3 KB)
- **Descrição do Problema:**
  - A task `scheduler_run` concentra o ciclo de agendamento completo: controle de concorrência por lock de processo, agrupamento de monitores por alvo/dispositivo, cálculo dinâmico de timeouts, despacho concorrente de pings, execução de coletas SNMP locais e distribuição para probes remotos, persistência de resultados, disparo do pipeline de eventos e avaliação de recuperação de alarmes.
- **Impacto / Risco:** Dificuldade para testar fluxos de execução isolados sem acionar o ciclo completo de agendador e banco de dados.
- **Recomendação de Refatoração:**
  - Decompor o ciclo em pequenos serviços executores (`PingBatchExecutor`, `SnmpBatchExecutor`, `ProbeDispatchExecutor`, `AlertEvaluationStep`).

---

### 🟡 BE-05: Inconsistência na Serialização da API (DTOs vs `serde_json::json!`)
- **Arquivos Afetados:**
  - `backend/src/controllers/devices.rs`
  - `backend/src/controllers/monitors.rs`
  - `backend/src/controllers/discovery.rs`
  - `backend/src/dtos/`
- **Descrição do Problema:**
  - Enquanto rotas como Logs e VPN usam DTOs explícitos anotados com `#[derive(TS)]` (`ts-rs`) para geração automática de tipos no frontend, controladores de dispositivos e monitores realizam projeções manuais construindo `serde_json::json!({ ... })`.
- **Impacto / Risco:** Quebra o fluxo automático de verificação de tipos (`ts-rs`) no frontend, permitindo que alterações em campos do backend passem sem acusar erro de compilação no `vue-tsc`.
- **Recomendação de Refatoração:**
  - Substituir a montagem manual de JSON por DTOs com derive de `Serialize` e `ts_rs::TS`.

---

### 🟢 BE-06: Acoplamento do Protocolo MAC-Telnet no Módulo de Syslog
- **Arquivos Afetados:**
  - `backend/src/services/syslog/mactelnet.rs` (26.1 KB)
- **Descrição do Problema:**
  - O parser e cliente do protocolo proprietário MikroTik MAC-Telnet reside dentro da pasta `services/syslog/`. Embora o provisionamento de syslog em dispositivos MikroTik utilize essa camada de transporte, MAC-Telnet é um protocolo de rede L2 independente de syslog.
- **Impacto / Risco:** Violação do princípio de responsabilidade única (Single Responsibility Principle) e acoplamento desnecessário na estrutura de pastas.
- **Recomendação de Refatoração:**
  - Mover `mactelnet.rs` para `backend/src/services/network_tools/mactelnet.rs` e apenas importá-lo no serviço de provisionamento de syslog.

---

## 3. Débitos Técnicos: Banco de Dados & Persistência

### 🟡 DB-01: Abstração de Busca Textual (SQLite FTS5 vs PostgreSQL)
- **Arquivos Afetados:**
  - `backend/migration/src/logs/m20260816_000001_device_logs_fts.rs`
  - `backend/src/services/syslog/repository.rs`
- **Descrição do Problema:**
  - A tabela de índice de busca textual dos logs utiliza a extensão virtual `FTS5` nativa do SQLite.
  - Para instalações de grande porte que optam por PostgreSQL via `DATABASE_URL`, o mecanismo de Full-Text Search necessita de tratamento diferenciado (`tsvector` e índice `GIN`).
- **Impacto / Risco:** Risco de incompatibilidade em deploys PostgreSQL corporativos se o fallback não estiver devidamente coberto por testes em ambos os dialetos.
- **Recomendação de Refatoração:**
  - Garantir branch de migração condicional e testes automatizados de busca de logs contra instâncias SQLite e PostgreSQL.

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

### 🟡 DO-01: Supervisão de Subprocessos no Container Único
- **Arquivos Afetados:**
  - `docker/entrypoint.sh`
  - `docker/wireguard-watcher.sh`
  - `Dockerfile`
  - `docker-compose.yml`
- **Descrição do Problema:**
  - O container executa em segundo plano múltiplos processos sob um mesmo ciclo de vida (API Loco.rs, syslog receiver UDP, WireGuard watcher root).
  - O `healthcheck` do Docker Compose checa a resposta HTTP da API na porta 3333. Se o script watcher do WireGuard ou o receiver de Syslog abortarem silenciosamente, o container permanece em estado `healthy`.
- **Impacto / Risco:** Falhas parciais de serviço (ex.: VPN para de sincronizar ou logs param de ser recebidos) sem alerta imediato no orquestrador de containers.
- **Recomendação de Refatoração:**
  - Incluir checagens de processo filho ou arquivos de heartbeat (`/tmp/wg_watcher.heartbeat`) dentro do script de healthcheck do Docker.

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
