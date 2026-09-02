# Arquitetura

Descreve o sistema **como ele é hoje**. Quando este documento e o código
divergirem, o código está certo e este arquivo está desatualizado.

O backend é **Rust sobre [Loco.rs](https://loco.rs/)**, em `backend/`. As
decisões técnicas vigentes estão em [`adr/`](adr/).

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
| Banco | SQLite em WAL (produção e testes); PostgreSQL suportado por `DATABASE_URL` |
| Autenticação | JWT (`loco_rs::auth`), HS512 |
| Tempo real | SSE |
| Frontend | Vue 3 + TypeScript + Vite + Vuetify + Pinia + PWA, servido pela própria API |
| Túnel | WireGuard (`wireguard-tools` no mesmo container) |
| Implantação | Docker Compose — **um serviço** |

Não há Redis, nem fila externa, nem broker. Ver [§12](#12-o-que-não-existe).

## 3. Topologia de processos

**Um container.** Dentro dele, dois processos e uma divisão de privilégio:

| Processo | Usuário | Papel |
| :--- | :--- | :--- |
| `backend-cli start` | `app` | API HTTP na 3333, SPA na mesma porta, barramento SSE e o ciclo de monitores. Sem capability alguma. |
| `wireguard-watcher.sh` | `root` | Aplica `wg0.conf` com `wg syncconf` e publica `wg0.status`. É quem usa o `NET_ADMIN`. |

```text
┌──────────────────────────── container netmonitor ────────────────────────────┐
│                                                                              │
│   :3333 ──▶ SPA (/app/web)  ─┐                                               │
│             API (/api/*)    ─┤  backend-cli start   (usuário `app`)     │
│             SSE             ─┤    └─ ciclo de monitores a cada 5 s           │
│                              │    └─ relay do event_outbox                   │
│                              └────────────┬──────────────┐                   │
│                                           │ wg0.conf     │ SQLite (WAL)      │
│                                           ▼              ▼                   │
│                              wireguard-watcher.sh    /data/netmonitor.sqlite │
│                              (root, NET_ADMIN)                               │
│                                     │  ▲                                     │
│                                     ▼  │ wg0.status                          │
│   :51820/udp ◀────────────────── wg0 ──┘                                     │
└──────────────────────────────────────────────────────────────────────────────┘
```

Eram oito serviços. O que aconteceu com cada um:

| Serviço | Para onde foi |
| :--- | :--- |
| `migration` | `auto_migrate` no boot do servidor. `create_app` migra, `run_task` não — um probe remoto continua sem tocar no esquema. |
| `scheduler` | Laço in-process (`initializers::monitoring`). O [ADR 007](adr/007-scheduler-processo-unico.md) exige um processo longevo, não um container próprio — e no mesmo processo os eventos do ciclo nascem onde estão as conexões SSE. |
| `probe` (LAN) | Redundante: enxergava exatamente a mesma rede que o servidor. Monitores sem `probe_id` rodam no próprio servidor. |
| `vpn-probe` | A `wg0` passou a ser do próprio processo. Não é mais registrado no boot, e os monitores da VPN rodam locais. |
| `wireguard` | O watcher virou um processo deste container. |
| `frontend` | A SPA é servida pela API (`src/spa.rs`): mesma porta, mesma origem, sem proxy e sem CORS. |
| `postgres` | SQLite em WAL no volume. |

**O processo da API continua sem executar `wg` e sem `docker exec`.** Ele
escreve `<iface>.conf` e lê `<iface>.status` num diretório combinado
(`WG_CONFIG_DIR`); quem aplica é o watcher. O que mudou foi a distância entre os
dois — de container para processo vizinho —, não o contrato. O entrypoint chama
a aplicação com `setpriv --inh-caps=-all`: o `NET_ADMIN` concedido ao container
não chega a ela.

Quando o módulo Docker está habilitado, a API acessa a **Docker Engine API**
diretamente pelo socket montado, via `bollard`; não chama `docker`, `docker exec`
nem shell. Essa montagem é uma autoridade separada das capabilities Linux e
equivale a administração do host. Por isso mutações e exportação de volume são
exclusivas de `admin` e auditadas. A decisão e as consequências estão na
[ADR 010](adr/010-docker-engine-api.md).

### Estáticos

O `dist` da SPA é copiado para `/app/web` e servido pelo `ServeDir`:

- `/assets/*` — nomes com hash, `Cache-Control: public, max-age=31536000, immutable`;
- o resto (`index.html`, `sw.js`, ícones) — `no-cache`, isto é, guarde mas
  revalide. Um service worker servido de cache trava a versão do app no
  navegador;
- o `.gz` de cada arquivo é gerado **no build da imagem** e entregue pelo
  `precompressed_gzip` a quem aceita gzip. Comprimir por request gastaria CPU
  para produzir sempre o mesmo byte.

Rota virtual do Vue Router funciona pelo `fallback` do `ServeDir` para o
`index.html`. Rotas registradas vencem o fallback, então `/api/*` nunca é
confundido com arquivo — e é por isso que a identificação do serviço saiu de
`GET /` para `GET /api/info`.

### Modos de instalação

- **Standalone** — `docker compose up -d --build`. Um container, um volume.
- **Central com probes remotos** — a mesma imagem em outro site, com
  `backend-cli task probe_run` e `PROBE_SERVER_URL` apontando para o
  servidor. O probe fala por HTTP autenticado; **nunca** acessa o banco.
- **Central com túnel** — os equipamentos remotos chegam pela `wg0` do próprio
  container e são medidos direto.
- **Instalação grande** — `DATABASE_URL` para um Postgres externo e, se o ciclo
  de monitores começar a disputar CPU com a API, `SCHEDULER_ENABLED=false` no
  servidor mais um container só com `task scheduler_loop`.

## 4. Organização do código

```text
backend/
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
├── devices/        adapters de plataforma, acesso, capacidades e dispositivo do sistema
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
├── backup/         export/restore das configurações
└── shared/         crypto, pagination, errors
```

### Adapters de dispositivo

Toda variação por sistema operacional ou família de equipamento parte de
`services/devices/adapters/`. O contrato `DeviceAdapter` concentra identidade,
evidências de detecção, meios de acesso, classificação e vínculos com adapters
especializados. O registro define a ordem pública e é consumido por cadastro,
discovery, Syslog e VPN.

O Syslog usa `SyslogConfigurationAdapter` para gerar comandos, identificar o
equipamento, emitir a linha de teste e interpretar extensões de dialeto. A VPN
usa `VpnProfileGenerator` para gerar artefatos e regras de firewall. O frontend
não possui enum/mapa próprio dessas plataformas: usa os cards e capacidades da
API. Ver [ADR 009](adr/009-device-adapters.md).

`devices/capabilities.rs` tem outra responsabilidade: deriva capacidades já
comprovadas por interfaces, métricas, eventos e logs persistidos. Ela não
confunde “a plataforma suporta” com “este equipamento respondeu”.

## 5. O ciclo de monitoramento

É o coração do sistema. O processo `scheduler` roda `run_cycle` em laço, uma vez
a cada `SCHEDULER_INTERVAL_SECONDS` (padrão 5 s), em
`src/tasks/scheduler_run.rs`:

```text
run_cycle (a cada 5 s, no mesmo processo)
   │
   ├─ monitores vencidos (next_run_at <= now)
   │     ├─ lote de até 50, com no máximo 16 execuções concorrentes
   │     ├─ monitor tem probe? → enfileira em `probe_tasks`
   │     └─ não tem, ou o probe está offline? → executa local (`run_monitor`)
   │
   ├─ discovery: agenda redes vencidas e despacha uma run local em background
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

Para forçar uma passada à mão: `backend-cli task scheduler_run`, que roda
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

### Guarda adaptativa de latência externa

Antes de avaliar qualquer regra baseada em latência, o
`alerts/adaptive_latency` classifica monitores HTTP/HTTPS, DNS, TCP e ping como
internos ou externos. Em modo automático, destinos SaaS, FQDNs públicos e IPs
públicos usam a guarda; redes privadas, nomes locais e demais tipos preservam
as regras fixas. O operador pode forçar `adaptive` ou `fixed` em
`configuration.latencyAlertPolicy`.

Para um destino externo, o limiar efetivo é o maior entre a média histórica
mais o percentual configurado, a média mais um aumento absoluto mínimo e a
banda estatística de 3σ. O alerta só é liberado após X resultados consecutivos
acima desse limiar; a sequência é reconstruída de `monitor_results`, portanto
reiniciar o processo não apaga confirmações. Essa confirmação substitui a
janela temporal da regra para os fatos de latência, evitando somar duas esperas.

Cada resultado da sequência é cruzado com `inBps` e `outBps` da interface
declarada como WAN/Uplink. A capacidade contratada de download/upload tem
precedência; sem ela, usa-se a velocidade negociada da interface. Uma amostra
acima do percentual de saturação interrompe a sequência, porque a elevação de
latência é explicada pelo uso normal do link. O sistema só infere a origem
quando encontra uma única WAN compatível no site; ambiguidade ou falta de
telemetria nunca é inventada como saturação e não silencia o alerta.

A guarda remove apenas fatos de tempo (`latencyMs`, desvio, Z-Score, conexão e
resolução) durante aprendizado, confirmação ou saturação. Indisponibilidade,
perda de pacotes, erro HTTP e falha DNS continuam independentes. O diagnóstico
completo segue em `alert_events.data.adaptiveLatency`, em
`GET /api/monitors/:id/baseline` e no payload SSE `monitor:result`. A tela faz
uma leitura HTTP inicial e depois atualiza a decisão diretamente em memória pelo
stream compartilhado, sem polling nem refetch por resultado.

### Confirmação de alcance quando o ICMP não responde

O checker de ping continua responsável por uma tentativa ICMP. A camada
`monitoring/ping_diagnostics` coordena `1 + retry_count` tentativas e, somente
quando todas terminam com perda de 100%, testa no máximo três portas TCP. As
portas vêm primeiro de monitores TCP habilitados do mesmo dispositivo e
`probe_id`, depois do discovery mais recente para o mesmo IP e origem. Essa
lista segue numa configuração transitória `_diagnostics`: ela chega à execução
local, manual e ao probe remoto, mas nunca é salva em `monitors.configuration`.

Uma conexão aceita (`open`) ou recusada por RST (`closed`) prova que o host
responde e converte o resultado em `warning`, com
`reachabilityCause: icmp_filtered`. Timeout, filtragem, rota inalcançável e
erros permanecem inconclusivos e mantêm o resultado `down`; ausência de
resposta TCP nunca é apresentada como certeza de bloqueio. As sondagens usam
TCP `connect`, em paralelo, com teto de 1,5 s e uma repetição apenas para
silêncio. Como `_diagnostics` acompanha a tarefa do probe, ICMP e TCP são
observados do mesmo ponto da rede.

## 6. Fila de tarefas dos probes

A fila é **persistente**, na tabela `probe_tasks` — não em memória. Quem
enfileira é o scheduler; quem entrega é a API, em `GET /api/probes/tasks`. Uma
fila em memória funciona nos testes e nunca em produção: o probe consultaria uma
fila sempre vazia e todo monitor atribuído a probe ficaria parado em `unknown`.

Discovery remoto não cria monitor fictício em `probe_tasks`: a própria linha
`discovery_runs` com `probe_id` e status `pending` é a fila persistente. O
endpoint entrega essas linhas em `discoveryTasks`; o agente executa os mesmos
scanners puros, devolve `discoveryResults` e somente o servidor central grava
`discovery_results`. Resultados de monitor e discovery usam buffers offline
separados no probe.

- Um monitor tem no máximo **uma** tarefa pendente.
- Tarefa parada além do TTL é descartada: probe que volta depois de um tempo
  fora executa uma checagem atual por monitor, não uma avalanche de vencidas.
- Probe sem heartbeat além do limite é marcado `offline` pelo watchdog
  (`services/probes/liveness.rs`).

### Ciclo de vida do probe

1. **Registro** — `backend-cli task probe_register` gera o token e o imprime
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

### Varredura de rede

- CIDRs IPv4 e IPv6 são expandidos em lotes de 1.024 até o fim, sem corte.
- ICMP não é pré-condição: TCP e SNMP/UDP são testados em todos os IPs do lote.
- O scanner TCP usa `tokio::net::TcpStream`, sem raw socket/capability, com
  limite global e por host, retries transitórios e perfis de carga.
- O cliente SNMP usa `async-snmp`: transporte UDP compartilhado, correlação de
  origem/request ID, GETBULK com fallback GETNEXT e SNMPv3 USM com cache de
  engine e chaves derivadas.
- Credenciais automáticas de discovery vêm apenas de
  `SNMP_DISCOVERY_COMMUNITIES` e `SNMP_DISCOVERY_V3_PROFILES`; nunca são
  persistidas dentro do resultado descoberto.

## 7. Esquema de dados

21 tabelas. A ordem de criação está em `src/models/tables.rs` (`CREATION_ORDER`)
e é a mesma das migrations — pai antes de filho, por causa das FKs. Um teste
garante que a lista e as migrations não divirjam.

```text
users  sites  probes  networks
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
  `probe_tasks`) têm só `created_at`. As duas primeiras
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

GET|POST         /api/users                 GET|PUT|DELETE /api/users/{id}

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
GET  /api/backup/export                    POST /api/backup/preview | /restore

POST /api/snmp/test                        POST /api/devices/{id}/snmp/scan | /poll | /apply-monitors
POST /api/port-scan
POST /api/dns/lookup | /benchmark          GET /api/dns/performance
GET|POST /api/dns-servers                  PUT|DELETE /api/dns-servers/{id}

GET  /api/docker/status | /metrics
GET  /api/docker/containers | /containers/{id} | /containers/{id}/logs
POST /api/docker/containers/{id}/start | /stop | /restart
DELETE /api/docker/containers/{id}
GET  /api/docker/volumes | /volumes/{name} | /volumes/{name}/export
DELETE /api/docker/volumes/{name}
GET|POST /api/docker/networks              GET|DELETE /api/docker/networks/{id}
POST /api/docker/networks/{id}/connect | /disconnect
GET  /api/docker/images | /images/{id}     DELETE /api/docker/images/{id}
POST /api/docker/images/prune

GET|PUT  /api/vpn/server                   POST /api/vpn/server/preflight | /detect-endpoint
GET|POST /api/vpn/peers                    PATCH|DELETE /api/vpn/peers/{id}
GET      /api/vpn/peers/next-ip            GET /api/vpn/peers/{id}/config | /qrcode
POST     /api/vpn/peers/{id}/rotate | /firewall-hints
```

### Backup de configuração

`GET /api/backup/export` devolve um JSON com as 12 tabelas de configuração
(`services::backup::service::BACKED_UP_TABLES`). Ficam de fora a telemetria — é
histórico, cresce sem limite e volta a ser produzida no ciclo seguinte — e
`users`, porque conta de acesso não é configuração e restaurá-la trocaria as
credenciais de quem está operando.

`POST /api/backup/restore` apaga a configuração atual e o histórico que depende
dela, e recarrega o arquivo **preservando os ids**. Preservar o id não é
detalhe: `monitors`, `alert_rules`, `device_links` e `vpn_peers` guardam FKs, e
renumerar significaria reescrever cada referência — inclusive as que vivem
dentro de JSON. Tudo roda em uma transação; no PostgreSQL as sequências são
realinhadas no fim, senão o próximo cadastro feito pela tela colidiria com um id
restaurado.

O arquivo carrega `probes.token_hash`, `devices.snmp_community` e as chaves da
VPN cifradas com a `ENCRYPTION_KEY` da instalação — restaurar em outra
instalação só devolve VPN funcional com a mesma chave.


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

O módulo Docker não usa polling no navegador. Um único produtor no processo
`server` coleta estado e métricas em ciclo curto e inventário em ciclo mais
espaçado, publicando `docker:snapshot` e `docker:inventory` no mesmo stream SSE.
O ciclo só consulta a Engine quando `EventBus::has_subscribers()` é verdadeiro;
quantas abas estiverem abertas recebem a mesma amostra. Mutações administrativas
forçam os dois snapshots imediatamente. O dashboard não renderiza seu resumo
quando o snapshot informa que a Engine está indisponível.

As métricas Docker seguem a mesma semântica do CLI: CPU nasce do delta entre
duas amostras e memória representa o working set (`usage - inactive_file` no
cgroup v2; `usage - total_inactive_file` no v1). Falhas isoladas de coleta são
marcadas como snapshot parcial, sem fazer a Engine inteira parecer offline.

As séries numéricas de um equipamento viajam no próprio `monitor:result`.
O frontend aplica `metrics` diretamente nas stores e oferece localmente o alias
`metric:recorded` aos gráficos; não existe segundo evento durável nem refetch.
Memória publica o trio `memory_usage`, `memory_used_bytes` e
`memory_total_bytes`, sempre medido pela fonte, nunca inferido pela interface.
Na apresentação, `memory_used_bytes` e `memory_total_bytes` são os valores
principais; `memory_usage` fica restrito ao contexto secundário, às cores e aos
limiares percentuais de alerta.

Telemetria Docker é efêmera e vai direto ao barramento em memória: persistir
amostras de três em três segundos no `event_outbox` faria o banco crescer sem
valor histórico. Os eventos de domínio duráveis descritos abaixo continuam
passando pelo outbox para atravessar processos.

**Os eventos de domínio duráveis passam por `event_outbox` antes do barramento.** Quem gera o evento
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
  chave derivada de `ENCRYPTION_KEY` (`services/shared/crypto.rs`).
- A telemetria do túnel é lida do arquivo `<iface>.status` escrito pelo watcher
  — nunca por `docker exec`.
- A `wg0` sobe no namespace de rede do próprio container, então ICMP e SNMP na
  faixa do túnel saem direto do processo da API. `VPN_PROBE_EXTERNAL=true`
  habilita o arranjo externo — túnel em outro namespace, medido por um
  `vpn-probe` dedicado — e é o que reativa o registro do agente no boot e o
  aviso `pingOutsideTunnel`.
- O token de fallback `DEFAULT_VPN_PROBE_TOKEN` **não pode ser removido**: é ele
  que garante registro zero-config em Docker, e é a razão de `probes.token_hash`
  não ter índice único.

## 11. Segurança

- Autenticação JWT; `JWT_SECRET` **precisa ser base64 válido** (HS512). Um valor
  que não seja base64 faz todo login responder 401.
- Tokens de probe guardados como `sha256`, revogáveis.
- Credenciais e chaves privadas cifradas em repouso com `ENCRYPTION_KEY`. Sem
  ela em produção o serviço **não sobe** — é intencional.
- Validação em toda entrada; rate limit; timeout por operação; limite de
  concorrência.
- Isolamento por site: um probe só enxerga o site e as redes autorizadas.
- Nenhum comando arbitrário chega ao probe.
- Ping por socket ICMP `SOCK_DGRAM`, sem `CAP_NET_RAW` e sem `execFile('ping')`
  — ver [ADR 003](adr/003-icmp-dgram.md). O `sysctl net.ipv4.ping_group_range`
  no compose é o que habilita isso.
- Autorização por perfil centralizada no guarda das rotas de negócio:
  `admin` tem acesso total e gerencia usuários; `operator` lê e escreve os
  recursos operacionais; `viewer` possui somente leitura. Contas inativas são
  recusadas em toda requisição e o último administrador ativo é protegido.
- O socket Docker é opt-out (`DOCKER_ENABLED=false` mais remoção da
  montagem). Leitura de inventário não expõe valores de variáveis cujo nome
  indica senha, token, segredo, chave privada ou credencial. Controle da Engine
  e exportação de volumes exigem `admin` e geram auditoria.

## 11-A. O próprio NetMonitor como dispositivo

O servidor é um dispositivo de primeira classe: aparece na lista, tem monitores,
regras, métricas, eventos e logs pelos **mesmos** fluxos de qualquer roteador.
Nenhuma rota, tabela, store ou tela existe só para ele.

```text
coletor de saúde local (services/monitoring/health/)
        │
        ▼
monitor gerenciado `system_health`  (services/monitoring/managed.rs)
        │
        ▼
  process_result ──▶ monitor_results          (série da checagem)
        │        └─▶ metrics                  (série do dispositivo)
        │        └─▶ motor existente de alert_rules
        │                    │
        │                    ▼
        │                alert_events
        │
tracing da aplicação ──▶ LogQueue existente ──▶ writer em lote existente
                                                       │
                                                  device_logs
                                                       │
                                       API/SSE de logs existente
                                                       │
                                       /logs e aba Logs do dispositivo
```

**A identidade é `devices.system_key`**, coluna anulável com índice único
(`netmonitor`). Nunca por ID, nome, IP, site ou rede: o ID varia por instalação,
o nome é editável e os demais podem ser nulos.
`services::devices::system_device` garante a linha num `Initializer` — não em
`after_context`, porque as migrations do banco principal só convergem depois do
`create_context` — e publica o ID num cache de processo para os caminhos
quentes. Uma restauração de backup **invalida o cache e reexecuta o serviço**:
o `wipe` + recarga devolve as linhas com os IDs do arquivo, e um ID cacheado
passaria a apontar para outro equipamento.

**As duas séries, e por que não são a mesma.** `monitor_results` guarda o
desfecho de *uma checagem* (status, duração, latência) por 14 dias;
`metrics` guarda a grandeza contínua *do dispositivo* por 30. Latência e perda
de pacote ficam apenas na primeira: copiá-las a cada ciclo multiplicaria a
tabela de maior volume do sistema sem acrescentar informação. A lista fechada
do que vira série de dispositivo está em
`monitoring::result_processor::DEVICE_SERIES`.

**Os dois vocabulários.** `metrics.name` usa `cpu_usage`, `memory_usage`,
`storage_usage`, `load_average_1m`, `process_memory_bytes`, `uptime_seconds`,
`inBps`, `outBps`. `condition.field` (a regra de alerta) usa camelCase:
`cpuUsagePercent`, `memoryUsedPercent`, `storageUsedPercent`, `loadAverage1m`.
O `METRIC_FIELD_MAP` de `alerts/datasets/monitor_result.rs` é o **único** ponto
de tradução. As chaves atravessam para o frontend por `ts-rs`
(`dtos/alerts.rs` → `bindings/AlertField.ts`), então renomear um campo no Rust
quebra o `typecheck` do frontend em vez de apagar um rótulo em silêncio.

**As capacidades governam a tela.** `GET /api/devices/{id}/capabilities`
(`services/devices/capabilities.rs`) responde o que existe para aquele
dispositivo, e a mesma projeção decide **abas e botões**. Toda capacidade nasce
de evidência persistida — uma interface inventariada, uma métrica gravada, um
evento registrado. `devices.snmp_enabled` é intenção de cadastro, não prova de
conexão: o estado "configurado, mas ainda não conectado" vira uma ação na Visão
Geral, não uma aba vazia.

**Retenção: uma disputa aceita, de propósito.** `retention::prune` corta o banco
de logs por idade *e* por tamanho (4 GB, mais antigo primeiro). Com o log da
aplicação gravando em `device_logs`, ele **disputa esse orçamento** com o syslog
do parque: um `DEBUG` ligado empurra log de roteador para fora, e vice-versa. A
decisão é aceitar a disputa — cota por origem custaria mais complexidade do que
resolve. Quem precisar de mais espaço para o syslog abaixa o nível do
`config.logger`; a coluna `device_logs.source` permite medir a proporção antes
de decidir.

**Memória dos históricos é limitada pela cardinalidade da resposta.** A coleta
SNMP consulta o último contador de cada `(interface, série)` por lookup indexado,
sem materializar o passado. Séries para gráficos percorrem cursores e mantêm
somente acumuladores fixos; o histórico DNS entrega no máximo 720 pontos por
série. O rollup de `monitor_results` agrega no banco em janelas de até sete dias
por ciclo e o relay SSE lê no máximo 500 linhas do outbox por passagem. Como o
banco padrão de produção é SQLite, seu pool também usa uma conexão por padrão;
instalações PostgreSQL dimensionam `DB_MAX_CONNECTIONS` explicitamente.

## 12. O que não existe

Registrar o que **não** foi construído evita que alguém procure por uma peça
ausente achando que ela está escondida:

- **Não há worker nem fila externa.** Não existe Redis, BullMQ ou equivalente.
  O scheduler executa diretamente lotes limitados de monitores, com concorrência
  máxima de 16, e a fila dos probes é a tabela `probe_tasks`. O discovery local
  é uma task Tokio no mesmo processo, não um worker. Não há nenhum worker
  registrado em `connect_workers`, e o `start` roda **sem**
  `--server-and-worker`.
- **O backpressure é local e limitado**: cada ciclo recolhe até 50 monitores e
  executa até 16 ao mesmo tempo; atrasos além desse lote ficam representados por
  `next_run_at` no banco para os ciclos seguintes.
- **Não há rollup da tabela `metrics`.** Seus contadores SNMP/VPN continuam com
  retenção por descarte no `data_pruner`; os resultados de monitoramento da
  tabela `monitor_results` têm rollup horário em `monitor_results_hourly`.
- **Não há segundo pipeline de log dentro do processo.** O log da aplicação usa
  a mesma fila limitada, o mesmo escritor em lote e o mesmo barramento do
  syslog; a camada de `tracing` (`syslog/app_layer.rs`) só monta o
  `PendingLog`. Não existe `runtime_logs`, `runtime_metrics` nem
  `/api/runtime/*`.
- **Não há observador externo do processo.** Um processo parado não consegue
  alertar sobre si: monitorar a queda total do NetMonitor exigiria um segundo
  agente, e isso está fora de escopo.
- **Não há trilha de auditoria.** Há autorização por papel (`admin`, `operator`
  e `viewer`), mas ainda não existe registro histórico de quem alterou cada
  recurso.

## 13. Decisões registradas

| ADR | Decisão |
| :--- | :--- |
| [001](adr/001-snmp-client.md) | Cliente SNMP |
| [002](adr/002-rustscan-embedding.md) | Estratégia de port scan sobre `tokio` |
| [003](adr/003-icmp-dgram.md) | Ping por socket ICMP `SOCK_DGRAM` |
| [004](adr/004-dns-wire.md) | Consultas DNS no formato wire |
| [007](adr/007-scheduler-processo-unico.md) | Scheduler em laço no processo principal |
| [008](adr/008-syslog-parser.md) | Parser e ingestão de Syslog |
| [009](adr/009-device-adapters.md) | Adapters extensíveis por plataforma de dispositivo |

## 14. Configuração

A configuração viva está em `backend/config/{development,test,production}.yaml`.
O `.env` só preenche os `get_env(...)` desses arquivos e as substituições do
compose — [`.env.example`](../.env.example) lista todas as variáveis lidas, e só
elas. O banco é apontado por **`DATABASE_URL`**, uma URL única.

Interface web e API dividem a porta configurada por `APP_PORT` em produção. Na
bridge, `APP_EXTERNAL_PORT` escolhe a porta publicada; no modo host, `APP_PORT`
é acessada diretamente. Em desenvolvimento o Vite serve a SPA na 5173 e faz
proxy de `/api` para a 3333.
