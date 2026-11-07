# Roadmap — Arquitetura modular (Áreas do produto + reconstrução em crates)

> **Decisão:** [ADR 017](adr/017-arquitetura-modular.md) — leia antes; este
> arquivo não repete o porquê, só o **como**, a **ordem** e o **aceite**.
> **Débitos atacados:** DB-02 e ARQ-01 a ARQ-06 em
> [`debitos_tecnicos.md`](debitos_tecnicos.md#7-débitos-de-arquitetura-modular-adr-017).
> **Branch da proposta:** `feat/arquitetura-modular`.
> **Última revisão:** 2026-11-05.

---

## 1. Objetivo

Duas entregas que se apoiam:

1. **Áreas do produto (Núcleo, Rede, Servidores, VPN)** — cada instalação vê e
   roda só o que usa. É o ganho que o usuário sente.
2. **Reconstrução no lugar, em crates** — o compilador passa a garantir que o
   `core` não depende de área e que áreas não dependem entre si. É o ganho que
   impede o sistema de voltar ao estado atual.

## 2. Premissas

- **Sem produção.** O sistema nunca foi instalado de verdade. Mudança de
  esquema **edita a migration existente** e recria o banco de desenvolvimento:
  nada de migration de compatibilidade, initializer de upgrade, estado `auto`
  ou *shim* de rota, campo ou tópico antigo. A regra se inverte na primeira
  instalação real — e isso será anunciado.
- **Sem reescrita.** O código atual é a especificação; ~2.400 testes são o
  contrato. Código **muda de lugar**, não de comportamento, salvo quando a fase
  diz o contrário.
- **Sem branch de longa duração.** A branch `feat/arquitetura-modular` carrega
  este plano. Cada fase sai numa branch curta a partir da `main` e volta para a
  `main` com tudo verde.

## 3. Regras de trabalho durante a reconstrução

| Regra | Por quê |
| :--- | :--- |
| **Mover não se mistura com mudar.** Um PR de extração só move código e inverte dependências por trait; mudança de comportamento vai em PR próprio. | Diff de movimento é revisável; diff misto não. |
| **Os testes vão junto com o código.** Teste de requisição que cruza áreas fica no `app`. | A suíte continua sendo a especificação de cada crate. |
| **Crate de área criado = código novo da área entra nele**, nunca no `legacy`. | O `legacy` só encolhe. |
| **Toda peça nova declara a área** (menu, rota, widget, ferramenta da IA, modelo de regra, modelo de tarefa, aba de configuração, etapa do onboarding). | ADR 017: `match` sem `_ =>`. |
| **Nenhuma dependência nova fora da regra.** O teste de arquitetura (Fase 1) recusa. | A lista de exceções só diminui. |
| **Cada fase atualiza a documentação que descreve o que mudou** (`arquitetura.md`, `AGENTS.md`, este arquivo). | Documento desatualizado é pior que ausente. |
| Validação: matriz do [`roadmap.md` §7](roadmap.md#7-matriz-obrigatória-de-validação), nos dois bancos. | AGENTS §1 e §3. |

## 4. Ordem das fases

```text
Fase 0  Navegação limpa ──────────────────────────────────────────────┐ (independente)
Fase 1  Esqueleto do workspace + base de dados                         │
   └─▶ Fase 2  Áreas no backend                                        │
          ├─▶ Fase 3  Frontend por áreas ◀────────────────────────────┘
          ├─▶ Fase 4  Onboarding por intenção
          └─▶ Fase 5  Backend respeita as áreas
                 └─▶ Fase 6  Piloto: crate `servers`  ── portão de decisão
                        └─▶ Fase 7 `vpn` ─▶ Fase 8 `assistant` ─▶ Fase 9 `network`
                                                                └─▶ Fase 10 Encerramento
```

As Fases 0–5 entregam o produto modular. As Fases 6–10 entregam a engenharia
modular. **A Fase 6 é um portão**: mede o custo real de uma extração antes de
comprometer as outras.

---

## 5. Fases

### Fase 0 — Navegação limpa (só frontend)

- [x] 🟢 Concluído — **Menu como registro e reorganizado por assunto**
  - **Débitos:** ARQ-01 (parcial), ARQ-03 (código morto)
  - **Esforço:** Pequeno
  - **Arquivos:** `frontend/src/layouts/DefaultLayout.vue`, novo `frontend/src/navigation.ts`, `frontend/src/stores/events.ts`
  - **Entregas:**
    - [x] `navigation.ts`: função pura que monta as seções a partir de entradas declarativas, já com `area` em cada uma (ainda sem efeito); o layout só renderiza.
    - [x] Docker (7 subitens) vira **1** entrada, com as abas que já existem em `DockerLayout.vue`; VPN (2 subitens) vira 1 entrada com abas.
    - [x] Docker e "Servidores remotos" passam a uma seção **Servidores**; "Sondas de rede" vai para **Rede**; o grupo "Coleta remota" deixa de existir.
    - [x] "Assistente IA" sai de Diagnóstico (já está na barra superior); "Benchmark dos modelos" vai para Configurações › IA.
    - [x] Remover o segundo `case 'device:updated'` de `events.ts`.
  - **Aceite:**
    - [x] Menu cai de ~36 para ~26 entradas.
    - [x] Testes Vitest do montador: papel, plugin-apps, grupo aberto pela rota.
    - [x] Nenhuma URL quebra.
  - **Entregue:** `frontend/src/navigation.ts` (`buildNavSections`, `groupOfPath`, ações nomeadas que o layout executa); `pages/vpn/VpnLayout.vue` com as abas Servidor e Dispositivos; 26 entradas para admin sem plugin-apps nem PWA; o filtro por `enabledAreas` já existe e é testado, sem fonte de dados até a Fase 3. Sites ficou na área Rede (na Núcleo, quem usa só Servidores veria uma seção "Rede" só com Sites). Teste: `tests/utils/navigation.spec.ts`.

### Fase 1 — Esqueleto do workspace e base de dados

- [x] 🟢 Concluído — **Workspace Cargo com o código atual num crate transitório**
  - **Débitos:** ARQ-06, ARQ-02 (congelamento)
  - **Esforço:** Médio
  - **Arquivos:** `backend/Cargo.toml`, novo `backend/crates/{app,legacy,core,agent-protocol,agent}/`, `Dockerfile`, `.github/workflows/ci.yaml`
  - **Entregas:**
    - [x] `backend/` vira raiz do workspace; o crate atual vira `crates/legacy`; os binários (`main.rs`, `agent.rs`) vão para `crates/app` e `crates/agent`. — *ajustado:* o pacote raiz fez o papel do `legacy` e do `app` (mover quebraria config do Loco, bindings e banco de teste); os binários ficaram em `src/bin/`, e o pacote raiz foi esvaziado no `crates/nucleo` na Fase 10
    - [x] `crates/agent-protocol`: `services/agents/protocol.rs` extraído (contrato pequeno e já fechado — `PROTOCOL_VERSION` inalterado). — *ajustado:* saiu na Fase 6, quando os tipos de Docker, monitoramento, vazão e plugins que o contrato carrega ganharam casa
    - [x] `crates/core` nasce com os **pontos de extensão** do ADR 017 (`MonitorChecker`, `Classifier`, `WidgetSpec`, `AgentCommandHandler`, `AlertDataset`, `AreaPolicy`) e os registros que o `app` monta. O `legacy` passa a implementá-los onde hoje há `match` fixo — sem mudar comportamento. — *ajustado:* cada ponto nasceu na fase que o usa (`AreaPolicy` na 2, `MonitorChecker`/`AgentCommandHandler` na 6 e na 9, `Classifier` na 8); `WidgetSpec` e `AlertDataset` não foram criados, por não haver `match` nem dependência a inverter (o `AlertDataset` nasceu depois da Fase 10, com o `vpn_peer` — ver Fase 7)
    - [x] `backend/tests/architecture.rs`: confere os `use crate::services::…` contra a matriz permitida; a lista de exceções começa com A1–A5 e **só pode diminuir**. — *ajustado:* mora em `tests/conventions/architecture.rs` e congelou 37 exceções (A1–A5 entre elas), zeradas na Fase 10
    - [x] Um `CARGO_TARGET_DIR` único para o workspace (AGENTS, limpeza). — *ajustado:* o workspace já compartilha `backend/target/`; não foi preciso fixar a variável
  - **Aceite:**
    - [x] Os quatro comandos do backend passam de `backend/`, sem nenhum teste alterado além de caminhos.
    - [x] Imagem Docker e binário do agente gerados como antes. — *conferido em 2026-11-05 (ver nota de validação)*
    - [x] Teste de arquitetura falha ao introduzir de propósito uma dependência nova proibida. — *conferido em 2026-11-05 (ver nota de validação)*
  - **Entregue:** `backend/` é a raiz do workspace e o **pacote raiz é o crate transitório** (e o ponto de montagem) — em vez de `crates/legacy` + `crates/app`: mover o pacote quebraria os caminhos relativos a ele (config do Loco, bindings, fixtures, banco de teste) sem ganho, e o papel é o mesmo. `crates/core` (`netmonitor-core`) recebeu `models`, `shared` e `events` — o conjunto fechado medido pelo grafo de imports —, com reexportação dos caminhos antigos. Versões comuns em `[workspace.dependencies]`; `TS_RS_EXPORT_DIR` fixo em `.cargo/config.toml`. `tests/conventions/architecture.rs` congela 37 exceções (o detector lê `use` agrupado e caminho inline; a lista só diminui). **Ajustes medidos:** o `agent-protocol` sai na Fase 6 — o contrato carrega tipos de Docker, monitoramento, vazão, plugins e telemetria, que só ganham casa quando o crate da área existe; e cada ponto de extensão nasce na fase que o usa (`AreaPolicy` na 2, `MonitorChecker`/`AgentCommandHandler` na 6, `Classifier` na 8), sem abstração especulativa.

- [x] 🟢 Concluído — **Base de dados consolidada**
  - **Débitos:** DB-02
  - **Esforço:** Pequeno
  - **Arquivos:** `backend/migration/`, `src/models/tables.rs`, `src/app.rs`
  - **Entregas:**
    - [x] As 64 migrations viram uma **base** na ordem do `CREATION_ORDER` (e a do banco de logs, a dela). A partir daqui, mudança de esquema edita a base. — *ajustado:* em vez de um arquivo só, uma migration de criação por tabela com os `ALTER` dobrados nela (64 → 37 no principal, 4 → 3 no de logs)
    - [x] Remover `migration::purge_removed_migrations` e a chamada em `after_context`.
  - **Aceite:**
    - [x] Banco recriado do zero em SQLite e PostgreSQL com o mesmo esquema de antes (comparação do esquema exportado).
    - [x] Entidades regeneradas **contra o PostgreSQL** sem diff. — *ajustado:* conferido pelo esquema exportado do PostgreSQL 17 (colunas, tipos, nulidade, padrões, índices, FKs), idêntico salvo um nome de FK e a posição ordinal — nada que mude as entidades
  - **Entregue:** 64 → 37 migrations no banco principal e 4 → 3 no de logs: uma migration de criação por tabela, com todo `ALTER` posterior dobrado nela (ordem de colunas preservada); as só de dados e as de criar-e-apagar foram removidas. Esquema comparado antes/depois em SQLite e PostgreSQL 17 (colunas, tipos, nulidade, padrões, índices, FKs): idêntico, salvo o nome da FK `database_connections.via_probe_id` (agora pelo helper `fk(...)`) e a posição ordinal no Postgres de `storage_destinations` (lacunas de coluna apagada). `purge_removed_migrations` removido.

### Fase 2 — Áreas no backend

- [x] 🟢 Concluído — **Serviço de áreas no `core`**
  - **Débitos:** ARQ-01
  - **Esforço:** Médio
  - **Arquivos:** `crates/core/src/areas/`, controller fino em `crates/app`
  - **Entregas:**
    - [x] `Area`, `AreaState` (ts-rs) e uma `AreaPolicy` por área com `evidence`, `consequences` e `can_disable`. — *ajustado:* o estado saiu como `AreasSnapshot`/`EnabledAreas`, e o `can_disable` virou `blocker` (devolve o motivo da recusa)
    - [x] Estado `on`/`off` em `system_settings`, chave `areas`. Padrão de banco novo: **tudo `on`**.
    - [x] `GET /api/areas`; `PUT /api/areas/{area}` só admin, auditado.
    - [x] Evento `areas:updated` na mudança e no snapshot ao conectar.
    - [x] VPN: `can_disable` recusa com servidor habilitado.
  - **Aceite:**
    - [x] Testes de evidência em SQLite em memória, incluindo "o agente local não é evidência de Servidores" e "o dispositivo do sistema não é evidência de Rede". — *ajustado:* os dois casos ficaram num teste de requisição (`evidencia_de_rede_nao_conta_o_que_o_sistema_cria_sozinho`, banco de teste SQLite); o `core` tem os testes do estado em memória
    - [x] Requisição: viewer recebe 403 no `PUT`; `off` da VPN recusado com servidor ativo; mudança registrada na auditoria.
    - [x] SQLite e PostgreSQL. — *conferido em 2026-11-05 (ver nota de validação)*
  - **Entregue:** `netmonitor_core::areas` (`Area`, `AreaSet`, `EnabledAreas`, `AreaPolicy`, `AreaService`, `AreasSnapshot`, `AreaImpact`); políticas em `services/area_policies/` (vão para o crate de cada área nas Fases 6–9); `controllers/areas.rs` com `GET /api/areas`, `GET /api/areas/{area}/impact` e `PUT /api/areas/{area}` (admin, auditado, publicado); `AreasInitializer` publica o snapshot no boot. `require_admin` virou um só (`auth_guard`), no lugar de duas cópias. Testes: `crates/core/src/areas/state.rs`, `tests/requests/areas.rs`.

- [x] 🟢 Concluído — **Declaração exaustiva de área e catálogo único de widgets**
  - **Débitos:** ARQ-04
  - **Esforço:** Médio
  - **Arquivos:** `services/ai/harness/tools/`, `services/dashboard/catalog.rs`, `services/alerts/catalog/`, catálogo de tarefas agendadas, `frontend/src/stores/dashboard.ts`
  - **Entregas:**
    - [x] `AiToolHandler::area()`, `DashboardWidgetType::area()` e `area` em cada modelo dos catálogos de alerta e de tarefa agendada — `match` sem `_ =>`. — *ajustado:* o modelo de alerta tira a área de `AlertCategory::area()` (enum no lugar de texto); a tarefa agendada, da área da ferramenta que ela chama
    - [x] O frontend passa a ler o catálogo de widgets pelo binding `ts-rs`; `CARD_TEMPLATES` e a lista duplicada saem de `stores/dashboard.ts`.
  - **Aceite:**
    - [x] Peça nova sem área não compila (provado por teste de compilação ou `match` exaustivo).
    - [x] Nenhuma lista de tipos de widget no frontend.
  - **Entregue:** `AiToolHandler::area()` obrigatório (60 ferramentas), `DashboardWidgetType::area()` e `AlertCategory` (enum no lugar de texto) com `area()` — todos `match` sem `_ =>`; `GET /api/dashboard/catalog` (`DashboardCatalog`), lido pelo frontend no lugar de `CARD_TEMPLATES`/`DEFAULT_WIDGETS`.

### Fase 3 — Frontend por áreas

- [x] 🟢 Concluído — **Registro de áreas na SPA**
  - **Débitos:** ARQ-01, ARQ-03, ARQ-04
  - **Esforço:** Médio
  - **Arquivos:** novo `frontend/src/areas/{core,network,servers,vpn}/index.ts`, `navigation.ts`, `router/index.ts`, `stores/events.ts`, `stores/dashboard.ts`, `pages/DashboardPage.vue`, `pages/SettingsPage.vue`
  - **Entregas:**
    - [x] `defineArea({ id, routes, nav, widgets, sse, settingsTabs, onboardingSteps })` e o registro em `areas/index.ts`. — *ajustado:* o contrato ficou com `{ id, routes, nav, widgets, sse }`; as abas de configuração ainda não têm conteúdo por área, e as etapas do assistente têm registro próprio por área (`components/setup/steps.ts`)
    - [x] `stores/areas.ts` alimentada **só pelo SSE** (`areas:updated` e snapshot).
    - [x] Menu filtrado; `meta.area` nas rotas; um guard; `AreaOffPage.vue` com "Ligar" para admin e o motivo para os demais.
    - [x] Card **Áreas do sistema** em Configurações › Geral, no padrão do `SetupAppsStep` (avatar colorido, frase, `v-switch` verde), com as consequências vindas da API num `ConfirmDialog`.
    - [x] Barramento SSE como **registro de handlers por prefixo**; o `switch` de `events.ts` some.
    - [x] `DashboardPage` sem `useDockerStore`: o `docker_summary` carrega a própria store.
    - [x] Layout do dashboard: widget de área desligada fica salvo e não é renderizado; `parseSavedList` deixa de reintroduzir padrão de área desligada.
    - [x] Abas do dispositivo exigem capacidade **e** área (VPN, Wi-Fi).
  - **Aceite:**
    - [x] Teste Vitest: `areas:updated` muda o menu **sem nenhuma chamada HTTP** (AGENTS §9).
    - [x] Link profundo para `/docker` com Servidores `off` mostra a `AreaOffPage`.
    - [x] Layout salvo não perde widget ao desligar e religar.
    - [x] Teste: área desligada não registra handlers SSE.
  - **Entregue:** `frontend/src/areas/{core,network,servers,vpn}` com `defineArea({ id, routes, nav, widgets, sse })` e registro em `areas/index.ts`; `areas/realtime.ts` registra e desregistra os handlers SSE por área; `events.ts` virou barramento genérico (o `switch` e os imports de stores saíram); `router/areaGuard.ts` + `AreaOffPage.vue`; `AreasCard.vue` em Configurações › Geral; catálogo do dashboard vindo da API. `settingsTabs` e `onboardingSteps` não entraram no contrato: as abas de configuração ainda não têm conteúdo por área, e as etapas do assistente têm registro próprio por área (`components/setup/steps.ts`). Testes: `tests/areas/*.spec.ts`, `tests/stores/dashboard.spec.ts`.

### Fase 4 — Onboarding por intenção

- [x] 🟢 Concluído — **"O que você vai gerenciar?"**
  - **Débitos:** ARQ-01
  - **Esforço:** Médio
  - **Arquivos:** `frontend/src/components/setup/`, `InitialSetupDialog.vue`, `backend/src/services/onboarding.rs`
  - **Entregas:**
    - [x] Primeira etapa com Rede, Servidores e Docker, e VPN (múltipla escolha), gravando `on`/`off`; Assistente IA continua do núcleo.
    - [x] Caminho só Servidores: Endereços da central → agente local e comando para conectar outro servidor (reaproveita `AgentEnrollmentDialog`) → Notificações.
    - [x] Caminho Rede: as etapas atuais; a etapa "Aplicativos" só com Rede ligada.
    - [x] `OnboardingStatus` considera as áreas escolhidas.
    - [x] Sugestões por evidência, avisadas uma vez em cada direção (ADR 017).
  - **Aceite:**
    - [x] Caminho só Servidores com no máximo 3 etapas.
    - [x] O status não cobra rede de quem escolheu só Servidores.
    - [x] Teste de componente para os dois caminhos.
  - **Entregue:** `POST /api/settings/onboarding/intent` (admin; exige Rede ou Servidores) e `OnboardingStatus.intentChosen`; `components/setup/steps.ts` (registro de etapas por área), `SetupIntentStep.vue`, `SetupAgentsStep.vue`, `SetupNotificationsStep.vue` e as etapas de rede extraídas do `InitialSetupDialog.vue` (de ~1600 para ~600 linhas); `components/areas/AreaSuggestions.vue` (sugestões por evidência, uma vez por direção). Testes: `tests/components/InitialSetupDialog.spec.ts`, `tests/components/AreaSuggestions.spec.ts`, `tests/requests/areas.rs`.

### Fase 5 — Backend respeita as áreas

- [x] 🟢 Concluído — **IA, rotinas, catálogos e tempo real filtrados pela área**
  - **Débitos:** ARQ-01, ARQ-05
  - **Esforço:** Médio
  - **Arquivos:** `services/ai/{harness,decider,mentions.rs}`, `services/scheduled_tasks/runner.rs`, `services/agents/background.rs`, ciclo do scheduler, `services/wifi/`, `services/link_tests/`, catálogo de alertas
  - **Entregas:**
    - [x] `ToolRegistry` e `UnattendedTools` filtram pelas áreas ligadas: grupo do `load_tools`, intenções do Decisor, linhas do prompt e menções se ajustam sozinhos.
    - [x] Tarefa agendada com ferramenta de área desligada fica `skipped`, com o motivo.
    - [x] Rede `off`: varredura periódica, testes de enlace agendados e watch de Wi-Fi pausados (o ciclo lê o estado do banco a cada volta, como `auto_discovery_enabled`, porque o scheduler pode rodar em outro processo).
    - [x] Servidores `off`: `SetLive{0}` aos agentes e streams de log fechados. Com Servidores `on`, o snapshot ao vivo passa a depender de quem usa dado Docker, não de qualquer assinante. — *ajustado:* os streams de log fecham pela SPA (a rota cai na `AreaOffPage` e o handler `docker:log` é desregistrado), não por corte no backend
    - [x] Modelos de regra da área somem do catálogo "Criar regra".
  - **Aceite:**
    - [x] Com Servidores `off`: nenhuma ferramenta `docker_*` no registro e nenhum `docker` no enum do `load_tools`; `SetLive{0}` enviado.
    - [x] **Monitores de container continuam rodando** com Servidores `off` (teste).
    - [x] Rotinas de Rede pausadas com `off` e retomadas com `on` (teste).
  - **Entregue:** `ToolPolicy.areas` (+ `with_areas`, que também desliga as ações em container) aplicado uma vez em `run_agent_loop` e na confirmação; `UnattendedTools::for_areas`; tarefas de área desligada viram `skipped` com o motivo (`scheduled_tasks/areas.rs`); menções `@` sem Docker; varredura periódica, enlaces agendados e vigia de Wi-Fi pausados com Rede `off`; **interesse por conexão SSE** (`?interest=docker`, `events::Interests`, base comum `ConnectionCounter` com a presença): o laço de `SetLive` só pede snapshot com alguém olhando o Docker **e** Servidores ligada (ARQ-05). Testes: `tools/mod.rs` (sem ferramenta docker), `tests/requests/areas.rs` (monitor de container continua; varredura pausa e volta).

### Fase 6 — Piloto: crate `servers` (portão de decisão)

- [x] 🟢 Concluído — **Extrair a área Servidores**
  - **Débitos:** ARQ-02 (A1, parte de A3–A5), ARQ-06
  - **Esforço:** Médio
  - **Arquivos:** `services/docker/`, parte de contêiner de `services/telemetry/`, `services/databases/`, `monitoring/checkers/{container,host_resources}.rs`, `ai/harness/tools/docker.rs`, controllers e views correspondentes, `frontend/src/areas/servers/`
  - **Entregas:**
    - [x] `crates/servers` com Docker, métricas de container, backup de bancos (ADR 013), ferramentas `docker_*`, widget `docker_summary` e os handlers de comando Docker/compose/volume do lado central. — *ajustado:* as ferramentas `docker_*` chegaram ao crate na Fase 8, com o `AiToolHandler` no `core`; o `docker_summary` continua entrada de catálogo com `area()`, sem código no crate
    - [x] `ContainerChecker` registrado como `MonitorChecker`; `monitoring/runner.rs` perde o `match` e o import de Docker (**A1 sai da lista de exceções**).
    - [x] Métricas de host ficam no `core` (servidor é dispositivo). — *ajustado:* ficaram no Núcleo — hoje o checker `host_resources` mora em `crates/nucleo`
    - [x] Tabelas de servidores na base do crate; FK só para o `core`. — *ajustado:* as migrations continuam no crate comum `migration`; as tabelas de servidores só têm FK para o `core`
  - **Aceite:**
    - [x] `crates/core` compila sem `servers`; `servers` não depende de nenhuma outra área.
    - [x] Suíte inteira verde nos dois bancos.
    - [x] **Portão:** registrar no ADR 017 o custo medido (tempo, linhas movidas, testes ajustados, ganho de compilação incremental) e decidir seguir, ajustar ou parar nas Fases 7–10.
  - **Entregue:** três crates novos. `crates/agent-protocol` (`agent_protocol`, sem dependência do workspace) leva o contrato (`protocol.rs`, `PROTOCOL_VERSION` inalterado, `policy.rs`, `tunnel.rs`) e os tipos que trafegam nele (DTOs Docker, `CheckResult`, `ThroughputSpec`, `DeviceIoCall`, tarefas de sonda, rollup de telemetria); as conversões com regra de negócio saíram do contrato (`AppError` ⇄ `RemoteError` no core, `ThroughputSpecExt` e `DriveRequest` ⇄ `Command` na vazão). O **transporte dos agentes** (`hub`, `session`, `bridge`, `HostKey`, identidade, rotas) foi para o `netmonitor-core` em vez de virar trait: só dependia do contrato e do `AppError`, e mover custou menos que abstrair chamadas, chunks e pontes. Também no core: `audit`, `storage`, `telemetry::{store, views}` e `monitoring` (`Checker`, `MonitorChecker`, `MonitorCheckers`). `crates/servers` (`netmonitor_servers`, depende só de `core` e `agent-protocol`): `docker/` (com o `AgentEngine`, o lado central dos comandos Docker/compose/volume), `databases/`, `ContainerMonitor` (registrado como `MonitorChecker`), `ServersPolicy`, controllers, DTOs e views. `src/registries.rs` monta políticas e checkers no ponto de montagem; `monitoring/runner.rs` perdeu o braço `container` e o import de Docker; `default-members` cobre o workspace inteiro (o CI roda `cargo test` sem `--workspace`). Exceções do teste de arquitetura: 37 → 34 (`monitoring → docker` — A1 —, `agents → docker`, `telemetry → docker`). **Ficou para depois, de propósito:** ferramentas `docker_*` da IA (o contrato `AiToolHandler` é do assistente — vão com a inversão da Fase 8); `WidgetSpec` (o `docker_summary` é uma entrada de catálogo sem lógica); `AgentCommandHandler` (nada no lado central precisou); migrations no crate `migration` (as tabelas de servidores só têm FK para o core). Portão registrado no ADR 017.

### Fase 7 — Crate `vpn`

- [x] 🟢 Concluído — **Extrair a área VPN**
  - **Débitos:** ARQ-06, ARQ-02 (parte)
  - **Esforço:** Pequeno
  - **Arquivos:** `services/vpn/`, `controllers/{vpn_servers,vpn_peers}.rs`, `frontend/src/areas/vpn/`
  - **Entregas:**
    - [x] `crates/vpn`.
    - [x] O contrato com o watcher (`<iface>.conf`/`<iface>.status` em `WG_CONFIG_DIR`) e o cofre de chaves em memória **inalterados** (AGENTS §7).
    - [x] O dataset de alerta `vpn_peer` registrado como `AlertDataset`. — *entregue depois da Fase 10:* o dataset e o observador dos túneis saíram do Núcleo para `crates/vpn/src/alerts/`
  - **Aceite:**
    - [x] `vpn` depende só de `core`.
    - [x] Snapshots `insta` dos artefatos sem diff.
    - [x] Testes `#[serial]` do cofre intactos.
  - **Entregue:** `crates/vpn` (`netmonitor_vpn`, depende **só** do `netmonitor-core`): servidor, peers, IPAM, chaves, `config_builder`/`config_writer`/`peer_status` (o contrato com o watcher — só mudaram os `use`), pré-voo, perfis de cliente e scripts, telemetria dos túneis, `access_control`, `VpnPolicy`, as rotas `/api/vpn/{server,peers}` e as views. Snapshots `insta` renomeados para o caminho do crate, com o corpo intacto; `services/vpn` virou reexportação do crate (IA, onboarding e scheduler até as Fases 8 e 10). **Núcleo primeiro:** foram para o `core` o cofre efêmero (`shared::secret_store`, byte a byte — o enrollment dos agentes e as senhas de sessão dos plugins também o usam), o enrollment e o cadastro dos agentes (`agents::{enrollment, registration}`; o `AgentService` delega), `devices::{adapters, system_device}`, `maintenance::resource_cleanup`, `monitoring::reachability` e `vpn_probe` (`probe_is_external`, `vpn_probe_name`). **Fica no pacote raiz, de propósito:** o registrador do `vpn-probe` (`services/probes/vpn_registrar.rs` — é cadastro de probe, usado no boot, na troca de banco e pelo `task vpn_probe_register`; o fallback `DEFAULT_VPN_PROBE_TOKEN` intacto) e o observador da transição dos túneis (`services/alerts/vpn_peer_watcher.rs`, junto do dataset `vpn_peer`, que o motor de alertas já tratava como um dataset qualquer — sem `match` nem import de VPN, então não houve `AlertDataset` a criar). O que a área pede ao resto da central (motor de alertas, apresentação de dispositivo, endereços da central) entra por um único trait da área, `netmonitor_vpn::central::CentralServices`, implementado em `src/registries.rs` e posto no `shared_store` pelo `after_context` (vale para o scheduler). Exceções do teste de arquitetura: **34 → 29** (`agents → vpn`, `devices → vpn`, `plugins → vpn`, `server_addresses → vpn`, `settings → vpn`); o detector conta `netmonitor_vpn::` como a área VPN. **Ficou para depois:** `ai → vpn` (ferramentas da IA, Fase 8), `monitoring → vpn` (o `maintenance_runner` chama a telemetria dos túneis — sai com um registro de rotinas por área) e `onboarding → vpn` (pré-voo e servidor no assistente inicial). **Depois da Fase 10:** o `AlertDataset` nasceu no `core` (`alerts::contracts`: `eventType`, fatos, mensagem e recuperação; `AlertEvaluationContext::of` monta o contexto com o alvo, `AlertTarget`) e o dataset `vpn_peer` e o observador dos túneis foram para `crates/vpn/src/alerts/`, entregando ao motor pelo `evaluate_alert` do `CentralServices` da VPN (o mesmo da Rede, no lugar de `evaluate_peer_states`) — o cliente Wi-Fi vigiado implementa o mesmo trait; o alias dos fatos passou a se chamar `AlertFacts`; eventos, mensagens, chaves de escopo e `last_connection_status` inalterados.

### Fase 8 — Crate `assistant`

- [x] 🟢 Concluído — **Extrair a IA e inverter o Decisor**
  - **Débitos:** ARQ-02 (A2, A3)
  - **Esforço:** Grande
  - **Arquivos:** `services/ai/`, `controllers/{ai,ai_conversations}.rs`, `initializers/ai.rs`
  - **Entregas:**
    - [x] `crates/assistant` com chat, harness, drivers, Decisor e proativo.
    - [x] `Classifier` no `core` com a heurística padrão; o `assistant` registra a implementação pelo Decisor. `devices`, `discovery`, `snmp` e `syslog` passam a depender só do trait (**A2 sai**).
    - [x] Ferramentas de cada domínio contribuídas pelo crate do domínio; o `assistant` não importa área (**A3 sai**). — *ajustado:* as da Rede vieram com o crate `network` na Fase 9; `scan_ports` e `playbook` ficaram no Núcleo (`services/ai_tools`), porque só usam primitivos dele
  - **Aceite:**
    - [x] Sem o `assistant` registrado, identificação de dispositivo e classificação de log funcionam pela heurística (teste).
    - [x] Benchmarks do Decisor e do chat sem regressão. — *conferido em 2026-11-05:* Decisor 93,3 e chat 96,1 ao vivo, iguais às rodadas oficiais (ver nota de validação)
  - **Entregue:** `crates/assistant` (`netmonitor_assistant`, depende **só** do `netmonitor-core`): chat e harness (laço do agente, aprovação, compactação, prompt, grounding), provedores (`drivers`, `ollama`, `openrouter`, `opencode`, `local_server`, `local_models`), Decisor, rotinas automáticas, marcações, conversas, pesquisa na web, `settings`, DTOs e as rotas `/api/ai` e `/api/ai/conversations` (o `AiInitializer` continua registrado no `app.rs`, só chama `proactive::spawn`). `services/ai`, `dtos/ai` e `controllers/{ai,ai_conversations}` viraram reexportação. **`Classifier` no `core`** (`netmonitor_core::classifier`): o trait com `min_confidence`, `identify_device`, `advise_interfaces` e `categorize_log`, a fila (`Lane`) e os dados que cruzam a fronteira (`DeviceFacts`, `IdentitySuggestion`, `DeciderSuggestion`, `LogPattern` e as categorias, `InterfaceCandidate` e o ranking); o padrão é a `HeuristicClassifier` (nenhum palpite — fica a heurística de quem chama) e o assistente registra o `DeciderClassifier` em `src/registries.rs`, instalado pelo `after_context` (vale para o scheduler). As perguntas e a leitura das respostas (`Decision`) continuam no assistente. **Contrato das ferramentas no `core`** (`netmonitor_core::ai`): `AiToolHandler`, `ToolKind`, `ToolGroup`/intenções, `ToolPolicy` (lê as configurações por um trait, `ToolSettings`), `ToolRegistry`/`UnattendedTools`, `ToolArgs`, risco, `AiChart`, `AiTool`, `AiAutomationMode`, a busca de dispositivo/monitor (`lookup`), as séries, a documentação que a IA consulta (`knowledge`), a fonte do `grep` (`GrepSource`) e a seção do estado da plataforma (`PlatformSection`). O catálogo, **na mesma ordem**, é montado em `src/registries.rs::ai_tool_catalog` e instalado com `install_catalog`: as `docker_*` e a fonte `docker` do `grep` vêm do crate `servers`, a seção `vpn` do `get_platform_status` do crate `vpn`, `get_topology`, a seção `discovery` e os plugins dos módulos da Rede (`services/{topology,discovery,plugins}/ai_tools.rs`, para a Fase 9), a `web_search` do assistente e o resto do núcleo (`services/ai_tools/`). **Núcleo primeiro:** foram para o `core` `devices::kinds`, `alerts::{contracts, feed}` (+ `PROBLEM_KIND`, `role_weight`), o contrato dos canais de notificação, `units::{round_two, duration_to_ms}`, o `if_type_label` do SNMP e o `authenticated_user`. O que o assistente pede ao resto da central (Web Push, canais de notificação, inibição dos alertas, plugins de dispositivo, Docker das marcações) entra por `netmonitor_assistant::central::CentralServices`, implementado em `src/registries.rs`. Exceções do teste de arquitetura: **29 → 18** (`ai → {discovery, docker, plugins, snmp, topology, vpn}` — **A3** —, `{devices, discovery, snmp, syslog} → ai` — **A2** — e `scheduled_tasks → ai`); o detector conta `netmonitor_assistant::` como o assistente. Teste de aceite: `sem_o_assistente_a_identificacao_segue_pela_heuristica` e `sem_o_assistente_o_log_segue_pela_heuristica` (o classificador retirado com `classifier::uninstall`; o Decisor configurado e no ar não é consultado). **Benchmarks:** o benchmark das decisões e o do chat (Laya-tuning) perguntam ao modelo a partir de uma cópia literal do contrato do NetMonitor — o resultado só muda se o contrato mudar. Conferido depois da Fase 10: `laya-tuning sync-check` dá "contrato idêntico ao código" tanto no código de antes da extração (`7864880`) quanto no atual (perguntas, rótulos, instruções, limites e confiança mínima do Decisor), e `backend/contracts/ai_chat.json` (prompt, núcleo, `load_tools`, grupos e schemas, agora gerado em `src/registries/ai_catalog_tests.rs`) é byte a byte o mesmo de antes e semanticamente igual ao do Laya-tuning. Pendência fora deste repositório: o `sync.py` do Laya-tuning lê os caminhos antigos (`backend/src/services/ai/...`) e precisa apontar para `crates/assistant` e `crates/core/src/{ai,classifier,devices}`. **Ficou para depois:** `plugins → ai` (a revisão de segurança do plugin chama o provedor do chat — pede um "completar texto" no `core`), as ferramentas de Rede para o crate `network` (Fase 9) e o nome `DeciderTemplateClassifier` do syslog (hoje delega ao `Classifier` do processo).

### Fase 9 — Crate `network`

- [x] 🟢 Concluído — **Extrair a área Rede, em quatro passos** (cada um um PR verde)
  - **Débitos:** ARQ-02 (A5), ARQ-06
  - **Esforço:** Grande
  - **Passos:**
    - [x] 1. Descoberta, fabricantes e varredura de portas. — *ajustado:* o 9.1 levou junto os serviços de SNMP e topologia, porque a descoberta chama o SNMP e o SNMP chama a topologia
    - [x] 2. SNMP e topologia (`SnmpChecker` como `MonitorChecker`). — *ajustado:* os serviços vieram no 9.1; o 9.2 fez o `SnmpMonitor` (`MonitorChecker`) e as rotas `/api/snmp` e `/api/topology`
    - [x] 3. Testes de enlace (ADR 015) e Wi-Fi. — *ajustado:* vieram por último (9.4), depois dos plugins, de que dependem
    - [x] 4. Plugins de dispositivo e espelho de pacotes (ADR 012); configuração de Syslog dos equipamentos. — *ajustado:* plugins e espelho no 9.3; a configuração do Syslog ficou no Núcleo, junto do receptor, e alcança o equipamento pelas sessões SSH/Telnet da Rede (`DeviceAccess`, Fase 10)
  - **Fica no `core`:** inventário (`sites`, `networks`, `devices`, `device_interfaces`), `probes`, `monitors` e os primitivos de `network_tools` usados pelos checkers do núcleo (ICMP, TCP, DNS) — são alvo de FK ou do motor de monitoramento.
  - **Aceite:**
    - [x] `network` depende só de `core` e `agent-protocol`.
    - [x] Handlers de `Discovery`/`DeviceIo`/vazão registrados como `AgentCommandHandler` (**A5 sai**).
    - [x] Política local do agente intocada.
  - **Entregue:** `crates/network` (`netmonitor_network`, depende **só** de `netmonitor-core` e do `agent-protocol`), em quatro commits verdes. **A ordem dos passos seguiu as dependências**, não a lista: a descoberta chama o SNMP (enriquecimento e coletores) e o SNMP chama a topologia, e o Wi-Fi e os testes de enlace dependem dos plugins — por isso o passo 1 levou SNMP e topologia junto e os plugins vieram antes do Wi-Fi e do enlace. **9.1** — descoberta, fabricantes (OUI), varredura de portas e os serviços de SNMP e topologia; `NetworkPolicy`; rotas `/api/discovery`, `/api/vendors`, `/api/port-scan`. **9.2** — o checker SNMP virou `SnmpMonitor` (`MonitorChecker`), registrado na central e no agente pelo mesmo `registries::monitor_checkers`: o `monitoring/runner.rs` perdeu o braço `snmp`; rotas `/api/snmp` e `/api/topology`. **9.3** — plugins de dispositivo (ADR 012: sandbox Rhai e `DeviceTransport` inalterados; plugin importado continua em quarentena; `reveal` intocado), espelho de pacotes, motores de vazão e o agente como origem do teste (`throughput::agent`); as sessões SSH/Telnet com o equipamento saíram do `syslog::provision` para `plugins::transport::shell`, de onde a ativação do log as usa. **9.4** — testes de enlace (ADR 015) e Wi-Fi (com o dataset de alerta `wifi_client`), rotas `/api/link-tests*` e `/api/wifi*`. **`AgentCommandHandler`** nasceu no `core` (`agents::command`) com os dois lados: no agente, o executor aplica a política local (`AGENT_ALLOW`, **intocada**) e entrega `Discovery`, `DeviceIo` e `Throughput` aos handlers da Rede (`agent_commands`); na central, o pump de `agents::background` entrega as varreduras pendentes pelo `AgentCommandDispatcher` da Rede (`discovery::remote::DiscoveryDispatch`). O enum continua fechado no `agent-protocol` e `PROTOCOL_VERSION` não mudou. **Núcleo primeiro:** foram para o `core` as preferências, os primitivos de `network_tools` dos checkers do núcleo (ICMP — sonda e cliente do processo —, TCP, UDP, vizinhos, NAT do container), `CheckDeps`, `device_status`, `execution_guard`, `link_speed`, `metrics_repository`, as séries e a coleta de saúde do sistema, os campos e o dataset de interface dos alertas (+ `format_seconds`), as plataformas (`devices::systems`), o papel do usuário (`roles`), o `access` e o teto/nomes de rede das ferramentas da IA, as chaves do palpite do Decisor, o alvo aceitável de equipamento (`shared::cidr`) e as redes aceitas pelo agente (`agents::targets`); o inventário, `probes` e `monitors` ficaram no núcleo. O que a Rede pede à central entra por `netmonitor_network::central::CentralServices` (motor de alertas, fechamento de alerta e regras do Wi-Fi vigiado, endereços da central, identificação de dispositivo, porta publicada da API e uma leitura da IA para a revisão de segurança do plugin — sai `plugins → ai` sem "completar texto" no `core`). As ferramentas da IA da Rede (topologia, seção `discovery`, plugins) vieram com os módulos. Testes que ligam a central a um agente de verdade foram para `tests/requests/{agent_commands,agent_throughput}.rs` (o servidor iperf3 de mentira atrás da feature `testing`); snapshots `insta` dos acessos dedicados renomeados com o corpo intacto. `contracts/ai_chat.json` e `frontend/src/bindings/` sem diff. Exceções do teste de arquitetura: **18 → 11** (`agent_runtime → {discovery, plugins}`, `agents → {discovery, plugins}`, `devices → discovery`, `network_tools → plugins`, `plugins → ai`); o detector conta `netmonitor_network::<módulo>` pelo módulo. **Ficou para depois, de propósito:** um registro de rotinas por área para o scheduler e a limpeza (`monitoring → {link_tests, snmp, wifi}`, `maintenance → link_tests` — o executor de grupos SNMP depende do `result_processor` do núcleo), o polling HTTP do probe pelo despachante (`probes → discovery`), as capacidades do dispositivo sem o Wi-Fi (`devices → wifi`), a ativação do log e as dicas do formulário do syslog (`syslog → {plugins, snmp}`, presas ao receptor do núcleo), o Docker do executor do agente como handler de Servidores (`agent_runtime → docker`) e as ferramentas `scan_ports`/`playbook` (área Rede) no `services/ai_tools` do núcleo, que só usam primitivos do núcleo.

### Fase 10 — Encerramento

- [x] 🟢 Concluído — **`legacy` vazio e documentação final**
  - **Esforço:** Pequeno
  - **Entregas:**
    - [x] O que restar do `legacy` vai para o `core`; o crate é removido. — *ajustado:* foi para `crates/nucleo`, não para o `core` (medido: mudar o `nucleo` custa 10 s, mudar o `core` 17 s e recompila as áreas); o pacote raiz ficou só como ponto de montagem
    - [x] Teste de arquitetura reduzido ao que o compilador não vê (ex.: toda área tem consumidor em menu, IA, widgets e rotinas).
    - [x] ADR 017 passa a **aceito e implementado**; ADR 018 decide manter o Loco.rs ou trocar por `axum` + `sea-orm` direto, com dados.
    - [x] `arquitetura.md` §4 descreve os crates como são; `AGENTS.md` e `backend/AGENTS.md` sem referência ao crate único.
  - **Aceite:**
    - [x] Lista de exceções do teste de arquitetura vazia.
    - [x] Nenhum `use` entre áreas no workspace.
  - **Entregue:** em três commits verdes. **10.1 — zero exceções.** Registro de rotinas no `core` (`routines`: `Routine`, `RetentionJob`, `AreaRoutines`, `cadence`) e grupos de monitores (`monitoring::MonitorGroup`, com o "testar agora"): a Rede contribui a coleta SNMP consolidada (`SnmpDeviceGroup`) e a sem monitor, a varredura agendada, o vigia do Wi-Fi, os enlaces agendados e a retenção deles; a VPN, a telemetria dos túneis. Área desligada pausa só o que já pausava (a própria rotina consulta a área); monitor nunca. O agente: o Docker virou `DockerCommands` (`AgentCommandHandler`) e `LocalHostDocker` (`agents::host::HostDocker`, no `core`) de Servidores, e o runtime recebe as peças montadas pelo binário (`AgentHost`) — `AGENT_ALLOW` intacto, antes dos handlers. `devices::access::DeviceAccess` no `core` (identidade SNMP, leitor de Wi-Fi, credencial salva, shell SSH/Telnet, ligar plugins no cadastro), implementado pela Rede; os tipos que cruzam (`CredentialView`, `SecretStorage`, `DedicatedAccess`, `SnmpSystemInfo`, prazos da sessão) foram para o `core` byte a byte. O probe por HTTP usa os despachantes (`pending_for_probe`, `receive`). Núcleo primeiro: IP público, rate limit e IP de origem, `VpnPeerResponse`, a cadência SNMP do dispositivo e o campo da triagem no `core`; `AreaPolicies` no `shared_store`. **10.2 — o crate transitório esvaziado.** Medido antes de decidir (build incremental do pacote raiz com uma função pública nova, `-j 2`): no `nucleo`, **10 s** (recompila ele e o ponto de montagem); no `core`, **17 s** (recompila `core`, as quatro áreas, o assistente e a montagem). Pôr o Núcleo no `core` faria toda mudança de serviço pagar o segundo preço e recompilar as áreas — por isso **`crates/nucleo`** (`netmonitor-nucleo`): serviços, controllers, DTOs, views e mailers do Núcleo (~64 mil linhas), dependendo só de `core` e `agent-protocol`, e nenhuma área depende dele. O cadastro de redes (que enfileira a varredura) foi para a Rede. O pacote raiz ficou com ~2,9 mil linhas: `app.rs`, `registries`, initializers, tarefas, binários e SPA. As reexportações de caminho antigo saíram — do pacote raiz e as de área dentro do Núcleo (`services::{docker, vpn, ai, snmp…}`, `dtos::ai`, `controllers::{ai, ai_conversations}`); os testes e a montagem usam os crates direto. O teste de arquitetura virou o que o compilador não vê: os manifestos seguem a regra, só `app.rs`/`registries`/`initializers` nomeiam área, e toda área tem consumidor no ponto de montagem e no frontend. **Achado:** o filtro padrão de log do Loco só libera o crate `backend` — o log dos crates do workspace sumia desde a Fase 6; corrigido com `WORKSPACE_CRATES` em `app.rs`. **10.3 — documentação:** ADR 017 aceito e implementado, [ADR 018](adr/018-loco-ou-axum.md) (manter o Loco.rs), `arquitetura.md` §4, `AGENTS.md`, ARQ-02 e ARQ-06 fechados. Suíte com o mesmo número de testes (2864 → 2866, os dois novos do teste de arquitetura), `contracts/ai_chat.json` e `frontend/src/bindings/` sem diff. **Ficou de fora, de propósito:** `WidgetSpec` (o `docker_summary` é dado de catálogo com `area()`, sem dependência); migrations por crate (as tabelas das áreas continuam no `migration` comum — só têm FK para o `core`).

### Fases 11–16 — Pendências decididas em entrevista (2026-11-06)

Decididas com o responsável pelo produto depois do encerramento. **Critério de
aceite comum a todas:** usabilidade e experiência do usuário conferidas na tela
(AGENTS §12: contraste, cores semânticas, nada de botão ativo cinza), SOLID
(cada peça com uma responsabilidade; extensão por registro, nunca por `if`
espalhado) e DRY (reaproveitar o filtro de áreas, os formatadores e os
componentes que já existem antes de criar outro). Ordem pela dependência:
as migrations por crate vêm antes das tabelas novas das Fases 15 e 16.

### Fase 11 — Pendências pequenas

- [x] 🟢 Concluído — **Fechar o que ficou solto**
  - **Entregas:**
    - [x] Gráfico de latência de 7 dias da IA (`chart_monitor_latency`) e linha de base dos alertas (`alerts/baseline.rs`) contam o trecho que o rollup ainda não agregou, pelo mesmo `rollup::unrolled_since` do uptime. — *ajustado:* o gráfico conta (`rollup::hourly_buckets`, buckets até onde o rollup chegou + o bruto agregado pela mesma SQL); a linha de base **não**, de propósito — o resultado avaliado já está gravado, e incluí-lo puxava a média para a própria anomalia (três testes de `baseline_alerts` provaram); o motivo ficou no módulo
    - [x] Dockerfile: o estágio `spike` deixa de copiar o exemplo inexistente `spike_snmp_v2c` (estágio removido ou exemplo corrigido). — exemplo corrigido para `spike_async_snmp` (o `docker-compose.icmp-spike.yml` usa o estágio)
    - [x] Laya-tuning (repositório vizinho): `sync.py` aponta para `crates/assistant` e `crates/core/src/{ai,classifier,devices}`; `sync-check` verde. — commit `c55859e` lá; "contrato idêntico ao código", pytest/mypy/ruff verdes
    - [x] Teste intermitente `throughput::btest::tests::cliente_envia_ao_servidor_armado`: causa achada e corrigida (QUA-07). — 2 falhas em 160 execuções; a asserção comparava totais sem ordem garantida pelo protocolo; 320 execuções sem falha depois
    - [x] `DeciderTemplateClassifier` do syslog renomeado para o que ele é (delega ao `Classifier` do processo). — `ProcessTemplateClassifier`
  - **Aceite:**
    - [x] Teste que falha sem a correção do gráfico/linha de base. — `buckets_horarios_incluem_o_que_o_rollup_ainda_nao_agregou`
    - [x] `docker build --target spike` (ou a ausência do alvo) sem erro.

### Fase 12 — Plugues por área: `settingsTabs` e `WidgetSpec`

- [x] 🟢 Concluído — **Configurações e widgets contribuídos pela área**
  - **Entregas:**
    - [x] `defineArea` ganha `settingsTabs` (abas e cartões de Configurações); `SettingsPage.vue` só renderiza o registro. O cadastro de fabricantes vai para a Rede; Endereços da central e DNS ficam no Núcleo — a aba de uma área desligada some. — `settingsTabs` + `settingsCards`, montados por `areas/settings.ts`; a aba do Núcleo virou **Conectividade** e a **Rede** é da área; os cartões ficaram autossuficientes (o de endereços abre o próprio diálogo, o de notificações faz o teste local, o de preferências carrega sozinho) e todos avisam pelo mesmo `saved`
    - [x] `WidgetSpec` no `core`: o widget declara área, dados e coleta; o `docker_summary` passa a ser contribuído pelo crate `servers`, e o catálogo é montado no ponto de montagem. — `netmonitor_core::dashboard` (`WidgetSpec`, `install_widgets`, `default_layout`); o tipo deixou de ser `enum` fechado e virou o id declarado pela área; Núcleo, Rede (`dns_latency`, SaaS, banda) e Servidores (`docker_summary`) declaram os seus; `registries::dashboard_widgets`; `contracts/ai_chat.json` sem diff (o `order` de cada card mantém a ordem)
  - **Aceite:**
    - [x] Com Rede desligada, Configurações não mostra nada só da Rede (teste Vitest). — `tests/areas/settings.spec.ts`
    - [x] Nenhuma lista fixa de abas nem de widgets fora dos registros.

### Fase 13 — Migrations por crate

- [x] 🟢 Concluído — **Cada crate leva o próprio esquema**
  - **Entregas:**
    - [x] Cada crate (`core`, `nucleo`, `servers`, `vpn`, `network`, `assistant`) expõe `migrations()` com as tabelas dele; o Migrator do ponto de montagem junta na ordem core → núcleo → áreas, num só histórico `seaql_migrations`. — *ajustado:* as tabelas do Núcleo ficaram no `core` (25 + o banco de logs), junto das entidades de todas as tabelas, que moram lá desde a Fase 1 — um `nucleo` à parte obrigaria os testes das áreas a depender dele; Rede 8, VPN 2, Servidores 1, assistente 1; `src/migrator.rs` monta; `area_test_migrator!` dá a cada crate o banco dos testes (núcleo + o dele)
    - [x] O crate comum `migration` fica só com os helpers de esquema (ou some). — some: os helpers viraram `netmonitor_core::schema`
    - [x] Teste de arquitetura: tabela de área só tem FK para o `core`/núcleo. — `tabela_de_area_so_aponta_para_o_nucleo_ou_para_a_propria_area`
  - **Aceite:**
    - [x] Banco recriado do zero com o mesmo esquema exportado (SQLite e PostgreSQL). — SQLite: os 123 objetos do `sqlite_master` iguais, texto a texto, ao banco criado pelo Migrator antigo; PostgreSQL 17: `db migrate` do zero, 37 migrations e 47 tabelas
    - [x] Entidades regeneradas contra o PostgreSQL sem diff. — campos e tipos idênticos nas 46 tabelas; em 7 a ordem de declaração de um campo difere, o que já valia antes desta fase (as entidades não mudam desde a Fase 1) e não afeta o SeaORM, que lê por nome

### Fase 14 — Crates de submódulo da Rede

- [ ] 🔴 Pendente — **`network-wifi` e `network-link-tests`**
  - **Entregas:**
    - [ ] Wi-Fi e testes de enlace viram crates que dependem do `network` (a base da área), continuando a área Rede.
    - [ ] O teste de arquitetura conhece submódulo: depende só da base da própria área e do `core`.
  - **Aceite:**
    - [ ] Só movimento (sem mudança de comportamento); suíte verde nos dois bancos.

### Fase 15 — Papel por área ([ADR 019](adr/019-papel-por-area.md))

- [ ] 🔴 Pendente — **Operator, viewer ou sem acesso, por área**
  - **Entregas:**
    - [ ] `user_area_roles` e `AreaAccess` no núcleo; admin global vale em tudo e é o único que atribui.
    - [ ] Sem acesso: some do menu e das rotas, a IA perde as ferramentas da área, widgets e eventos SSE dela não chegam, notificações da área não são entregues ao usuário.
    - [ ] Tela de usuário com a matriz área × nível e o atalho "mesmo papel em todas".
  - **Aceite:**
    - [ ] Testes de requisição: viewer e sem-acesso por área em rota, IA, SSE e notificação.
    - [ ] Vitest: perda de acesso no meio da sessão muda menu e rotas pelo SSE, sem chamada HTTP.

### Fase 16 — Menu por usuário

- [ ] 🔴 Pendente — **Fixar e ocultar, salvo no servidor**
  - **Entregas:**
    - [ ] Preferência de menu por usuário (itens fixados no topo, itens ocultos), salva no servidor e entregue pelo SSE.
    - [ ] O montador `buildNavSections` aplica as preferências depois do filtro de áreas e papéis (uma regra só).
    - [ ] Ações de fixar/ocultar no próprio menu, com "restaurar padrão".
  - **Aceite:**
    - [ ] Vitest do montador com fixados e ocultos; preferência vale em outro navegador.

### Nota de validação (2026-11-05)

Conferências feitas depois da Fase 10, sobre o código final:

- **SQLite e PostgreSQL:** `cargo test --workspace --all-targets` contra um
  PostgreSQL 17 (`postgres:17-alpine`, como o CI). Achou três testes que só
  passavam no SQLite em fuso `-03:00` — não intermitentes, já quebrados antes
  desta frente e escondidos porque o job PostgreSQL do CI não rodava (o digest
  fixado da imagem deixou de existir). Corrigidos: filtro de fuso negativo nos
  snapshots de `auth` e o tipo de banco esperado pelo banco em uso em
  `settings::database_size`; os 28 testes envolvidos verdes nos dois bancos, e
  o digest do CI atualizado. Detalhes em QUA-07 (`debitos_tecnicos.md`).
- **Teste de arquitetura pega dependência proibida:** com
  `netmonitor-servers` acrescentado de propósito ao `crates/vpn/Cargo.toml`,
  `os_manifestos_seguem_a_regra_de_dependencia` falha com
  `["vpn → servers"]`; revertido, os cinco testes voltam a passar.
- **Imagem Docker e binário do agente:** `docker build .` completo (526 s);
  na imagem, `backend-cli` (lista as sete tarefas, `vpn_probe_register`
  incluída) e `/app/agent-dist/netmonitor-agent-x86_64` (sobe e recusa a
  configuração sem `AGENT_SERVER_URL`, como deve). `cargo build --release`
  também verde.
- **Benchmarks (Fase 8), rodados ao vivo:** `laya-tuning sync-check` —
  contrato idêntico ao código antes (`7864880`) e depois da extração. Depois,
  as duas rodadas reais nesta máquina, contra a rodada oficial:
  - **Decisor** (`bench`, 471 casos, `netmonitor/decima-network:v13` no
    Ollaya local): **93,3** geral, igual à oficial `2026-11-02-decima-network-v13`
    (0,9326), com a mesma nota nas seis decisões (revisão de ação 91,9,
    identidade 98,0, triagem 94,6, interfaces 81,2, log 96,0, ferramentas
    97,9) e o mesmo digest de contrato (`9e80fa82f155`).
  - **Chat** (`chat bench`, 26 cenários, `lfm3-netmonitor:latest`,
    `reasoning=none`, prompt compacto, no Ollama local): **96,1** — "atende",
    igual à oficial `2026-11-05-chat-bench-v5-official` (0,9606) —, com o
    mesmo prompt médio (1218 tokens).
  As rodadas foram de conferência (`tmp`) e removidas do Laya-tuning.

---

## 6. Como medir

| Métrica | Antes | Meta | Ao fim da Fase 10 |
| :--- | :--- | :--- | :--- |
| Entradas de menu — perfil Completo | ~36 | ~26 (Fase 0) | 26 para admin (Fase 0) |
| Entradas de menu — só Servidores | ~36 | ~15 (Fase 3) | entregue (Fase 3) |
| Etapas do onboarding — só Servidores | 8, nenhuma de servidor | ≤ 3 (Fase 4) | entregue (Fase 4) |
| Ferramentas da IA — só Rede | ~60 | sem nenhuma `docker_*` (Fase 5) | nenhuma `docker_*` |
| Snapshots Docker sem ninguém olhando | contínuos com qualquer aba | zero (Fase 5) | zero |
| Exceções no teste de arquitetura | 5 (A1–A5) + A6–A7 no frontend | 0 (Fase 10) | **0** (lista removida) |
| Linhas no crate `legacy` | ~185 mil | 0 (Fase 10) | 0 — pacote raiz só monta (~2,9 mil linhas) |
| Migrations | 64 | uma base por crate (Fases 1 e 6–9) | 37 + 3 (uma por tabela, no `migration` comum) |

## 7. Fora do plano, de propósito

- Features do Cargo, imagens por edição, microsserviços — rejeitados no ADR 017.
- Subáreas ligáveis (Wi-Fi, enlace como chave própria) — os crates de submódulo
  da Fase 14 continuam sendo a área Rede.

Papel por escopo, menu por usuário e crates de submódulo saíram daqui em
2026-11-06: viraram as Fases 14–16.

## 8. Decisões que estavam em aberto

Nenhuma. As três do [ADR 017](adr/017-arquitetura-modular.md#decisões-tomadas-antes-em-aberto)
foram decididas: o nome na interface é **"Áreas"**; os plugins de dispositivo
ficam na **Rede**; as ferramentas `ping_host` e `traceroute` da IA ficam no
**Núcleo** (as páginas, na Rede). A troca do Loco.rs foi decidida no
[ADR 018](adr/018-loco-ou-axum.md): fica — confirmado pelo responsável pelo
produto em 2026-11-06.

Entrevista de 2026-11-06 (tudo o que restava em aberto):

| Pergunta | Decisão |
| :--- | :--- |
| Manter o Loco.rs? | Manter (ADR 018 confirmado). |
| Migrations no crate comum ou por crate? | Por crate, um Migrator só (Fase 13). |
| `settingsTabs` e `WidgetSpec`? | Os dois agora (Fase 12). |
| Gráfico de 7 dias e linha de base com o buraco da hora fechada? | Corrigir (Fase 11). |
| Pendências pequenas (Dockerfile `spike`, `sync.py`, `btest`, nome do classificador)? | Todas (Fase 11). |
| Papel por escopo? | Papel por área; sem papel = sem acesso; admin global vale em tudo; sem acesso esconde menu, rotas, IA, widgets, eventos e notificações ([ADR 019](adr/019-papel-por-area.md), Fase 15). |
| Menu por usuário? | Fixar e ocultar, salvo no servidor (Fase 16). |
| Crates de submódulo? | `network-wifi` e `network-link-tests` sobre o `network` (Fase 14). |
| A branch? | Fica sem PR até concluir as Fases 11–16. |
