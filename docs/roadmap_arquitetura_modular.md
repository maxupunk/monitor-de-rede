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
    - `navigation.ts`: função pura que monta as seções a partir de entradas declarativas, já com `area` em cada uma (ainda sem efeito); o layout só renderiza.
    - Docker (7 subitens) vira **1** entrada, com as abas que já existem em `DockerLayout.vue`; VPN (2 subitens) vira 1 entrada com abas.
    - Docker e "Servidores remotos" passam a uma seção **Servidores**; "Sondas de rede" vai para **Rede**; o grupo "Coleta remota" deixa de existir.
    - "Assistente IA" sai de Diagnóstico (já está na barra superior); "Benchmark dos modelos" vai para Configurações › IA.
    - Remover o segundo `case 'device:updated'` de `events.ts`.
  - **Aceite:**
    - Menu cai de ~36 para ~26 entradas.
    - Testes Vitest do montador: papel, plugin-apps, grupo aberto pela rota.
    - Nenhuma URL quebra.
  - **Entregue:** `frontend/src/navigation.ts` (`buildNavSections`, `groupOfPath`, ações nomeadas que o layout executa); `pages/vpn/VpnLayout.vue` com as abas Servidor e Dispositivos; 26 entradas para admin sem plugin-apps nem PWA; o filtro por `enabledAreas` já existe e é testado, sem fonte de dados até a Fase 3. Sites ficou na área Rede (na Núcleo, quem usa só Servidores veria uma seção "Rede" só com Sites). Teste: `tests/utils/navigation.spec.ts`.

### Fase 1 — Esqueleto do workspace e base de dados

- [x] 🟢 Concluído — **Workspace Cargo com o código atual num crate transitório**
  - **Débitos:** ARQ-06, ARQ-02 (congelamento)
  - **Esforço:** Médio
  - **Arquivos:** `backend/Cargo.toml`, novo `backend/crates/{app,legacy,core,agent-protocol,agent}/`, `Dockerfile`, `.github/workflows/ci.yaml`
  - **Entregas:**
    - `backend/` vira raiz do workspace; o crate atual vira `crates/legacy`; os binários (`main.rs`, `agent.rs`) vão para `crates/app` e `crates/agent`.
    - `crates/agent-protocol`: `services/agents/protocol.rs` extraído (contrato pequeno e já fechado — `PROTOCOL_VERSION` inalterado).
    - `crates/core` nasce com os **pontos de extensão** do ADR 017 (`MonitorChecker`, `Classifier`, `WidgetSpec`, `AgentCommandHandler`, `AlertDataset`, `AreaPolicy`) e os registros que o `app` monta. O `legacy` passa a implementá-los onde hoje há `match` fixo — sem mudar comportamento.
    - `backend/tests/architecture.rs`: confere os `use crate::services::…` contra a matriz permitida; a lista de exceções começa com A1–A5 e **só pode diminuir**.
    - Um `CARGO_TARGET_DIR` único para o workspace (AGENTS, limpeza).
  - **Aceite:**
    - Os quatro comandos do backend passam de `backend/`, sem nenhum teste alterado além de caminhos.
    - Imagem Docker e binário do agente gerados como antes.
    - Teste de arquitetura falha ao introduzir de propósito uma dependência nova proibida.
  - **Entregue:** `backend/` é a raiz do workspace e o **pacote raiz é o crate transitório** (e o ponto de montagem) — em vez de `crates/legacy` + `crates/app`: mover o pacote quebraria os caminhos relativos a ele (config do Loco, bindings, fixtures, banco de teste) sem ganho, e o papel é o mesmo. `crates/core` (`netmonitor-core`) recebeu `models`, `shared` e `events` — o conjunto fechado medido pelo grafo de imports —, com reexportação dos caminhos antigos. Versões comuns em `[workspace.dependencies]`; `TS_RS_EXPORT_DIR` fixo em `.cargo/config.toml`. `tests/conventions/architecture.rs` congela 37 exceções (o detector lê `use` agrupado e caminho inline; a lista só diminui). **Ajustes medidos:** o `agent-protocol` sai na Fase 6 — o contrato carrega tipos de Docker, monitoramento, vazão, plugins e telemetria, que só ganham casa quando o crate da área existe; e cada ponto de extensão nasce na fase que o usa (`AreaPolicy` na 2, `MonitorChecker`/`AgentCommandHandler` na 6, `Classifier` na 8), sem abstração especulativa.

- [x] 🟢 Concluído — **Base de dados consolidada**
  - **Débitos:** DB-02
  - **Esforço:** Pequeno
  - **Arquivos:** `backend/migration/`, `src/models/tables.rs`, `src/app.rs`
  - **Entregas:**
    - As 64 migrations viram uma **base** na ordem do `CREATION_ORDER` (e a do banco de logs, a dela). A partir daqui, mudança de esquema edita a base.
    - Remover `migration::purge_removed_migrations` e a chamada em `after_context`.
  - **Aceite:**
    - Banco recriado do zero em SQLite e PostgreSQL com o mesmo esquema de antes (comparação do esquema exportado).
    - Entidades regeneradas **contra o PostgreSQL** sem diff.
  - **Entregue:** 64 → 37 migrations no banco principal e 4 → 3 no de logs: uma migration de criação por tabela, com todo `ALTER` posterior dobrado nela (ordem de colunas preservada); as só de dados e as de criar-e-apagar foram removidas. Esquema comparado antes/depois em SQLite e PostgreSQL 17 (colunas, tipos, nulidade, padrões, índices, FKs): idêntico, salvo o nome da FK `database_connections.via_probe_id` (agora pelo helper `fk(...)`) e a posição ordinal no Postgres de `storage_destinations` (lacunas de coluna apagada). `purge_removed_migrations` removido.

### Fase 2 — Áreas no backend

- [x] 🟢 Concluído — **Serviço de áreas no `core`**
  - **Débitos:** ARQ-01
  - **Esforço:** Médio
  - **Arquivos:** `crates/core/src/areas/`, controller fino em `crates/app`
  - **Entregas:**
    - `Area`, `AreaState` (ts-rs) e uma `AreaPolicy` por área com `evidence`, `consequences` e `can_disable`.
    - Estado `on`/`off` em `system_settings`, chave `areas`. Padrão de banco novo: **tudo `on`**.
    - `GET /api/areas`; `PUT /api/areas/{area}` só admin, auditado.
    - Evento `areas:updated` na mudança e no snapshot ao conectar.
    - VPN: `can_disable` recusa com servidor habilitado.
  - **Aceite:**
    - Testes de evidência em SQLite em memória, incluindo "o agente local não é evidência de Servidores" e "o dispositivo do sistema não é evidência de Rede".
    - Requisição: viewer recebe 403 no `PUT`; `off` da VPN recusado com servidor ativo; mudança registrada na auditoria.
    - SQLite e PostgreSQL.
  - **Entregue:** `netmonitor_core::areas` (`Area`, `AreaSet`, `EnabledAreas`, `AreaPolicy`, `AreaService`, `AreasSnapshot`, `AreaImpact`); políticas em `services/area_policies/` (vão para o crate de cada área nas Fases 6–9); `controllers/areas.rs` com `GET /api/areas`, `GET /api/areas/{area}/impact` e `PUT /api/areas/{area}` (admin, auditado, publicado); `AreasInitializer` publica o snapshot no boot. `require_admin` virou um só (`auth_guard`), no lugar de duas cópias. Testes: `crates/core/src/areas/state.rs`, `tests/requests/areas.rs`.

- [x] 🟢 Concluído — **Declaração exaustiva de área e catálogo único de widgets**
  - **Débitos:** ARQ-04
  - **Esforço:** Médio
  - **Arquivos:** `services/ai/harness/tools/`, `services/dashboard/catalog.rs`, `services/alerts/catalog/`, catálogo de tarefas agendadas, `frontend/src/stores/dashboard.ts`
  - **Entregas:**
    - `AiToolHandler::area()`, `DashboardWidgetType::area()` e `area` em cada modelo dos catálogos de alerta e de tarefa agendada — `match` sem `_ =>`.
    - O frontend passa a ler o catálogo de widgets pelo binding `ts-rs`; `CARD_TEMPLATES` e a lista duplicada saem de `stores/dashboard.ts`.
  - **Aceite:**
    - Peça nova sem área não compila (provado por teste de compilação ou `match` exaustivo).
    - Nenhuma lista de tipos de widget no frontend.
  - **Entregue:** `AiToolHandler::area()` obrigatório (60 ferramentas), `DashboardWidgetType::area()` e `AlertCategory` (enum no lugar de texto) com `area()` — todos `match` sem `_ =>`; `GET /api/dashboard/catalog` (`DashboardCatalog`), lido pelo frontend no lugar de `CARD_TEMPLATES`/`DEFAULT_WIDGETS`.

### Fase 3 — Frontend por áreas

- [x] 🟢 Concluído — **Registro de áreas na SPA**
  - **Débitos:** ARQ-01, ARQ-03, ARQ-04
  - **Esforço:** Médio
  - **Arquivos:** novo `frontend/src/areas/{core,network,servers,vpn}/index.ts`, `navigation.ts`, `router/index.ts`, `stores/events.ts`, `stores/dashboard.ts`, `pages/DashboardPage.vue`, `pages/SettingsPage.vue`
  - **Entregas:**
    - `defineArea({ id, routes, nav, widgets, sse, settingsTabs, onboardingSteps })` e o registro em `areas/index.ts`.
    - `stores/areas.ts` alimentada **só pelo SSE** (`areas:updated` e snapshot).
    - Menu filtrado; `meta.area` nas rotas; um guard; `AreaOffPage.vue` com "Ligar" para admin e o motivo para os demais.
    - Card **Áreas do sistema** em Configurações › Geral, no padrão do `SetupAppsStep` (avatar colorido, frase, `v-switch` verde), com as consequências vindas da API num `ConfirmDialog`.
    - Barramento SSE como **registro de handlers por prefixo**; o `switch` de `events.ts` some.
    - `DashboardPage` sem `useDockerStore`: o `docker_summary` carrega a própria store.
    - Layout do dashboard: widget de área desligada fica salvo e não é renderizado; `parseSavedList` deixa de reintroduzir padrão de área desligada.
    - Abas do dispositivo exigem capacidade **e** área (VPN, Wi-Fi).
  - **Aceite:**
    - Teste Vitest: `areas:updated` muda o menu **sem nenhuma chamada HTTP** (AGENTS §9).
    - Link profundo para `/docker` com Servidores `off` mostra a `AreaOffPage`.
    - Layout salvo não perde widget ao desligar e religar.
    - Teste: área desligada não registra handlers SSE.
  - **Entregue:** `frontend/src/areas/{core,network,servers,vpn}` com `defineArea({ id, routes, nav, widgets, sse })` e registro em `areas/index.ts`; `areas/realtime.ts` registra e desregistra os handlers SSE por área; `events.ts` virou barramento genérico (o `switch` e os imports de stores saíram); `router/areaGuard.ts` + `AreaOffPage.vue`; `AreasCard.vue` em Configurações › Geral; catálogo do dashboard vindo da API. `settingsTabs` e `onboardingSteps` não entraram no contrato: as abas de configuração ainda não têm conteúdo por área, e as etapas do assistente têm registro próprio por área (`components/setup/steps.ts`). Testes: `tests/areas/*.spec.ts`, `tests/stores/dashboard.spec.ts`.

### Fase 4 — Onboarding por intenção

- [x] 🟢 Concluído — **"O que você vai gerenciar?"**
  - **Débitos:** ARQ-01
  - **Esforço:** Médio
  - **Arquivos:** `frontend/src/components/setup/`, `InitialSetupDialog.vue`, `backend/src/services/onboarding.rs`
  - **Entregas:**
    - Primeira etapa com Rede, Servidores e Docker, e VPN (múltipla escolha), gravando `on`/`off`; Assistente IA continua do núcleo.
    - Caminho só Servidores: Endereços da central → agente local e comando para conectar outro servidor (reaproveita `AgentEnrollmentDialog`) → Notificações.
    - Caminho Rede: as etapas atuais; a etapa "Aplicativos" só com Rede ligada.
    - `OnboardingStatus` considera as áreas escolhidas.
    - Sugestões por evidência, avisadas uma vez em cada direção (ADR 017).
  - **Aceite:**
    - Caminho só Servidores com no máximo 3 etapas.
    - O status não cobra rede de quem escolheu só Servidores.
    - Teste de componente para os dois caminhos.
  - **Entregue:** `POST /api/settings/onboarding/intent` (admin; exige Rede ou Servidores) e `OnboardingStatus.intentChosen`; `components/setup/steps.ts` (registro de etapas por área), `SetupIntentStep.vue`, `SetupAgentsStep.vue`, `SetupNotificationsStep.vue` e as etapas de rede extraídas do `InitialSetupDialog.vue` (de ~1600 para ~600 linhas); `components/areas/AreaSuggestions.vue` (sugestões por evidência, uma vez por direção). Testes: `tests/components/InitialSetupDialog.spec.ts`, `tests/components/AreaSuggestions.spec.ts`, `tests/requests/areas.rs`.

### Fase 5 — Backend respeita as áreas

- [x] 🟢 Concluído — **IA, rotinas, catálogos e tempo real filtrados pela área**
  - **Débitos:** ARQ-01, ARQ-05
  - **Esforço:** Médio
  - **Arquivos:** `services/ai/{harness,decider,mentions.rs}`, `services/scheduled_tasks/runner.rs`, `services/agents/background.rs`, ciclo do scheduler, `services/wifi/`, `services/link_tests/`, catálogo de alertas
  - **Entregas:**
    - `ToolRegistry` e `UnattendedTools` filtram pelas áreas ligadas: grupo do `load_tools`, intenções do Decisor, linhas do prompt e menções se ajustam sozinhos.
    - Tarefa agendada com ferramenta de área desligada fica `skipped`, com o motivo.
    - Rede `off`: varredura periódica, testes de enlace agendados e watch de Wi-Fi pausados (o ciclo lê o estado do banco a cada volta, como `auto_discovery_enabled`, porque o scheduler pode rodar em outro processo).
    - Servidores `off`: `SetLive{0}` aos agentes e streams de log fechados. Com Servidores `on`, o snapshot ao vivo passa a depender de quem usa dado Docker, não de qualquer assinante.
    - Modelos de regra da área somem do catálogo "Criar regra".
  - **Aceite:**
    - Com Servidores `off`: nenhuma ferramenta `docker_*` no registro e nenhum `docker` no enum do `load_tools`; `SetLive{0}` enviado.
    - **Monitores de container continuam rodando** com Servidores `off` (teste).
    - Rotinas de Rede pausadas com `off` e retomadas com `on` (teste).
  - **Entregue:** `ToolPolicy.areas` (+ `with_areas`, que também desliga as ações em container) aplicado uma vez em `run_agent_loop` e na confirmação; `UnattendedTools::for_areas`; tarefas de área desligada viram `skipped` com o motivo (`scheduled_tasks/areas.rs`); menções `@` sem Docker; varredura periódica, enlaces agendados e vigia de Wi-Fi pausados com Rede `off`; **interesse por conexão SSE** (`?interest=docker`, `events::Interests`, base comum `ConnectionCounter` com a presença): o laço de `SetLive` só pede snapshot com alguém olhando o Docker **e** Servidores ligada (ARQ-05). Testes: `tools/mod.rs` (sem ferramenta docker), `tests/requests/areas.rs` (monitor de container continua; varredura pausa e volta).

### Fase 6 — Piloto: crate `servers` (portão de decisão)

- [x] 🟢 Concluído — **Extrair a área Servidores**
  - **Débitos:** ARQ-02 (A1, parte de A3–A5), ARQ-06
  - **Esforço:** Médio
  - **Arquivos:** `services/docker/`, parte de contêiner de `services/telemetry/`, `services/databases/`, `monitoring/checkers/{container,host_resources}.rs`, `ai/harness/tools/docker.rs`, controllers e views correspondentes, `frontend/src/areas/servers/`
  - **Entregas:**
    - `crates/servers` com Docker, métricas de container, backup de bancos (ADR 013), ferramentas `docker_*`, widget `docker_summary` e os handlers de comando Docker/compose/volume do lado central.
    - `ContainerChecker` registrado como `MonitorChecker`; `monitoring/runner.rs` perde o `match` e o import de Docker (**A1 sai da lista de exceções**).
    - Métricas de host ficam no `core` (servidor é dispositivo).
    - Tabelas de servidores na base do crate; FK só para o `core`.
  - **Aceite:**
    - `crates/core` compila sem `servers`; `servers` não depende de nenhuma outra área.
    - Suíte inteira verde nos dois bancos.
    - **Portão:** registrar no ADR 017 o custo medido (tempo, linhas movidas, testes ajustados, ganho de compilação incremental) e decidir seguir, ajustar ou parar nas Fases 7–10.
  - **Entregue:** três crates novos. `crates/agent-protocol` (`agent_protocol`, sem dependência do workspace) leva o contrato (`protocol.rs`, `PROTOCOL_VERSION` inalterado, `policy.rs`, `tunnel.rs`) e os tipos que trafegam nele (DTOs Docker, `CheckResult`, `ThroughputSpec`, `DeviceIoCall`, tarefas de sonda, rollup de telemetria); as conversões com regra de negócio saíram do contrato (`AppError` ⇄ `RemoteError` no core, `ThroughputSpecExt` e `DriveRequest` ⇄ `Command` na vazão). O **transporte dos agentes** (`hub`, `session`, `bridge`, `HostKey`, identidade, rotas) foi para o `netmonitor-core` em vez de virar trait: só dependia do contrato e do `AppError`, e mover custou menos que abstrair chamadas, chunks e pontes. Também no core: `audit`, `storage`, `telemetry::{store, views}` e `monitoring` (`Checker`, `MonitorChecker`, `MonitorCheckers`). `crates/servers` (`netmonitor_servers`, depende só de `core` e `agent-protocol`): `docker/` (com o `AgentEngine`, o lado central dos comandos Docker/compose/volume), `databases/`, `ContainerMonitor` (registrado como `MonitorChecker`), `ServersPolicy`, controllers, DTOs e views. `src/registries.rs` monta políticas e checkers no ponto de montagem; `monitoring/runner.rs` perdeu o braço `container` e o import de Docker; `default-members` cobre o workspace inteiro (o CI roda `cargo test` sem `--workspace`). Exceções do teste de arquitetura: 37 → 34 (`monitoring → docker` — A1 —, `agents → docker`, `telemetry → docker`). **Ficou para depois, de propósito:** ferramentas `docker_*` da IA (o contrato `AiToolHandler` é do assistente — vão com a inversão da Fase 8); `WidgetSpec` (o `docker_summary` é uma entrada de catálogo sem lógica); `AgentCommandHandler` (nada no lado central precisou); migrations no crate `migration` (as tabelas de servidores só têm FK para o core). Portão registrado no ADR 017.

### Fase 7 — Crate `vpn`

- [x] 🟢 Concluído — **Extrair a área VPN**
  - **Débitos:** ARQ-06, ARQ-02 (parte)
  - **Esforço:** Pequeno
  - **Arquivos:** `services/vpn/`, `controllers/{vpn_servers,vpn_peers}.rs`, `frontend/src/areas/vpn/`
  - **Entregas:** `crates/vpn`; o contrato com o watcher (`<iface>.conf`/`<iface>.status` em `WG_CONFIG_DIR`) e o cofre de chaves em memória **inalterados** (AGENTS §7); o dataset de alerta `vpn_peer` registrado como `AlertDataset`.
  - **Aceite:** `vpn` depende só de `core`; snapshots `insta` dos artefatos sem diff; testes `#[serial]` do cofre intactos.
  - **Entregue:** `crates/vpn` (`netmonitor_vpn`, depende **só** do `netmonitor-core`): servidor, peers, IPAM, chaves, `config_builder`/`config_writer`/`peer_status` (o contrato com o watcher — só mudaram os `use`), pré-voo, perfis de cliente e scripts, telemetria dos túneis, `access_control`, `VpnPolicy`, as rotas `/api/vpn/{server,peers}` e as views. Snapshots `insta` renomeados para o caminho do crate, com o corpo intacto; `services/vpn` virou reexportação do crate (IA, onboarding e scheduler até as Fases 8 e 10). **Núcleo primeiro:** foram para o `core` o cofre efêmero (`shared::secret_store`, byte a byte — o enrollment dos agentes e as senhas de sessão dos plugins também o usam), o enrollment e o cadastro dos agentes (`agents::{enrollment, registration}`; o `AgentService` delega), `devices::{adapters, system_device}`, `maintenance::resource_cleanup`, `monitoring::reachability` e `vpn_probe` (`probe_is_external`, `vpn_probe_name`). **Fica no pacote raiz, de propósito:** o registrador do `vpn-probe` (`services/probes/vpn_registrar.rs` — é cadastro de probe, usado no boot, na troca de banco e pelo `task vpn_probe_register`; o fallback `DEFAULT_VPN_PROBE_TOKEN` intacto) e o observador da transição dos túneis (`services/alerts/vpn_peer_watcher.rs`, junto do dataset `vpn_peer`, que o motor de alertas já tratava como um dataset qualquer — sem `match` nem import de VPN, então não houve `AlertDataset` a criar). O que a área pede ao resto da central (motor de alertas, apresentação de dispositivo, endereços da central) entra por um único trait da área, `netmonitor_vpn::central::CentralServices`, implementado em `src/registries.rs` e posto no `shared_store` pelo `after_context` (vale para o scheduler). Exceções do teste de arquitetura: **34 → 29** (`agents → vpn`, `devices → vpn`, `plugins → vpn`, `server_addresses → vpn`, `settings → vpn`); o detector conta `netmonitor_vpn::` como a área VPN. **Ficou para depois:** `ai → vpn` (ferramentas da IA, Fase 8), `monitoring → vpn` (o `maintenance_runner` chama a telemetria dos túneis — sai com um registro de rotinas por área) e `onboarding → vpn` (pré-voo e servidor no assistente inicial).

### Fase 8 — Crate `assistant`

- [x] 🟢 Concluído — **Extrair a IA e inverter o Decisor**
  - **Débitos:** ARQ-02 (A2, A3)
  - **Esforço:** Grande
  - **Arquivos:** `services/ai/`, `controllers/{ai,ai_conversations}.rs`, `initializers/ai.rs`
  - **Entregas:**
    - `crates/assistant` com chat, harness, drivers, Decisor e proativo.
    - `Classifier` no `core` com a heurística padrão; o `assistant` registra a implementação pelo Decisor. `devices`, `discovery`, `snmp` e `syslog` passam a depender só do trait (**A2 sai**).
    - Ferramentas de cada domínio contribuídas pelo crate do domínio; o `assistant` não importa área (**A3 sai**).
  - **Aceite:** sem o `assistant` registrado, identificação de dispositivo e classificação de log funcionam pela heurística (teste); benchmarks do Decisor e do chat sem regressão.
  - **Entregue:** `crates/assistant` (`netmonitor_assistant`, depende **só** do `netmonitor-core`): chat e harness (laço do agente, aprovação, compactação, prompt, grounding), provedores (`drivers`, `ollama`, `openrouter`, `opencode`, `local_server`, `local_models`), Decisor, rotinas automáticas, marcações, conversas, pesquisa na web, `settings`, DTOs e as rotas `/api/ai` e `/api/ai/conversations` (o `AiInitializer` continua registrado no `app.rs`, só chama `proactive::spawn`). `services/ai`, `dtos/ai` e `controllers/{ai,ai_conversations}` viraram reexportação. **`Classifier` no `core`** (`netmonitor_core::classifier`): o trait com `min_confidence`, `identify_device`, `advise_interfaces` e `categorize_log`, a fila (`Lane`) e os dados que cruzam a fronteira (`DeviceFacts`, `IdentitySuggestion`, `DeciderSuggestion`, `LogPattern` e as categorias, `InterfaceCandidate` e o ranking); o padrão é a `HeuristicClassifier` (nenhum palpite — fica a heurística de quem chama) e o assistente registra o `DeciderClassifier` em `src/registries.rs`, instalado pelo `after_context` (vale para o scheduler). As perguntas e a leitura das respostas (`Decision`) continuam no assistente. **Contrato das ferramentas no `core`** (`netmonitor_core::ai`): `AiToolHandler`, `ToolKind`, `ToolGroup`/intenções, `ToolPolicy` (lê as configurações por um trait, `ToolSettings`), `ToolRegistry`/`UnattendedTools`, `ToolArgs`, risco, `AiChart`, `AiTool`, `AiAutomationMode`, a busca de dispositivo/monitor (`lookup`), as séries, a documentação que a IA consulta (`knowledge`), a fonte do `grep` (`GrepSource`) e a seção do estado da plataforma (`PlatformSection`). O catálogo, **na mesma ordem**, é montado em `src/registries.rs::ai_tool_catalog` e instalado com `install_catalog`: as `docker_*` e a fonte `docker` do `grep` vêm do crate `servers`, a seção `vpn` do `get_platform_status` do crate `vpn`, `get_topology`, a seção `discovery` e os plugins dos módulos da Rede (`services/{topology,discovery,plugins}/ai_tools.rs`, para a Fase 9), a `web_search` do assistente e o resto do núcleo (`services/ai_tools/`). **Núcleo primeiro:** foram para o `core` `devices::kinds`, `alerts::{contracts, feed}` (+ `PROBLEM_KIND`, `role_weight`), o contrato dos canais de notificação, `units::{round_two, duration_to_ms}`, o `if_type_label` do SNMP e o `authenticated_user`. O que o assistente pede ao resto da central (Web Push, canais de notificação, inibição dos alertas, plugins de dispositivo, Docker das marcações) entra por `netmonitor_assistant::central::CentralServices`, implementado em `src/registries.rs`. Exceções do teste de arquitetura: **29 → 18** (`ai → {discovery, docker, plugins, snmp, topology, vpn}` — **A3** —, `{devices, discovery, snmp, syslog} → ai` — **A2** — e `scheduled_tasks → ai`); o detector conta `netmonitor_assistant::` como o assistente. Teste de aceite: `sem_o_assistente_a_identificacao_segue_pela_heuristica` e `sem_o_assistente_o_log_segue_pela_heuristica` (o classificador retirado com `classifier::uninstall`; o Decisor configurado e no ar não é consultado). **Benchmarks:** precisam de LLM real e não rodam aqui; a garantia é o contrato sem diff — `backend/contracts/ai_chat.json` (prompt, núcleo, `load_tools`, grupos e schemas, agora gerado em `src/registries/ai_catalog_tests.rs`) saiu byte a byte igual, e as perguntas do Decisor só mudaram de casa. **Ficou para depois:** `plugins → ai` (a revisão de segurança do plugin chama o provedor do chat — pede um "completar texto" no `core`), as ferramentas de Rede para o crate `network` (Fase 9) e o nome `DeciderTemplateClassifier` do syslog (hoje delega ao `Classifier` do processo).

### Fase 9 — Crate `network`

- [x] 🟢 Concluído — **Extrair a área Rede, em quatro passos** (cada um um PR verde)
  - **Débitos:** ARQ-02 (A5), ARQ-06
  - **Esforço:** Grande
  - **Passos:**
    1. Descoberta, fabricantes e varredura de portas.
    2. SNMP e topologia (`SnmpChecker` como `MonitorChecker`).
    3. Testes de enlace (ADR 015) e Wi-Fi.
    4. Plugins de dispositivo e espelho de pacotes (ADR 012); configuração de Syslog dos equipamentos.
  - **Fica no `core`:** inventário (`sites`, `networks`, `devices`, `device_interfaces`), `probes`, `monitors` e os primitivos de `network_tools` usados pelos checkers do núcleo (ICMP, TCP, DNS) — são alvo de FK ou do motor de monitoramento.
  - **Aceite:** `network` depende só de `core` e `agent-protocol`; handlers de `Discovery`/`DeviceIo`/vazão registrados como `AgentCommandHandler` (**A5 sai**); política local do agente intocada.
  - **Entregue:** `crates/network` (`netmonitor_network`, depende **só** de `netmonitor-core` e do `agent-protocol`), em quatro commits verdes. **A ordem dos passos seguiu as dependências**, não a lista: a descoberta chama o SNMP (enriquecimento e coletores) e o SNMP chama a topologia, e o Wi-Fi e os testes de enlace dependem dos plugins — por isso o passo 1 levou SNMP e topologia junto e os plugins vieram antes do Wi-Fi e do enlace. **9.1** — descoberta, fabricantes (OUI), varredura de portas e os serviços de SNMP e topologia; `NetworkPolicy`; rotas `/api/discovery`, `/api/vendors`, `/api/port-scan`. **9.2** — o checker SNMP virou `SnmpMonitor` (`MonitorChecker`), registrado na central e no agente pelo mesmo `registries::monitor_checkers`: o `monitoring/runner.rs` perdeu o braço `snmp`; rotas `/api/snmp` e `/api/topology`. **9.3** — plugins de dispositivo (ADR 012: sandbox Rhai e `DeviceTransport` inalterados; plugin importado continua em quarentena; `reveal` intocado), espelho de pacotes, motores de vazão e o agente como origem do teste (`throughput::agent`); as sessões SSH/Telnet com o equipamento saíram do `syslog::provision` para `plugins::transport::shell`, de onde a ativação do log as usa. **9.4** — testes de enlace (ADR 015) e Wi-Fi (com o dataset de alerta `wifi_client`), rotas `/api/link-tests*` e `/api/wifi*`. **`AgentCommandHandler`** nasceu no `core` (`agents::command`) com os dois lados: no agente, o executor aplica a política local (`AGENT_ALLOW`, **intocada**) e entrega `Discovery`, `DeviceIo` e `Throughput` aos handlers da Rede (`agent_commands`); na central, o pump de `agents::background` entrega as varreduras pendentes pelo `AgentCommandDispatcher` da Rede (`discovery::remote::DiscoveryDispatch`). O enum continua fechado no `agent-protocol` e `PROTOCOL_VERSION` não mudou. **Núcleo primeiro:** foram para o `core` as preferências, os primitivos de `network_tools` dos checkers do núcleo (ICMP — sonda e cliente do processo —, TCP, UDP, vizinhos, NAT do container), `CheckDeps`, `device_status`, `execution_guard`, `link_speed`, `metrics_repository`, as séries e a coleta de saúde do sistema, os campos e o dataset de interface dos alertas (+ `format_seconds`), as plataformas (`devices::systems`), o papel do usuário (`roles`), o `access` e o teto/nomes de rede das ferramentas da IA, as chaves do palpite do Decisor, o alvo aceitável de equipamento (`shared::cidr`) e as redes aceitas pelo agente (`agents::targets`); o inventário, `probes` e `monitors` ficaram no núcleo. O que a Rede pede à central entra por `netmonitor_network::central::CentralServices` (motor de alertas, fechamento de alerta e regras do Wi-Fi vigiado, endereços da central, identificação de dispositivo, porta publicada da API e uma leitura da IA para a revisão de segurança do plugin — sai `plugins → ai` sem "completar texto" no `core`). As ferramentas da IA da Rede (topologia, seção `discovery`, plugins) vieram com os módulos. Testes que ligam a central a um agente de verdade foram para `tests/requests/{agent_commands,agent_throughput}.rs` (o servidor iperf3 de mentira atrás da feature `testing`); snapshots `insta` dos acessos dedicados renomeados com o corpo intacto. `contracts/ai_chat.json` e `frontend/src/bindings/` sem diff. Exceções do teste de arquitetura: **18 → 11** (`agent_runtime → {discovery, plugins}`, `agents → {discovery, plugins}`, `devices → discovery`, `network_tools → plugins`, `plugins → ai`); o detector conta `netmonitor_network::<módulo>` pelo módulo. **Ficou para depois, de propósito:** um registro de rotinas por área para o scheduler e a limpeza (`monitoring → {link_tests, snmp, wifi}`, `maintenance → link_tests` — o executor de grupos SNMP depende do `result_processor` do núcleo), o polling HTTP do probe pelo despachante (`probes → discovery`), as capacidades do dispositivo sem o Wi-Fi (`devices → wifi`), a ativação do log e as dicas do formulário do syslog (`syslog → {plugins, snmp}`, presas ao receptor do núcleo), o Docker do executor do agente como handler de Servidores (`agent_runtime → docker`) e as ferramentas `scan_ports`/`playbook` (área Rede) no `services/ai_tools` do núcleo, que só usam primitivos do núcleo.

### Fase 10 — Encerramento

- [ ] **`legacy` vazio e documentação final**
  - **Esforço:** Pequeno
  - **Entregas:**
    - O que restar do `legacy` vai para o `core`; o crate é removido.
    - Teste de arquitetura reduzido ao que o compilador não vê (ex.: toda área tem consumidor em menu, IA, widgets e rotinas).
    - ADR 017 passa a **aceito e implementado**; ADR 018 decide manter o Loco.rs ou trocar por `axum` + `sea-orm` direto, com dados.
    - `arquitetura.md` §4 descreve os crates como são; `AGENTS.md` e `backend/AGENTS.md` sem referência ao crate único.
  - **Aceite:** lista de exceções do teste de arquitetura vazia; nenhum `use` entre áreas no workspace.

---

## 6. Como medir

| Métrica | Hoje | Meta |
| :--- | :--- | :--- |
| Entradas de menu — perfil Completo | ~36 | ~26 (Fase 0) |
| Entradas de menu — só Servidores | ~36 | ~15 (Fase 3) |
| Etapas do onboarding — só Servidores | 8, nenhuma de servidor | ≤ 3 (Fase 4) |
| Ferramentas da IA — só Rede | ~60 | sem nenhuma `docker_*` (Fase 5) |
| Snapshots Docker sem ninguém olhando | contínuos com qualquer aba | zero (Fase 5) |
| Exceções no teste de arquitetura | 5 (A1–A5) + A6–A7 no frontend | 0 (Fase 10) |
| Linhas no crate `legacy` | ~185 mil | 0 (Fase 10) |
| Migrations | 64 | uma base por crate (Fases 1 e 6–9) |

## 7. Fora do plano, de propósito

- Papel por escopo ("operador só de Docker") — outro ADR, se pedido.
- Menu por usuário ("fixar/ocultar no meu menu") — depois das Fases 0–3.
- Features do Cargo, imagens por edição, microsserviços — rejeitados no ADR 017.
- Crates de submódulo (Wi-Fi, enlace) — só com demanda real.

## 8. Decisões em aberto

Ver [ADR 017 — Decisões em aberto](adr/017-arquitetura-modular.md#decisões-em-aberto):
nome na interface, lugar dos plugins de dispositivo e ferramentas de ping e
traceroute para quem usa só Servidores. Precisam de resposta antes da Fase 0
(nome) e da Fase 2 (as outras duas).
