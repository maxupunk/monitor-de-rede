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

- [ ] **Workspace Cargo com o código atual num crate transitório**
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

- [ ] **Base de dados consolidada**
  - **Débitos:** DB-02
  - **Esforço:** Pequeno
  - **Arquivos:** `backend/migration/`, `src/models/tables.rs`, `src/app.rs`
  - **Entregas:**
    - As 64 migrations viram uma **base** na ordem do `CREATION_ORDER` (e a do banco de logs, a dela). A partir daqui, mudança de esquema edita a base.
    - Remover `migration::purge_removed_migrations` e a chamada em `after_context`.
  - **Aceite:**
    - Banco recriado do zero em SQLite e PostgreSQL com o mesmo esquema de antes (comparação do esquema exportado).
    - Entidades regeneradas **contra o PostgreSQL** sem diff.

### Fase 2 — Áreas no backend

- [ ] **Serviço de áreas no `core`**
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

- [ ] **Declaração exaustiva de área e catálogo único de widgets**
  - **Débitos:** ARQ-04
  - **Esforço:** Médio
  - **Arquivos:** `services/ai/harness/tools/`, `services/dashboard/catalog.rs`, `services/alerts/catalog/`, catálogo de tarefas agendadas, `frontend/src/stores/dashboard.ts`
  - **Entregas:**
    - `AiToolHandler::area()`, `DashboardWidgetType::area()` e `area` em cada modelo dos catálogos de alerta e de tarefa agendada — `match` sem `_ =>`.
    - O frontend passa a ler o catálogo de widgets pelo binding `ts-rs`; `CARD_TEMPLATES` e a lista duplicada saem de `stores/dashboard.ts`.
  - **Aceite:**
    - Peça nova sem área não compila (provado por teste de compilação ou `match` exaustivo).
    - Nenhuma lista de tipos de widget no frontend.

### Fase 3 — Frontend por áreas

- [ ] **Registro de áreas na SPA**
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

### Fase 4 — Onboarding por intenção

- [ ] **"O que você vai gerenciar?"**
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

### Fase 5 — Backend respeita as áreas

- [ ] **IA, rotinas, catálogos e tempo real filtrados pela área**
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

### Fase 6 — Piloto: crate `servers` (portão de decisão)

- [ ] **Extrair a área Servidores**
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

### Fase 7 — Crate `vpn`

- [ ] **Extrair a área VPN**
  - **Débitos:** ARQ-06
  - **Esforço:** Pequeno
  - **Arquivos:** `services/vpn/`, `controllers/{vpn_servers,vpn_peers}.rs`, `frontend/src/areas/vpn/`
  - **Entregas:** `crates/vpn`; o contrato com o watcher (`<iface>.conf`/`<iface>.status` em `WG_CONFIG_DIR`) e o cofre de chaves em memória **inalterados** (AGENTS §7); o dataset de alerta `vpn_peer` registrado como `AlertDataset`.
  - **Aceite:** `vpn` depende só de `core`; snapshots `insta` dos artefatos sem diff; testes `#[serial]` do cofre intactos.

### Fase 8 — Crate `assistant`

- [ ] **Extrair a IA e inverter o Decisor**
  - **Débitos:** ARQ-02 (A2, A3)
  - **Esforço:** Grande
  - **Arquivos:** `services/ai/`, `controllers/{ai,ai_conversations}.rs`, `initializers/ai.rs`
  - **Entregas:**
    - `crates/assistant` com chat, harness, drivers, Decisor e proativo.
    - `Classifier` no `core` com a heurística padrão; o `assistant` registra a implementação pelo Decisor. `devices`, `discovery`, `snmp` e `syslog` passam a depender só do trait (**A2 sai**).
    - Ferramentas de cada domínio contribuídas pelo crate do domínio; o `assistant` não importa área (**A3 sai**).
  - **Aceite:** sem o `assistant` registrado, identificação de dispositivo e classificação de log funcionam pela heurística (teste); benchmarks do Decisor e do chat sem regressão.

### Fase 9 — Crate `network`

- [ ] **Extrair a área Rede, em quatro passos** (cada um um PR verde)
  - **Débitos:** ARQ-02 (A5), ARQ-06
  - **Esforço:** Grande
  - **Passos:**
    1. Descoberta, fabricantes e varredura de portas.
    2. SNMP e topologia (`SnmpChecker` como `MonitorChecker`).
    3. Testes de enlace (ADR 015) e Wi-Fi.
    4. Plugins de dispositivo e espelho de pacotes (ADR 012); configuração de Syslog dos equipamentos.
  - **Fica no `core`:** inventário (`sites`, `networks`, `devices`, `device_interfaces`), `probes`, `monitors` e os primitivos de `network_tools` usados pelos checkers do núcleo (ICMP, TCP, DNS) — são alvo de FK ou do motor de monitoramento.
  - **Aceite:** `network` depende só de `core` e `agent-protocol`; handlers de `Discovery`/`DeviceIo`/vazão registrados como `AgentCommandHandler` (**A5 sai**); política local do agente intocada.

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
