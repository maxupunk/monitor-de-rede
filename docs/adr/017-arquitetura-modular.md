# ADR 017 — Arquitetura modular: Áreas do produto e reconstrução em crates

## Status

**Aceito e implementado** (2026-11-05). Proposto em 2026-11-05; as Fases 0–10
do [roadmap da arquitetura modular](../roadmap_arquitetura_modular.md) foram
entregues — o plano de execução e o que cada fase entregou moram lá; este
documento registra **o que** foi decidido e **por quê**. O que ficou de
fora, de propósito, está em [Implementação](#implementação-fase-10).

Consolida duas análises independentes do código (registradas antes de serem
comparadas — ver [Divergências resolvidas](#divergências-resolvidas)) e uma
premissa do dono do produto: **o sistema nunca foi para produção**.

## Contexto

O NetMonitor atende dois públicos: quem gerencia **rede** (roteadores,
switches, APs, enlaces, VPN) e quem gerencia **servidores com Docker** (hosts,
containers, compose, bancos). Muitas instalações usam um lado só e recebem o
produto inteiro.

### O excesso está na superfície

| Superfície | Hoje (commit `be8d227`) | Quem sofre |
| :--- | :--- | :--- |
| Menu | ~36 entradas fixas em `layouts/DefaultLayout.vue` (`navSections`). Só o papel admin, os plugin-apps e o PWA escondem algo. Docker está dentro de "Ferramentas"; "Servidores remotos", em "Infraestrutura". | Quem usa só Docker vê ~13 entradas de rede; quem usa só rede vê 7 de Docker. |
| Onboarding | 8 etapas em `InitialSetupDialog.vue`, todas de rede; `services/onboarding.rs` conta só sites, redes, DNS e VPN. | Quem usa só Docker não tem etapa que conecte um servidor. |
| Dashboard | `DEFAULT_WIDGETS` (`stores/dashboard.ts`) traz os dois lados; o catálogo existe duplicado em `services/dashboard/catalog.rs`. | Os dois públicos. |
| Assistente IA | ~60 ferramentas numa lista estática (`ai/harness/tools/mod.rs`), com o grupo Docker, a regra de containers do prompt e a menção `@container` sempre presentes. | Modelos pequenos (Decisor, `lfm3-netmonitor`) erram mais o roteamento com ferramentas inúteis no contexto. |
| Tempo real | Com **qualquer** aba aberta, a central pede snapshot Docker a todos os agentes (`agents/background.rs`, `desired_live_interval(has_subscribers)`). | CPU e rede gastos com quem nunca abre o Docker. |

### O código não tem fronteira entre os domínios

Medido por referências `crate::services::<x>`:

- **Tudo é registrado incondicionalmente:** 38 `add_route` em `app.rs`, 7
  initializers, e `initializers/monitoring.rs` dispara agendador, agentes,
  backups, fabricantes e tarefas agendadas sem condição.
- **Dependências na direção errada:**

| # | Acoplamento | Onde |
| :-: | :--- | :--- |
| A1 | Motor de monitoramento conhece Docker | `monitoring/runner.rs` (`match` em `"container"`/`"host_resources"`, importa `DockerEngine`) |
| A2 | Rede chama a IA concretamente | `devices/identify.rs`, `discovery/decider_identity.rs`, `snmp/decider_interfaces.rs`, `syslog/categorizer.rs` → `ai::decider` |
| A3 | IA conhece todos os domínios | `ai/harness/tools/{docker,platform,grep/sources}.rs`, `ai/mentions.rs`, lista estática `all_handlers` |
| A4 | Catálogo de widgets com Docker no núcleo, e duplicado | enum em `dashboard/catalog.rs` **e** `CARD_TEMPLATES`/`DEFAULT_WIDGETS` no frontend |
| A5 | Agente conhece discovery e network_tools | `agents/background.rs`, `agent_runtime/` |
| A6 | Barramento SSE central no frontend | `stores/events.ts`: importa 26 stores, `switch` com ~58 casos |
| A7 | Dashboard da SPA importa Docker | `pages/DashboardPage.vue` → `useDockerStore` |

- **Tamanho:** ~185 mil linhas de Rust num crate só, ~107 mil no frontend,
  ~2.400 testes, 47 tabelas, 64 migrations.

### O que já está certo e serve de molde

- **Capacidades comprovadas por dispositivo** (`devices/capabilities.rs`): a aba
  só aparece com evidência. "Funcionalidade indisponível não gera aba vazia."
- **Plugins com `auto_enable`**: ligam por evidência; desligar à mão é
  permanente.
- **Registros em vez de `v-if`**: `widgetRegistry.ts`, `inspectorRegistry.ts`,
  `AiToolHandler` + `all_handlers`.
- **Docker isolado na lógica de domínio**: o `bollard` só existe no agente; a
  central fala pelo trait `DockerEngine`, já injetado como
  `Option<Arc<dyn DockerEngine>>` no runner.
- **Agente multi-domínio por permissão** (`agents/policy.rs`): Docker
  (`read`, `lifecycle`, `update`, `compose`, `volume_export`), rede
  (`monitor`, `discovery`, `device_io`, `throughput`) e bancos (`database`).
- **Tópicos SSE com prefixo de domínio** (`docker:*`, `wifi:*`, `link_test:*`).
- **"Preferência que ninguém lê é fachada"** (`services/preferences.rs`) — vale
  para área também.

### A premissa que simplifica tudo

**Nenhuma instalação real existe.** Isso elimina, de uma vez:

- migration de compatibilidade, initializer de upgrade e o estado `auto` que
  servia para "quem já usa o sistema";
- o cuidado com `seaql_migrations` (o débito DB-02 deixa de ser cosmético e
  passa a ser resolvível);
- qualquer *shim* para manter rota, campo ou tópico antigo.

Até a primeira instalação real, **mudança de esquema edita a migration
existente** e o banco de desenvolvimento é recriado.

## Como produtos maduros resolvem

| Produto | Mecanismo | Lição |
| :--- | :--- | :--- |
| **Portainer** | Barra lateral muda conforme o tipo de ambiente. | A interface segue o que existe. |
| **Netdata** | Só mostra o que foi detectado. | Revelação progressiva. |
| **Proxmox** | Painel do Ceph oferece instalação quando não existe. | Área vazia vira convite, não tela morta. |
| **Zabbix** | Papéis com acesso a elementos de UI — e o aviso de que esconder uma página não bloqueia o dado em outra. | **Esconder não é segurança.** |
| **Grafana** | App plugins declaram páginas no menu (`includes`/`addToNav`). | Menu montado por declaração. |
| **GitLab** | Toggles por projeto com dependências; épico aberto sobre remoção inconsistente da navegação. | Toggle sem registro central fica inconsistente; granularidade fina vira dívida. |
| **Home Assistant** | Ocultar itens vale por navegador (issue aberta). | Personalização gravada no servidor. |
| **Odoo** | Desinstalar módulo apaga dados (bugs públicos). | **Desligar nunca é destrutivo.** |
| **Shopify** | Monólito modular com fronteiras verificadas por ferramenta (*packwerk*). | Fronteira por convenção apodrece; tem de ser verificada. |

## Alternativas avaliadas

| Opção | Veredito | Por quê |
| :--- | :--- | :--- |
| Builds ou imagens por edição (features do Cargo) | ❌ | Multiplica a matriz de testes (features × SQLite/Postgres); `CREATION_ORDER`, entidades e bindings `ts-rs` variariam por build; "ligar depois" viraria trocar de imagem; o handshake estrito do agente não admite recursos opcionais. |
| Microsserviços | ❌ | Problema de experiência, não de escala. Dois bancos, eventos distribuídos e auth compartilhada sem ganho. |
| Domínios como plugins Rhai | ❌ | O Rhai é sandbox de equipamento (ADR 012); o Docker é privilegiado e de enum fechado. |
| Módulos "duros" em runtime (desmontar rotas, 404) | ❌ | Quebra links de alertas, eventos e IA; vira controle de acesso disfarçado. |
| Só revelação automática por evidência | ❌ sozinha | Falha na instalação nova (a tela cria a evidência) e não deixa esconder o que existe mas não interessa. Fica como **sugestão**. |
| Navegação por usuário | ⏸ | Não resolve o excesso por instalação; volta depois como "fixar/ocultar no meu menu". |
| Reescrever do zero | ❌ | Jogaria fora ~2.400 testes e o conhecimento acumulado (`BEGIN IMMEDIATE`, ICMP `SOCK_DGRAM`, `run_task` sem initializers, entidades contra Postgres) e pararia o produto por meses. |
| **Áreas em runtime + reconstrução no lugar em crates** | ✅ | Ver Decisão. |

## Decisão

Duas trilhas complementares, com responsabilidades distintas:

- **Áreas** — decisão de **produto**, em runtime: o que a instalação mostra e
  quais rotinas próprias roda.
- **Crates** — decisão de **engenharia**, em build: quem pode depender de quem.

Área e crate **não precisam coincidir**. Uma tabela pode morar no `core` (por
causa das FKs) e a tela dela pertencer à área Rede.

### 1. Áreas do produto

| Área | Conteúdo visível | Pode desligar? |
| :--- | :--- | :--- |
| **Núcleo** | Dashboard, Dispositivos (servidor é dispositivo), Monitores de **todos** os tipos, Alertas e regras, Eventos, Logs, Manutenção, Tarefas agendadas, Assistente IA, Notificações, Servidores remotos (agentes), Usuários, Auditoria, Backup, Destinos de armazenamento, Configurações | Não |
| **Rede** | Topologia, Descoberta, Redes IP, Sites (a tela; a tabela é do `core`), Testes de velocidade (ADR 015), Sinais Wi-Fi, Sondas de rede, Diagnóstico (Playbooks, Traceroute, Velocidade do navegador), Plugins de dispositivo e aplicativos de frota, configuração de Syslog dos equipamentos, servidores DNS, registro de fabricantes, widgets de banda/latência/DNS/SaaS | Sim |
| **Servidores** | Docker (containers, volumes, redes, imagens, projetos, histórico), Backup de bancos (ADR 013), widget `docker_summary`, ferramentas `docker_*` da IA, menção `@container`, modelo de tarefa de ação em container | Sim |
| **VPN** | Servidor WireGuard, dispositivos VPN, aba VPN do dispositivo | Sim, com servidor desativado antes |

**Não criar mais áreas.** Dentro de uma área ligada, quem esconde o que não
existe continua sendo a capacidade por dispositivo.

#### Estados: `on` e `off`

- Sem estado `auto`: ele existia para o upgrade de instalações reais, que não
  existem.
- **Onboarding** pergunta "O que você vai gerenciar?" e grava `on`/`off`.
  Quem pula o assistente fica com **tudo ligado** — o comportamento atual,
  sem nada sumir.
- **Evidência gera sugestão, nunca mudança.** Um aviso único em cada direção:
  "Encontramos um host Docker. Mostrar a área Servidores?" ou "Nenhum host Docker
  além desta central. Ocultar a área Servidores?". A evidência ignora o que o
  sistema cria sozinho — **o agente local, o dispositivo do sistema e os DNS
  padrão não contam**.

#### O que a área decide — e o que não decide

| Peça | Área `off` |
| :--- | :--- |
| Menu | Some. Montado por registro declarativo, com `area` em cada entrada. |
| Rotas da SPA | Continuam registradas com `meta.area`; o guard leva à página "Área desligada" (admin vê "Ligar"). Link profundo nunca dá 404. |
| **API** | **Continua montada e autorizada pelo papel.** Área não é controle de acesso. |
| **Monitores** | **Nunca param.** São intenção explícita do usuário; a confirmação de desligar informa "N monitores continuam ativos". |
| Rotinas próprias | Rede: varredura periódica, testes de enlace agendados e watch de Wi-Fi pausados. Servidores: `SetLive{0}` aos agentes e streams de log fechados. Métricas de host continuam (são do núcleo). |
| SSE | Os eventos deixam de ser **produzidos na fonte**; nada é filtrado no transporte. O estado viaja em `areas:updated` e no snapshot ao conectar (AGENTS §9). |
| IA | Ferramentas, grupo do `load_tools`, intenções do Decisor, linhas do prompt e menções da área somem. |
| Tarefas agendadas | Com ferramenta da área: `skipped`, com o motivo. |
| Alertas | Regras e eventos existentes continuam; os modelos da área somem do catálogo "Criar regra". |
| Dashboard | Widgets da área ficam no layout salvo e não são renderizados; religar restaura. |
| Configurações | Cartões e chaves acompanham a área. |
| Dados | **Nada é apagado, nunca.** |

#### Regras de mudança

- Só admin altera; a mudança é auditada e publicada em `areas:updated`.
- Persistência em `system_settings`, chave `areas`, JSON — como
  `preferences.rs`.
- A confirmação mostra as **consequências calculadas pelo backend**.
- **Desligar é recusado com exposição ativa**: VPN com servidor habilitado
  precisa ser desativada antes — esconder um túnel vivo é antipadrão de
  segurança.
- **Papéis ficam como estão** (admin, operator, viewer). Área decide
  relevância; papel decide permissão. Papel por escopo, se pedido, é outro ADR.

#### Contrato

```rust
// crate core — services/areas
pub enum Area { Core, Network, Servers, Vpn }                 // ts-rs
pub struct AreaState { area: Area, enabled: bool }           // ts-rs

pub trait AreaPolicy {                                        // uma impl por área
    fn area(&self) -> Area;
    async fn evidence<C: ConnectionTrait>(&self, db: &C) -> AppResult<Evidence>;
    async fn consequences<C: ConnectionTrait>(&self, db: &C) -> AppResult<Vec<Consequence>>;
    async fn can_disable<C: ConnectionTrait>(&self, db: &C) -> AppResult<Result<(), Blocker>>;
}
```

- `GET /api/areas` e `PUT /api/areas/{area}` (admin, auditado).
- **Declaração exaustiva:** `AiToolHandler::area()`, `DashboardWidgetType::area()`
  e cada modelo dos catálogos de alerta e de tarefa agendada declaram a sua área
  por `match` **sem `_ =>`** — peça nova sem área não compila.
- **Catálogo único de widgets**: o frontend lê o enum do backend pelo `ts-rs`.

### 2. Reconstrução no lugar, em crates

O backend vira um **workspace Cargo** em `backend/`, sem reescrita: o código
atual entra inteiro num crate transitório e sai dele um módulo por vez, com a
suíte verde a cada passo.

```text
backend/
├── Cargo.toml                [workspace]
└── crates/
    ├── core/                 plataforma + pontos de extensão (traits) + inventário
    ├── agent-protocol/       contrato central ↔ agente (enum fechado, PROTOCOL_VERSION)
    ├── network/              área Rede
    ├── servers/              área Servidores
    ├── vpn/                  área VPN
    ├── assistant/            IA: chat, harness, Decisor, proativo
    ├── legacy/               transitório: o que ainda não foi extraído (some no fim)
    ├── app/                  binário da central: Hooks do Loco, montagem dos registros
    └── agent/                binário do agente remoto
```

#### Regra de dependência

```text
app, agent  ──▶  qualquer crate        (pontos de montagem)
network, servers, vpn, assistant  ──▶  core, agent-protocol
core  ──▶  agent-protocol               (nunca uma área)
área  ─╳─▶  outra área                  (proibido; conversa por trait do core)
```

O compilador garante a regra: a dependência proibida não existe no
`Cargo.toml`. Até os crates existirem, um teste de arquitetura faz o papel dele
(ver Consequências).

#### Pontos de extensão no `core`

As áreas **implementam**; o `core` **consulta um registro** montado pelo `app`.
Cada um resolve um acoplamento da tabela do Contexto:

| Trait | Resolve | Exemplo de implementação |
| :--- | :--- | :--- |
| `MonitorChecker` | A1 | `ContainerChecker` em `servers`; `SnmpChecker` em `network` |
| `Classifier` (identificar dispositivo, classificar log, nomear interface) | A2 | `assistant` implementa com o Decisor; o `core` tem a heurística padrão |
| `AiToolHandler` + `area()` | A3 | ferramentas `docker_*` em `servers` |
| `WidgetSpec` | A4 | `docker_summary` em `servers` |
| `AgentCommandHandler` (lado central) | A5 | `Discovery` em `network`; `Docker` em `servers` — o **enum** continua fechado em `agent-protocol` |
| `AlertDataset`, `AreaPolicy` | — | `wifi_client` em `network`; `vpn_peer` em `vpn` |

**Monitores rodam sempre**: todos os checkers são compilados e registrados; a
área só muda o que se vê. Crate é fronteira de código, não interruptor.

#### Dados

- **Cada crate é dono das suas tabelas, entidades e migrations.** O `app`
  concatena as migrations e o `CREATION_ORDER` na ordem de dependência.
- **FK só aponta para o `core`**, nunca entre áreas. Por isso o **inventário**
  (`sites`, `networks`, `devices`, `device_interfaces`), `probes` e `monitors`
  ficam no `core`, mesmo com telas na área Rede — `devices.network_id` e
  `monitors.probe_id` são FKs para eles.
- **Base consolidada**: as 64 migrations viram **uma base por crate**,
  editada no lugar até a primeira instalação real. O
  `migration::purge_removed_migrations` deixa de ser necessário.
- Entidades continuam geradas **contra o PostgreSQL**; SQLite e PostgreSQL
  continuam obrigatórios.

#### Frontend

```text
frontend/src/areas/
├── core/  network/  servers/  vpn/
│   └── index.ts   defineArea({ id, routes, nav, widgets, sse, settingsTabs, onboardingSteps })
└── index.ts       registro (import.meta.glob)
```

- `navigation.ts` puro monta o menu a partir das contribuições; `meta.area` nas
  rotas; um guard; `AreaOffPage.vue`.
- O barramento SSE vira **registro de handlers por prefixo** (`docker:` → store
  Docker); o `switch` de `events.ts` some.
- `stores/areas.ts` alimentada **só por SSE**.
- Mesmo pacote, sem monorepo de pacotes: não há ganho nesse tamanho.

#### O que **não** muda

Rust, Vue 3 + Vuetify, SQLite padrão com PostgreSQL opcional, SSE sem polling,
agente que só disca para fora, política local soberana (`AGENT_ALLOW`), sandbox
Rhai, ponte de banco que só copia bytes, fronteiras do AGENTS §7. A
reconstrução é de **estrutura**, não de tecnologia.

#### Loco.rs

Fica durante a reconstrução (decidido depois dela, com dados, no
[ADR 018](018-loco-ou-axum.md): fica). Hoje 176 arquivos o referenciam e o `app.rs` já
contorna três comportamentos dele (`run_task` sem initializers, logger
reimplementado, `file_appender` não reproduzido). Quando o `app` for o único
ponto de montagem, trocá-lo por `axum` + `sea-orm` direto vira uma troca
localizada — decidida então, com dados, num ADR próprio.

## Divergências resolvidas

As duas análises concordaram em: monólito com áreas em runtime, nada de builds
separados nem plugins, estado em `system_settings`, SSE com snapshot, desligar
sem apagar, onboarding por intenção, filtro de IA e widgets, agentes no núcleo,
papéis separados de áreas. Divergiram em:

| # | Ponto | Decisão | Motivo |
| :-: | :--- | :--- | :--- |
| 1 | Rota de área desligada devolve 404 | API continua montada | Esconder não protege; 404 quebraria links profundos. |
| 2 | Monitores pausados com a área | Monitores nunca param | Pausar calaria um alerta sem aviso. |
| 3 | Estados `on/off` × `auto/on/off` | `on/off` + sugestão por evidência | O `auto` existia para upgrade de instalações reais; não há nenhuma. |
| 4 | VPN dentro de Rede × área própria | Área própria | Serve aos dois lados (o assistente de peer instala agente em servidor); tem porta UDP e regra própria de desligamento. |
| 5 | Fronteiras por crate agora × fora de escopo | Crates, pela reconstrução no lugar | Com a premissa de reconstruir e sem produção, o custo cai: base de migrations consolidável, nenhum shim, extração incremental com a suíte como especificação. |

## Consequências

**Positivas**

- Menu de ~36 entradas cai para ~26 com a limpeza de navegação e ~15 no perfil
  só Servidores.
- Onboarding de quem usa só Servidores com no máximo 3 etapas.
- Menos ferramentas no contexto da IA no perfil de um lado só.
- Fim do snapshot Docker para quem nunca abre o Docker.
- Fronteira garantida pelo compilador; compilação incremental por crate, o
  que pesa nesta máquina (erro 1455 de paginação com o crate único).
- Feature nova tem lugar óbvio: implementa um trait do `core` dentro da sua área.

**Custos assumidos**

- **Cada área é um toggle permanente**: testada ligada e desligada para sempre.
- **Pontos de extensão custam indireção**: um registro a mais para ler em vez
  de um `match`.
- **A extração é trabalho longo** e convive com o crate `legacy` durante
  várias fases. Mitigação: cada fase entra na `main` com a suíte verde — não
  existe branch de reescrita.
- **Até os crates existirem**, a regra de dependência é verificada por um teste
  de arquitetura (`backend/tests/architecture.rs`) com a lista de exceções
  A1–A7 congelada; a lista só diminui.

**Antipadrões proibidos**

1. Esconder como forma de segurança.
2. Desligar de forma destrutiva.
3. Área que ninguém lê (teste garante consumidor em menu, IA, widgets e rotinas).
4. Área ou flag por página.
5. Evidência volátil (online/offline).
6. `if área` fora dos registros.
7. Desmontar rotas da API.
8. Polling do estado das áreas.
9. Features do Cargo para isso.
10. Catálogos duplicados entre frontend e backend.
11. Contar o que o sistema cria sozinho como evidência.
12. Dependência entre áreas, ou do `core` para uma área.

## Portão da Fase 6 (medido)

A extração piloto (`crates/servers`, com `agent-protocol` e o transporte dos
agentes no `core`) foi medida antes de seguir:

| Medida | Valor |
| :--- | :--- |
| Tempo de trabalho | ~1 h de extração + ~1 h de validação (suíte completa ~7 min por rodada) |
| Código movido | `servers` 10,8 mil linhas, `agent-protocol` 2,8 mil, `core` +~4 mil; pacote raiz ficou com ~160 mil |
| Diff fora das renomeações | 108 arquivos, +770 / −1387 (o grosso é caminho de `use`) |
| Testes ajustados | 4 (conversões que saíram do contrato) + 2 novos (credencial, registro) |
| `cargo check` após tocar um arquivo | `servers`: 48 s · pacote raiz: 119 s |
| `cargo build` incremental | `servers`: 36 s · raiz: 54 s · `core`: 71 s (recompila todos os dependentes) |
| Exceções do teste de arquitetura | 37 → 34 |

**Decisão: seguir, com ajustes.** O ganho existe e é o esperado — quem mexe
numa área extraída tem retorno de `check` em menos da metade do tempo, e o
compilador passa a impedir a dependência entre áreas. O custo foi menor que o
previsto porque nenhum comportamento mudou e a suíte serviu de especificação.
Ajustes para as Fases 7–10:

1. **Núcleo primeiro.** Antes de extrair uma área, mover para o `core` o
   serviço do núcleo de que ela depende (como foi o transporte dos agentes),
   em vez de abstraí-lo em trait.
2. **Registros no ponto de montagem** (`src/registries.rs`) desde o início da
   fase; `default-members` já cobre o workspace.
3. **Contratos do assistente na Fase 8.** As ferramentas `docker_*` e as das
   outras áreas só saem quando o `AiToolHandler` estiver no `core`.
4. **`core` é caro de tocar** (recompila tudo): mudanças no `core` em PR
   próprio, antes da extração que as usa.

## Decisões tomadas (antes em aberto)

1. **Nome na interface: "Áreas".** "Módulo" sugere instalar e desinstalar;
   "Recursos" é genérico demais. É o nome em Configurações › Geral, no
   onboarding e na página de área desligada.
2. **Plugins de dispositivo na Rede.** Moram em `crates/network` (ADR 012); o
   núcleo os alcança só pelo `DeviceAccess` (credencial, shell, plugins no
   cadastro). Um plugin de servidor, se vier, decide o seu lugar no ADR dele.
3. **`ping_host` e `traceroute` da IA no Núcleo.** Quem usa só Servidores
   continua diagnosticando alcance pela IA; as páginas de ping e traceroute
   ficam na Rede.

## Implementação (Fase 10)

O workspace ao fim da reconstrução difere do desenho da seção 2 em três
pontos, todos medidos:

- **Sem `crates/legacy` nem `crates/app`/`crates/agent`.** O pacote raiz
  (`backend`) foi o crate transitório e virou o ponto de montagem dos dois
  binários (central e `netmonitor-agent`): mover o pacote quebraria os caminhos
  relativos a ele (configuração do Loco, bindings, banco de teste) sem ganho.
- **O Núcleo em dois crates.** `core` guarda os contratos e o que as áreas
  também usam; os serviços, rotas e telas do Núcleo estão em `crates/nucleo`,
  do qual nenhuma área depende. Medido (build incremental com uma função
  pública nova, `-j 2`): mudar o `nucleo` custa **10 s** (ele e a montagem);
  mudar o `core`, **17 s** (todas as áreas, o assistente e a montagem). Com o
  Núcleo dentro do `core`, toda mudança de serviço pagaria o segundo preço.
- **Pontos de extensão a mais, nascidos quando foram precisos:** além dos da
  tabela, `MonitorGroup` (coleta consolidada), `Routine`/`RetentionJob`
  (rotinas do scheduler e retenção por área), `HostDocker` (o Docker do host
  do agente), `DeviceAccess` (o acesso ao equipamento que a Rede oferece) e o
  `CentralServices` de cada área (o que ela pede à central). O `WidgetSpec`
  **não** foi criado: o `docker_summary` é uma entrada de catálogo com
  `area()`, sem código que dependa de Servidores. Migrations continuam num
  crate comum (`migration`): as tabelas das áreas só têm FK para o `core`.

A regra de dependência é do compilador; `tests/conventions/architecture.rs`
guarda o que ele não vê. A troca do Loco.rs foi decidida no
[ADR 018](018-loco-ou-axum.md): fica.

## Referências

- [Zabbix — User roles](https://www.zabbix.com/documentation/8.0/en/manual/web_interface/frontend_sections/users/user_roles)
- [Grafana — Build an app plugin](https://grafana.com/developers/plugin-tools/tutorials/build-an-app-plugin)
- [GitLab — épico 3719](https://gitlab.com/groups/gitlab-org/-/epics/3719)
- [Home Assistant frontend — issue 30024](https://github.com/home-assistant/frontend/issues/30024)
- [Odoo — bug 933328](https://bugs.launchpad.net/bugs/933328)
- Internas: [ADR 007](007-scheduler-processo-unico.md), [ADR 011](011-agente-remoto.md),
  [ADR 012](012-plugins-de-dispositivo.md), [ADR 014](014-docker-local-pelo-agente.md),
  [Servidor NetMonitor como dispositivo](../roadmap_servidor_netmonitor_como_dispositivo.md),
  [débitos DB-02 e ARQ-01 a ARQ-06](../debitos_tecnicos.md#7-débitos-de-arquitetura-modular-adr-017)
