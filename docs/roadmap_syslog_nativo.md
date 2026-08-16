# Roadmap — Servidor de Syslog Nativo

Receber logs de roteadores (RouterOS, OpenWRT, Linux, Ubiquiti) dentro do
processo que já existe, vincular aos `devices`, consultar pela SPA e virar
alerta. Alvo: 30–40 dispositivos, ~12 msg/s típico, ~200 msg/s de pico.

**Veredito da análise**: a abordagem in-process com banco separado está certa —
200 msg/s ocupa ~0,3% da capacidade de escrita de um SQLite em WAL, e Loki ou
Elasticsearch seriam infraestrutura cara para sustentar uma tabela. Seis pontos
do desenho original mudaram; estão marcados com **⚠** onde aparecem.

---

## 1. Decisões

| decisão | motivo |
|---|---|
| Escuta em **5514**, `514:5514` publicado | o processo roda com `CapEff` zerado (`setpriv --inh-caps=-all`) |
| Banco separado `/data/logs.sqlite` | o `DELETE` de ~1 M linhas da retenção segura o *write lock* e congelaria o scheduler |
| `syslog_loose` 0.23.0 (MIT, `nom 8` interno) | não devolve `Result` — linha ruim vira "tudo é mensagem" |
| `received_at` é a verdade | RFC 3164 vem sem ano e sem fuso |
| Fonte que não resolve não grava | um host solto enche o disco |
| Só `LIKE` na busca | FTS5 é do SQLite; PostgreSQL precisa valer igual |
| ⚠ **`LogBus` dedicado**, não o `EventBus` | o `EventBus` é `broadcast` de 1024 compartilhado com o SSE do dashboard; 12 msg/s rola o anel em 85 s e o painel perde `monitor:result` |
| ⚠ Fila cheia descarta o **mais novo** | `mpsc::try_send` devolve o mais novo; descartar o mais antigo exigiria ring próprio. Contador exposto do mesmo jeito |
| ⚠ `from` padrão de 24 h, janela com teto | `LIKE '%q%'` sem janela varre ~1,5 GB em 7 M de linhas |
| ⚠ `RETENTION_LOGS_MAX_MB=4096` | medido: 290 B/linha com os 3 índices → ~301 MB/dia, ~2,1 GB em 7 dias. 2048 MB ficaria no limite; o teto folgado só custa disco quando o disco é usado |
| ⚠ Regra de log casa **na ingestão** | regex sobre janela não usa índice; na ingestão são ~120 regex/s |
| ⚠ IP de origem verificado no spike | se o Docker mascarar a origem, nada é gravado e não há erro visível |

---

## 2. O que será criado

### Backend

```
src/services/syslog/
  mod.rs
  listener.rs      UDP + TCP:5514, RFC 6587 (octet-counting e LF)
  parser.rs        syslog_loose + tópicos do RouterOS + resolvedor de ano
  resolver.rs      source_ip → device; fallback hostname; fallback CIDR
  queue.rs         mpsc(10k), try_send, contadores
  writer.rs        lote de 500 linhas ou 200 ms
  bus.rs           LogBus (broadcast dedicado)
  sources.rs       fontes vistas (conhecidas, desconhecidas, ambíguas)
  snippets.rs      comandos por fabricante
  repository.rs    consulta com cursor de keyset
  matcher.rs       Fase 6 — regex na ingestão
src/controllers/logs.rs
src/models/logs/           entidades escritas à mão (não geradas)
src/dtos/logs.rs           camelCase
migration/src/logs/        LogsMigrator próprio
examples/spikes/syslog_parse.rs
```

### Frontend

```
src/pages/LogsPage.vue
src/stores/logs.ts
src/composables/useInfiniteCursor.ts
src/components/logs/LogTable.vue, LogFilters.vue, SyslogSetupCard.vue
```

Aba **Logs** no `DeviceDetailPage.vue`, já filtrada.

### Tabela `device_logs`

Append-only (helper `append_only`, só `created_at`/`received_at`):

```
id · device_id (null, sem FK) · source_ip · received_at · device_time (null)
facility (null) · severity (null) · hostname (null) · app_name (null)
pid (null) · topics (null) · message
```

`topics` é texto vírgula-separado, não JSON — `LIKE` vale nos dois dialetos.
Payload cru só com `SYSLOG_STORE_RAW=true`.

Índices:

```
idx-device_logs-device-received     (device_id, received_at)
idx-device_logs-received            (received_at)
idx-device_logs-severity-received   (severity, received_at)
```

### API

```
GET  /api/logs                     deviceId, severity, facility, from, to, q, cursor, limit
GET  /api/logs/stream              SSE, live tail filtrado
GET  /api/logs/sources             fontes vistas
POST /api/logs/sources/{ip}/bind   vincula IP a device
GET  /api/logs/setup-snippet       comandos por fabricante
```

Sob `business_auth`. Envelope de cursor:

```json
{ "data": [...], "meta": { "nextCursor": "…", "hasMore": true, "limit": 100 } }
```

⚠ `useInfiniteList` decide fim de lista por `currentPage >= lastPage` e cursor
não tem `lastPage` — daí o `useInfiniteCursor.ts`. Não fabricar página falsa.

### Configuração

```bash
SYSLOG_ENABLED=true
SYSLOG_UDP_PORT=5514
SYSLOG_TCP_PORT=5514
SYSLOG_DB_URL=sqlite:///data/logs.sqlite?mode=rwc
SYSLOG_MAX_MSG_BYTES=8192
SYSLOG_RATE_LIMIT_PER_SOURCE=50    # msgs/s, rajada 200 (a 200/s uma fonte come o pico global)
SYSLOG_QUEUE_CAPACITY=10000
SYSLOG_ACCEPT_UNKNOWN_SOURCES=false
SYSLOG_STORE_RAW=false
RETENTION_LOGS_DAYS=7
RETENTION_LOGS_MAX_MB=4096
```

`docker-compose.yml`:

```yaml
- "${SYSLOG_EXTERNAL_PORT:-514}:5514/udp"
- "${SYSLOG_EXTERNAL_PORT:-514}:5514/tcp"
```

---

## 3. Armadilhas de implementação

**Registro no Loco** — `run_task` não executa `Initializer`:

| peça | gancho |
|---|---|
| conexão + migrations do banco de logs, `LogBus` | `after_context` (o pruner roda no `scheduler`) |
| listener, escritor em lote | `Initializer` (só servidor) |

**Ordem dos PRAGMAs**: `auto_vacuum = INCREMENTAL` → `journal_mode = WAL` →
migrations. `auto_vacuum` é propriedade do arquivo e só pega com o banco vazio;
inverter é erro silencioso que só aparece meses depois, em produção.

**Retenção**: dias e tamanho, o que vencer primeiro. Tamanho é dialeto-específico
(`page_count × page_size` / `pg_total_relation_size`) — mesmo `match` que
`tables::existing_tables` usa. Corte por tamanho apaga em blocos de 10 k, com
teto de iterações. Depois: `PRAGMA incremental_vacuum` e
`wal_checkpoint(TRUNCATE)` — `DELETE` sozinho não devolve disco.

**Os PRAGMAs de vacuum vão por `execute_unprepared`, não `query_one_raw`.**
Medido na Fase 2: o `query_one_raw` usa `fetch_optional` e para no primeiro
passo do statement, enquanto o `incremental_vacuum` devolve **uma página por
passo**. Com 563 páginas livres ele devolvia uma, retornava `Ok` e não
registrava nada no log. `DELETE ... LIMIT` também não serve: não existe no
PostgreSQL e depende de flag de compilação no SQLite — selecione os ids do bloco
e apague por `IN`.

**IP não é único em `devices`**: o índice único é `(network_id, ip_address)`;
dois dispositivos em redes diferentes podem ter `192.168.1.1`. Desempata pelo
CIDR da `network`; empate remanescente vira fonte **ambígua** esperando bind
manual. Vincular errado contamina a aba do aparelho e, na Fase 6, alerta no alvo
errado.

**Sem `JOIN` entre bancos**: `GET /api/logs` faz duas consultas — página no banco
de logs, nomes por `IN (…)` na base principal, junção no *view*. Apagar `device`
não cascateia; as órfãs saem pela retenção normal.

**`insert_many`**: 500 linhas × 12 colunas = 6 000 parâmetros. Cabe no SQLite
(32 766) e no PostgreSQL (65 535). Não subir para 5 000 sem refazer a conta.

**Entidades à mão**: `cargo loco db entities` aponta para a base principal.
Inteiros conforme o PostgreSQL (`i32`/`i64`), nunca conforme o SQLite reporta.

**`bsd-syslog=yes` é recomendado, não obrigatório**: sem a flag o RouterOS manda
formato próprio, sem timestamp e sem hostname. O resgate do `<pri>` recupera
severidade, tópicos e mensagem — perde-se só `device_time` e `hostname`
(medido, ADR 008).

**A severidade do RouterOS vem dos tópicos**, não do `<pri>` — que carrega o
`syslog-severity` fixo da *action*. Vence a mais grave entre `emergency`,
`alert`, `critical`, `error`, `warning`, `info` e `debug`; sem palavra de
severidade nos tópicos, vale o `<pri>`.

---

## 4. Fases

### Fase 1 — Spike 🟢 Concluída

Resultados em [ADR 008](adr/008-syslog-parser.md).

- [x] `Cargo.toml`: `syslog_loose = "0.23"` + entrada `[[example]]`
- [x] `examples/spikes/syslog_parse.rs` com amostras reais de RouterOS (com e
      sem `bsd-syslog=yes`), OpenWRT, Linux e Ubiquiti
- [x] Extração dos tópicos do RouterOS — caem no lugar do `tag` do BSD, sem
      parser próprio
- [x] Resolvedor de ano escolhendo o mais próximo de `received_at` (virada
      31/dez, nas duas direções)
- [x] Medir inserção em lote — **85 228 linhas/s** em debug, **290 B/linha**
      com os 3 índices; 0,23% do tempo a 200 msg/s
- [ ] Medir o IP de origem observado dentro do container, com pacote de outra
      máquina — **em aberto**, exige a imagem de produção. Procedimento na
      ADR 008; é o item de maior risco do projeto
- [x] `docs/adr/008-syslog-parser.md`

Duas descobertas que mudam a Fase 2:

- **Resgate do `<pri>`**: sem `bsd-syslog=yes` o RouterOS manda formato próprio
  e o `syslog_loose` joga a linha inteira em `msg`, perdendo a severidade.
  Decompor o `<n>` à mão e reparsear o resto recupera tudo menos `device_time`
  e `hostname`.
- **Severidade vem dos tópicos**: o `<pri>` do RouterOS carrega o
  `syslog-severity` fixo da *action* (`info` para tudo, nas versões sem
  `auto`). A severidade real está em `system,error,critical`. Sem isso, filtrar
  por severidade não separaria nada num parque MikroTik.

### Fase 2 — Ingestão 🟢 Concluída

- [x] `migration/src/logs/` + `LogsMigrator` (tabela + 3 índices nomeados)
- [x] Entidade `device_logs` escrita à mão
- [x] Conexão, PRAGMAs na ordem certa e migrations em `after_context`
- [x] `parser.rs`
- [x] `resolver.rs` (IP → hostname → CIDR → ambígua/desconhecida)
- [x] `queue.rs` (mpsc 10k, `try_send`, contadores, limitador por fonte)
- [x] `writer.rs` (500 linhas ou 200 ms)
- [x] `listener.rs` UDP + TCP com as duas molduras da RFC 6587
- [x] `sources.rs` — fontes vistas (a API delas é da Fase 4; o contador é da
      ingestão, senão a regra da fonte desconhecida descarta em silêncio)
- [x] `Initializer` do listener, com a trava por **ambiente de teste** — não por
      variável: `request_with_config` roda os initializers, e depender de
      lembrar de `SYSLOG_ENABLED=false` em cada teste deixaria a suíte colidindo
      na 5514
- [x] Retenção (dias + tamanho, `incremental_vacuum`,
      `wal_checkpoint(TRUNCATE)`), ligada ao ciclo do `scheduler`
- [x] `docker-compose.yml`: `514:5514/udp` e `/tcp` + variáveis
- [x] Testes: 53 unitários + 5 de integração em porta 0

Três defeitos que os testes pegaram, todos com o mesmo formato — "parece que
funcionou, mas não fez nada":

- **`PRAGMA incremental_vacuum` devolvia uma página só.** O `query_one_raw` do
  SeaORM usa `fetch_optional` e para no primeiro passo do statement; o
  `incremental_vacuum` devolve **uma página por passo**. Medido: com 563 páginas
  livres, o `page_count` caía de 572 para 571, o `PRAGMA` retornava `Ok` e o
  disco não voltava. Corrigido com `execute_unprepared`, que leva o statement até
  o fim — aí o `page_count` cai para 9.
- **Linha começando com data derrubava a conexão TCP.** Detectar contagem de
  octetos por "espaço nos primeiros bytes" fazia `2026-08-15 algo` virar
  cabeçalho inválido. A detecção correta é *só dígitos* seguidos de espaço.
- **A ordem do corte por tamanho não era testável** com o bloco fixo de 10 000:
  o primeiro bloco levava o conjunto inteiro. Teto e bloco viraram parâmetros
  (`prune_with`), como o `writer::run_with` já fazia.

### Fase 3 — Consulta 🟢 Concluída

- [x] `repository.rs` com cursor de keyset sobre `(received_at, id)`
- [x] `GET /api/logs` + DTOs em camelCase (`LogEntry`, `LogPageMeta`,
      `LogPageResponse` exportados para o frontend)
- [x] Hidratação de nome de dispositivo em duas consultas
- [x] `useInfiniteCursor.ts`
- [x] `LogsPage.vue` com filtros (dispositivo, severidade, período, texto) +
      entrada no menu e rota `/logs`
- [x] `stores/logs.ts`
- [x] Aceite: **medido** com 1 M de linhas (283 MB, `spike_syslog_parse -- query`)

| consulta | tempo |
|---|---|
| janela de 24 h, 1ª página | 0,19–0,91 ms |
| janela + dispositivo | 0,15–0,68 ms |
| janela + severidade (erro e acima) | 4,2–8,2 ms |
| janela + severidade larga (info e acima) | 0,31 ms |
| janela + `LIKE` (busca textual) | 0,32–0,55 ms |

Alvo era ~200 ms; o pior caso ficou 25× abaixo. Duas leituras:

- **A janela obrigatória é o que salva o `LIKE`.** Restrita a 24 h, a busca
  textual varre ~6 000 linhas em vez de 1 M — 0,3 ms contra o que seria uma
  varredura de 283 MB. A Fase 5 vira conforto, como previsto.
- **O filtro estreito de severidade é o pior caso, não o largo.** Com
  `severity <= 3` o planejador escolhe `(severity, received_at)` e varre todas
  as severidades 0–3 de **todo** o período antes de recortar pela janela; com
  `<= 6` ele prefere `received_at`, que já vem ordenado. Contraintuitivo e
  irrelevante nesta escala, mas é o que cresce se o parque virar um parque de
  erros.

### Fase 4 — Tempo real e diagnóstico 🔴 Pendente

- [ ] `bus.rs` (`LogBus` dedicado)
- [ ] `GET /api/logs/stream` com filtro
- [ ] Live tail ligável na `/logs`
- [ ] Aba **Logs** no `DeviceDetailPage`
- [ ] `GET /api/logs/sources` + `POST /api/logs/sources/{ip}/bind`
- [ ] Banner na `/logs` enquanto houver fonte desconhecida descartando
- [ ] `GET /api/logs/setup-snippet` + `SyslogSetupCard.vue`

```
# RouterOS
/system logging action add name=netmonitor target=remote \
    remote=<IP> remote-port=514 bsd-syslog=yes
/system logging add topics=system action=netmonitor
/system logging add topics=error action=netmonitor

# OpenWRT
uci set system.@system[0].log_ip='<IP>'
uci set system.@system[0].log_port='514'
uci commit system && /etc/init.d/log restart
```

### Fase 5 — Full-text 🔴 Pendente

Só depois de a Fase 3 estar em uso e o `LIKE` com janela provar insuficiente.

- [ ] Trait `LogSearch`
- [ ] Implementação FTS5 (SQLite)
- [ ] Implementação `tsvector` (PostgreSQL)
- [ ] `LIKE` como fallback

### Fase 6 — Alertas por padrão 🔴 Pendente

- [ ] Dataset `log_pattern` ao lado de `interface_state`, `monitor_result` e
      `vpn_peer`
- [ ] `matcher.rs`: regex compilada uma vez, casamento na ingestão
- [ ] Contador de janela deslizante em memória por `(rule_id, device_id)`
- [ ] Disparo alimentando o `manager` existente (herda histerese, flapping e
      higiene de notificação — tempestade de log não vira tempestade de
      notificação)
- [ ] Catálogo: falha de login, `system started` inesperado, OSPF/BGP down,
      PPPoE caindo, pool DHCP esgotado, `out of memory`, alteração de
      configuração
- [ ] Tela de regra de log no frontend

---

## 5. Aceite

```bash
# backend/
cargo fmt --all --check && cargo clippy --all-targets -- -D warnings
cargo test && cargo build --release

npm --prefix frontend run typecheck && npm --prefix frontend run format
npm --prefix frontend run lint && npm --prefix frontend run build
```

Bindings TS saem do `cargo test` — `npm run format` depois dele, nunca antes.

**Testes**: `127.0.0.1` só, timeout de 5 s, `#[serial]` em estado global.
Listener em **porta 0** (porta fixa colide em teste paralelo).
`SYSLOG_ENABLED=false` no `config/test.yaml` — `request_with_config` roda os
initializers e sem a trava todo teste de requisição tentaria abrir a 5514.
Limpeza própria do banco de logs: `Hooks::truncate` não o alcança.
