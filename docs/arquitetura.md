# Arquitetura

Descreve o sistema **como ele é hoje**. Quando este documento e o código
divergirem, o código está certo e este arquivo está desatualizado.

O backend é **Rust sobre [Loco.rs](https://loco.rs/)**, em `backend-rust/`. O
backend anterior era AdonisJS; ele saiu do repositório e o registro daquela
migração está em [`historico/`](historico/). As decisões técnicas que continuam
valendo estão em [`adr/`](adr/).

---

## 1. O que o sistema faz

Monitora redes residenciais e de pequenas empresas: descobre dispositivos,
executa checagens de disponibilidade (ICMP, HTTP, TCP, DNS, SNMP), coleta
métricas, monta topologia, avalia regras de alerta, notifica, e publica tudo em
tempo real para uma SPA. Opcionalmente sobe um servidor WireGuard para alcançar
redes remotas.

## 2. Stack

| Camada | Tecnologia |
| :--- | :--- |
| API, scheduler, probe | Rust — [Loco.rs](https://loco.rs/) sobre `axum` e `tokio` |
| ORM e migrations | SeaORM + `sea-orm-migration` |
| Banco | PostgreSQL (produção e desenvolvimento em Docker); SQLite na suíte de testes |
| Autenticação | JWT (`loco_rs::auth`), HS512 |
| Tempo real | SSE |
| Frontend | Vue 3 + TypeScript + Vite + Vuetify + Pinia + PWA |
| Túnel | WireGuard (`linuxserver/wireguard`) |
| Implantação | Docker Compose |

Não há Redis, nem fila externa, nem broker. Ver [§12](#12-o-que-não-existe).

## 3. Topologia de processos

Um único binário, `backend_rust-cli`, roda em quatro papéis diferentes conforme
o comando. São **oito serviços** no `docker-compose.yml`:

| Serviço | Comando | Papel |
| :--- | :--- | :--- |
| `migration` | `db migrate` | Roda as migrations e **sai**. Os outros esperam `service_completed_successfully`. |
| `server` | `start` | API HTTP na porta 3333 e o barramento SSE. |
| `scheduler` | `task scheduler_loop` | Processo de longa duração que repete o ciclo de monitores a cada 5 s ([ADR 007](adr/007-scheduler-processo-unico.md)). |
| `probe` | `task probe_run` | Agente de coleta na LAN. Processo de longa duração. |
| `vpn-probe` | `task probe_run` | Mesmo agente, mas no namespace de rede do WireGuard, para enxergar a `wg0`. |
| `wireguard` | — | Único container com `NET_ADMIN` e única porta UDP publicada. |
| `frontend` | — | nginx servindo a SPA e fazendo proxy de `/api/` para `server:3333`. |
| `postgres` | — | Banco. |

```text
                    ┌──────────┐
                    │ migration│  roda uma vez, sai
                    └────┬─────┘
                         │ (completed)
     ┌───────────────────┼───────────────────┐
     ▼                   ▼                   ▼
┌─────────┐        ┌───────────┐       ┌──────────┐
│ server  │◀──────▶│ scheduler │       │ postgres │
│  :3333  │        └───────────┘       └──────────┘
└────┬────┘              ▲  ▲
     │ /api/probes/*     │  │
     ├───────────────────┘  │
     │                      │
┌────▼────┐          ┌──────┴──────┐     ┌───────────┐
│  probe  │          │  vpn-probe  │────▶│ wireguard │
│  (LAN)  │          │  (túnel)    │     │   wg0     │
└─────────┘          └─────────────┘     └───────────┘
```

**O `server` nunca executa `wg` nem `docker exec`.** Ele escreve `<iface>.conf`
e lê `<iface>.status` num volume compartilhado; quem aplica é o container do
WireGuard. É o que mantém o servidor sem `NET_ADMIN`.

### Modos de instalação

- **Standalone** — tudo na mesma máquina, uma rede só. É o `docker compose up`.
- **Central com probes remotos** — o `server` central e um `probe` por site. O
  probe fala com o servidor por HTTP autenticado; **nunca** acessa o banco.
- **Central com túnel** — as redes remotas chegam por WireGuard e o `vpn-probe`
  mede dentro do túnel.

## 4. Organização do código

```text
backend-rust/
├── src/
│   ├── controllers/     extrai, valida, delega, serializa — sem regra de negócio
│   ├── services/        todo o domínio, testável sem HTTP
│   ├── models/          entidades SeaORM (_entities/, geradas) + regras de modelo
│   ├── views/           serialização de saída
│   ├── dtos/            entrada e tipos de contrato
│   ├── tasks/           comandos de CLI (inclui o laço do scheduler e o probe)
│   ├── initializers/    process_deps (todos os modos) + monitoring (só servidor)
│   ├── mailers/         templates de e-mail
│   └── bin/             entrypoint
├── migration/           uma migration por tabela + helpers em shared.rs
├── config/              development.yaml, test.yaml, production.yaml
├── examples/spikes/     protótipos que sustentam as ADRs
└── tests/               requests/, models/, conventions/
```

`src/services/` é onde o sistema realmente vive:

```text
services/
├── monitoring/     checkers/, runner, result_processor, device_status, presenter
├── discovery/      service, queue, merger, device_identifier, oui_lookup, cidr_range
├── snmp/           sessões, coletores, perfis
├── topology/       ligações e confiança
├── alerts/         datasets → evaluator → manager (+ catalog, recovery, repository)
├── notifications/  channels/ (telegram, discord, webhook, email) + formatter
├── probes/         agent, dispatcher, receiver, liveness, buffer
├── vpn/            peers, chaves, config_builder/writer, preflight, telemetria
├── events/         bus (SSE) e relay (outbox → bus)
├── network_tools/  port scanner e utilidades de rede
├── maintenance/    data_pruner e limpeza de recursos
├── zabbix/         importação de templates
└── shared/         crypto, pagination, errors
```

## 5. O ciclo de monitoramento

É o coração do sistema. O processo `scheduler` roda `run_cycle` em laço, uma vez
a cada `SCHEDULER_INTERVAL_SECONDS` (padrão 5 s), em
`src/tasks/scheduler_run.rs`:

```text
run_cycle (a cada 5 s, no mesmo processo)
   │
   ├─ monitores vencidos (next_run_at <= now)
   │     ├─ monitor tem probe? → enfileira em `probe_tasks`
   │     └─ não tem, ou o probe está offline? → executa local (`run_monitor`)
   │
   ├─ discovery: processa runs pendentes, agenda redes vencidas
   ├─ VPN: sincroniza telemetria dos túneis
   ├─ watchdog: probe sem heartbeat vira `offline`
   └─ data_pruner (a cada 1h): aplica as janelas de retenção
```

O ciclo **não** drena o `event_outbox` — quem faz isso é o servidor. Ver
[§9](#9-tempo-real).

Rodar o ciclo em laço dentro de um processo, em vez de um subprocesso por tique,
é o que faz as cadências internas valerem: telemetria de VPN a cada 10 s,
histórico de tráfego a cada 30 s, purga a cada hora. Elas são memória de
processo — com um processo novo por tique, todas rodariam a cada 5 s
([ADR 007](adr/007-scheduler-processo-unico.md)).

Para forçar uma passada à mão: `backend_rust-cli task scheduler_run`, que roda
**um** ciclo e sai.

Depois de executar, `next_run_at` avança a partir do horário **previsto**, não
do horário real — é o que evita deslocamento acumulado.

**Fallback local obrigatório.** Se o probe está offline, o scheduler tenta a
execução local antes de reportar `unknown`. Não remova essa tratativa: sem ela,
perder o probe significa perder o monitoramento inteiro em silêncio.

### Contrato de resultado

Todo checker devolve a mesma estrutura (`services/monitoring/contracts.rs`):
status (`up`/`down`/`warning`/`unknown`), início, fim, duração, mensagem,
métricas e dados livres. O `result_processor` grava `monitor_results`, extrai
`metrics`, recalcula o estado do dispositivo e alimenta o motor de alertas.

Os cinco checkers são `ping`, `http`, `tcp`, `dns` e `snmp`
(`services/monitoring/checkers/`).

## 6. Fila de tarefas dos probes

A fila é **persistente**, na tabela `probe_tasks` — não em memória. Quem
enfileira é o scheduler; quem entrega é a API, em `GET /api/probes/tasks`. Uma
fila em memória funciona nos testes e nunca em produção: o probe consultaria uma
fila sempre vazia e todo monitor atribuído a probe ficaria parado em `unknown`.

- Um monitor tem no máximo **uma** tarefa pendente.
- Tarefa parada além do TTL é descartada: probe que volta depois de um tempo
  fora executa uma checagem atual por monitor, não uma avalanche de vencidas.
- Probe sem heartbeat além do limite é marcado `offline` pelo watchdog
  (`services/probes/liveness.rs`).

### Ciclo de vida do probe

1. **Registro** — `backend_rust-cli task probe_register` gera o token e o imprime
   **uma única vez**; o banco guarda só o `sha256`.
2. **Autenticação** — header `X-Probe-Token` em toda requisição. Fora do guarda
   JWT: o probe não tem sessão de usuário.
3. **Heartbeat** — `POST /api/probes/heartbeat`.
4. **Tarefas** — `GET /api/probes/tasks`.
5. **Resultados** — `POST /api/probes/results`.
6. **Offline** — os resultados vão para um buffer local
   (`services/probes/buffer.rs`) e sobem quando a conexão volta.

O probe **não** recebe comando de shell. As tarefas são de tipos previamente
definidos; qualquer coisa fora do catálogo é rejeitada.

## 7. Esquema de dados

23 tabelas. A ordem de criação está em `src/models/tables.rs` (`CREATION_ORDER`)
e é a mesma das migrations — pai antes de filho, por causa das FKs. Um teste
garante que a lista e as migrations não divirjam.

```text
users  sites  probes  networks
zabbix_templates  zabbix_template_items
devices  device_interfaces  device_links
monitors  monitor_results  metrics
discovery_runs  discovery_results
alert_rules  alert_events
vpn_servers  vpn_peers
dns_servers  event_outbox  probe_tasks  system_settings
```

Notas que valem mais que a lista:

- **`devices.ip_address`** é o endereço primário, único dentro da rede
  (`unique(network_id, ip_address)`). Endereços secundários e MACs vêm das
  interfaces em `device_interfaces`.
- **`monitors.configuration`** é JSON: cada tipo de monitor tem os seus campos
  (`{"host", "packetCount"}` para ping, `{"url", "method",
  "acceptedStatusCodes"}` para HTTP).
- **Tabelas append-only** (`monitor_results`, `metrics`, `event_outbox`,
  `probe_tasks`, `zabbix_template_items`) têm só `created_at`. As duas primeiras
  são as de maior volume e recebem inserção em rajada; uma coluna de 8 bytes que
  nunca é lida, vezes milhões de linhas, é escrita jogada fora.
- **FKs anuláveis com `CASCADE`** existem de propósito (`probes.site_id`,
  `networks.site_id`, `devices.site_id`, `monitors.device_id`, as três de
  `alert_rules`). Por isso as FKs são declaradas à mão em
  `migration/src/shared.rs`, e não pelo `refs` do Loco, que derivaria a ação da
  nulabilidade.
- **`auth_tokens` não existe.** A autenticação é JWT stateless.
- **Não existe usuário semeado em produção.** Banco vazio significa instalação
  pendente: o frontend manda para `/setup` e o primeiro usuário nasce ali,
  autorizado pelo token de instalação (`SETUP_TOKEN` ou sorteado no boot e
  guardado em `system_settings`, chave `auth.setup_token`). Ver
  `services/auth/setup.rs`. As fixtures com `admin@monitor.local` valem só para
  `cargo loco db seed` em teste/desenvolvimento.
- Os índices têm **nome explícito**, nunca o que o banco derivaria.

## 8. API

Prefixo `/api`, vindo do `AppRoutes::prefix` em `src/app.rs` — não do
controller. `GET /`, `_ping` e `_health` ficam **fora** do prefixo e fora da
autenticação.

Tudo abaixo de `/api` passa pelo guarda JWT (`controllers/auth_guard.rs`), com
duas exceções: `/api/auth/*` e as rotas de agente do probe, que se autenticam
por `X-Probe-Token` dentro do handler.

`POST /api/auth/register` é a exceção da exceção: fica sob `/api/auth/*`, mas
exige sessão no próprio handler (`auth::JWT`). Cadastro aberto num sistema que
enxerga a rede inteira transformaria o token de instalação em teatro — bastaria
pular a tela de `/setup` e chamar `register`.

```text
GET  /                              identificação do serviço (health check)

GET  /api/auth/setup                a instalação ainda espera o 1º usuário?
POST /api/auth/setup                cria o 1º usuário (token de instalação)
POST /api/auth/login | /forgot | /reset | /logout
POST /api/auth/register             ← exige JWT
GET  /api/auth/me | /current | /verify/{token} | /magic-link/{token}

GET|POST         /api/sites                 GET|PUT|DELETE /api/sites/{id}
GET|POST         /api/networks              GET|PUT|DELETE /api/networks/{id}
POST             /api/networks/{id}/scan
GET|POST         /api/devices               GET|PUT|DELETE /api/devices/{id}
GET              /api/devices/{id}/monitors | /metrics | /events | /interfaces
GET|POST         /api/monitors              GET|PUT|DELETE /api/monitors/{id}
POST             /api/monitors/{id}/run | /enable | /disable
GET              /api/monitors/{id}/results | /alerts

GET  /api/discovery/scan-state | /runs | /runs/{id}
GET  /api/discovery/scan-stream            (SSE)
POST /api/discovery/scan | /scan-cancel
DELETE /api/discovery/cleanup

GET  /api/topology                         POST /api/topology/links
POST /api/topology/recalculate             DELETE /api/topology/links/{id}

GET|POST /api/probes                       GET|PUT|DELETE /api/probes/{id}
POST     /api/probes/{id}/revoke | /test
POST     /api/probes/heartbeat | /results  GET /api/probes/tasks   (X-Probe-Token)

GET|POST /api/alert-rules                  PUT|DELETE /api/alert-rules/{id}
GET      /api/alert-rules/catalog          POST /api/alert-rules/catalog
GET      /api/alerts                       POST /api/alerts/{id}/acknowledge | /silence | /verify

GET  /api/events                           GET /api/events/stream   (SSE)
GET  /api/dashboard/layout                 POST /api/dashboard/layout

POST /api/snmp/test                        POST /api/devices/{id}/snmp/scan | /poll | /apply-monitors
POST /api/port-scan
POST /api/dns/lookup | /benchmark          GET /api/dns/performance
GET|POST /api/dns-servers                  PUT|DELETE /api/dns-servers/{id}

GET|PUT  /api/vpn/server                   POST /api/vpn/server/preflight | /detect-endpoint
GET|POST /api/vpn/peers                    PATCH|DELETE /api/vpn/peers/{id}
GET      /api/vpn/peers/next-ip            GET /api/vpn/peers/{id}/config | /qrcode
POST     /api/vpn/peers/{id}/rotate | /firewall-hints

GET|POST /api/zabbix-templates             GET|DELETE /api/zabbix-templates/{id}
```

### Convenções de contrato

- **`camelCase` em todo DTO.** Não é estilo: `tests/conventions/camel_case.rs`
  falha se faltar. As duas exceções (`TopologyLinkRequest`, `TopologyQuery`)
  estão listadas lá com justificativa.
- **Erros** saem como `{"message": "..."}`, em português — é o que o frontend lê.
- **Paginação** no envelope `{data, meta}` (`services/shared/pagination.rs`), e
  não no `PaginationResponse` do Loco: o `useInfiniteList` do frontend decide o
  fim da lista por `meta.currentPage >= meta.lastPage`.
- **Modo dual**: endpoints de lista devolvem array cru sem `?page` e o envelope
  paginado com `?page`.

## 9. Tempo real

O frontend consome `GET /api/events/stream` (SSE). Eventos publicados incluem
mudança de estado de dispositivo, resultado de monitor, abertura e resolução de
alerta, conexão e desconexão de probe, dispositivo descoberto, progresso de scan
e atualização de topologia.

**Os eventos passam por `event_outbox` antes do barramento.** Quem gera o evento
é o scheduler, num processo; quem tem a conexão SSE aberta é o `server`, em
outro. Publicar direto na memória do processo que gerou faria o evento morrer ali
— o relay (`services/events/relay.rs`) é a ponte, e a tabela é o que garante que
nada se perde entre um ciclo e outro.

**O relay roda no `server`**, num laço subido pelo `MonitoringInitializer`. Tem
de ser ali: o barramento é in-process, e só o servidor tem assinantes SSE. O
relay começa com `if !bus.has_subscribers() { return }` — chamado de qualquer
outro processo, ele sai no primeiro `if` e o evento nunca chega à tela. Foi
exatamente esse o bug corrigido pela [ADR 007](adr/007-scheduler-processo-unico.md).

## 10. Módulo VPN

- O `server` gera as chaves em Rust puro (`x25519-dalek`), sem depender do
  binário `wg` e sem `NET_ADMIN`.
- A **chave privada de um peer nunca vai ao banco**: vive num cofre em memória
  até a primeira leitura. Depois disso, só rotacionando.
- Os segredos do servidor ficam cifrados em repouso com XChaCha20-Poly1305,
  chave derivada de `APP_KEY` (`services/shared/crypto.rs`).
- A telemetria do túnel é lida do arquivo `<iface>.status` no volume
  compartilhado — nunca por `docker exec`.
- O `vpn-probe` compartilha o namespace de rede do WireGuard
  (`network_mode: "service:wireguard"`) para medir ICMP/SNMP na faixa do túnel.
- O token de fallback `DEFAULT_VPN_PROBE_TOKEN` **não pode ser removido**: é ele
  que garante registro zero-config em Docker, e é a razão de `probes.token_hash`
  não ter índice único.

## 11. Segurança

- Autenticação JWT; `JWT_SECRET` **precisa ser base64 válido** (HS512). Um valor
  que não seja base64 faz todo login responder 401.
- Tokens de probe guardados como `sha256`, revogáveis.
- Credenciais e chaves privadas cifradas em repouso com `APP_KEY`. Sem `APP_KEY`
  em produção o serviço **não sobe** — é intencional.
- Validação em toda entrada; rate limit; timeout por operação; limite de
  concorrência.
- Isolamento por site: um probe só enxerga o site e as redes autorizadas.
- Nenhum comando arbitrário chega ao probe.
- Ping por socket ICMP `SOCK_DGRAM`, sem `CAP_NET_RAW` e sem `execFile('ping')`
  — ver [ADR 003](adr/003-icmp-dgram.md). O `sysctl net.ipv4.ping_group_range`
  no compose é o que habilita isso.

## 12. O que não existe

Registrar o que **não** foi construído evita que alguém procure por uma peça
ausente achando que ela está escondida:

- **Não há worker nem fila externa.** Não existe Redis, BullMQ ou equivalente.
  O scheduler executa os monitores inline e a fila dos probes é a tabela
  `probe_tasks`. Não há nenhum worker registrado em `connect_workers`, e o
  `start` roda **sem** `--server-and-worker`.
- **A dívida disso é backpressure**: uma rajada de monitores vencidos não tem
  onde ser represada. Ver a Fase 2 do [roadmap](roadmap.md).
- **Não há agregação de métricas** (rollup por hora/dia). A retenção é por
  descarte, no `data_pruner`.
- **Não há auditoria** nem permissões por papel — a autorização hoje é
  "autenticado ou não".

## 13. Decisões registradas

| ADR | Decisão |
| :--- | :--- |
| [001](adr/001-snmp-client.md) | Cliente SNMP |
| [002](adr/002-rustscan-embedding.md) | Estratégia de port scan sobre `tokio` |
| [003](adr/003-icmp-dgram.md) | Ping por socket ICMP `SOCK_DGRAM` |
| [004](adr/004-dns-wire.md) | Consultas DNS no formato wire |
| [005](adr/005-scheduler-loco.md) | Scheduler nativo do Loco, um ciclo por tique |
| [006](adr/006-prioridade-do-padrao-rust.md) | Preferir o idioma Rust ao espelhamento do código anterior |

## 14. Configuração

A configuração viva está em `backend-rust/config/{development,test,production}.yaml`.
O `.env` só preenche os `get_env(...)` desses arquivos e as substituições do
compose — [`.env.example`](../.env.example) lista todas as variáveis lidas, e só
elas. O banco é apontado por **`DATABASE_URL`**, uma URL única.

A porta é **3333** nos três ambientes: o proxy do Vite e o do nginx apontam para
ela.
