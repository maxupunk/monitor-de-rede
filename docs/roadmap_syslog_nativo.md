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
| Busca atrás de trait, escolhida em runtime | FTS5 no SQLite, `tsvector` no PostgreSQL, `LIKE` de fundo. Medido: os dois perdem em lados opostos — ver Fase 5 |
| ⚠ **`LogBus` dedicado**, não o `EventBus` | o `EventBus` é `broadcast` de 1024 compartilhado com o SSE do dashboard; 12 msg/s rola o anel em 85 s e o painel perde `monitor:result` |
| ⚠ Fila cheia descarta o **mais novo** | `mpsc::try_send` devolve o mais novo; descartar o mais antigo exigiria ring próprio. Contador exposto do mesmo jeito |
| ⚠ `from` padrão de 24 h, janela com teto | `LIKE '%q%'` sem janela varre ~1,5 GB em 7 M de linhas |
| ⚠ `RETENTION_LOGS_MAX_MB=4096` | medido: 290 B/linha com os 3 índices → ~301 MB/dia, ~2,1 GB em 7 dias. 2048 MB ficaria no limite; o teto folgado só custa disco quando o disco é usado |
| ⚠ Regra de log casa **na ingestão** | regex sobre janela não usa índice; na ingestão são ~120 regex/s |
| ⚠ IP de origem verificado no spike | se o Docker mascarar a origem, nada é gravado e não há erro visível |
| ⚠ **O Docker mascara mesmo** (Fase 7) | medido em produção. Resolvedor cai para o `HOSTNAME`, limitador e lista de fontes separam por nome, e o *bind* do gateway é recusado. `network_mode: host` é a correção de raiz |
| ⚠ Credencial de ativação não é persistida | guardar a senha de admin de 30 roteadores para poupar um trabalho pontual inverte o custo/benefício |
| ⚠ `devices.access_mode` guarda a **declaração**, nunca a dedução (Fase 8) | a dedução envelhece: gravá-la deixaria um equipamento que saiu da VPN marcado como VPN para sempre |

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
  search.rs        Fase 5 — FTS5 / tsvector / LIKE por densidade
  retention.rs     dias + tamanho, o que vencer primeiro
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
src/components/logs/LogTable.vue, LogSourcesDialog.vue, SyslogSetupDialog.vue
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
device_logs_device_received_index     (device_id, received_at)
device_logs_received_at_index         (received_at)
device_logs_severity_received_index   (severity, received_at)
device_logs_fts                       FTS5 de conteúdo externo (Fase 5)
```

### API

```
GET  /api/logs                     deviceId, severity, facility, from, to, q, cursor, limit
GET  /api/logs/stream              SSE, live tail filtrado
GET  /api/logs/sources             fontes vistas
POST /api/logs/sources/{key}/bind  vincula origem a device (IP ou host:<nome>)
GET  /api/logs/setup-snippet       comandos por fabricante
POST /api/logs/devices/{id}/provision        ativa o log (SSH/Telnet/MAC-Telnet)
GET  /api/logs/devices/{id}/provision-hints  palpites para preencher a tela
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

### Fase 4 — Tempo real e diagnóstico 🟢 Concluída

- [x] `bus.rs` (`LogBus` dedicado, anel de 512 separado do de domínio)
- [x] `GET /api/logs/stream` com filtro por dispositivo e severidade
- [x] Live tail ligável na `/logs`, com o filtro reiniciando o stream junto
- [x] Aba **Logs** no `DeviceDetailPage`, reaproveitando a store e o `LogTable`
- [x] `GET /api/logs/sources` + `POST /api/logs/sources/{ip}/bind`
- [x] Banner na `/logs` enquanto houver fonte desconhecida descartando
- [x] `GET /api/logs/setup-snippet` + `SyslogSetupDialog.vue` (4 fabricantes)

Duas decisões que a implementação exigiu:

- **O `bind` manual vence toda a heurística** e persiste em `system_settings`
  (chave `syslog.source_bindings`). Tabela própria custaria migration, entrada
  no `CREATION_ORDER` e uma FK que não pode existir — o log mora no outro banco.
  Vincular invalida o cache do resolvedor na hora; sem isso o operador
  continuaria vendo "desconhecida" por até 30 s.
- **O pipeline subiu para fora da trava de teste.** A trava barrava o serviço
  inteiro, e com ele a API de diagnóstico — `GET /api/logs/sources` respondia
  400 em teste, e responderia 400 em produção se a porta estivesse ocupada,
  justamente onde o operador iria procurar o motivo. Agora `build` monta o
  pipeline em todo ambiente e só `spawn_listeners` é barrado.

O live tail publica **depois** da gravação, com o `id` que o banco devolveu: é
ele que permite à tela deduplicar na fronteira entre o tempo real e a
paginação. O `RETURNING` extra só é pago quando há assinante.

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

### Fase 5 — Full-text 🟢 Concluída

O gate desta fase era "o `LIKE` com janela provar insuficiente". **Provou** — e
por um motivo que a medição da Fase 3 não tinha alcançado, porque só testou a
janela de 24 h. Com 1 M de linhas e janela de 7 dias:

| termo | `LIKE` | FTS5 |
|---|---|---|
| denso (casa em 12% das linhas) | 567 µs | **450 ms** |
| esparso (não casa com nada) | **847 ms** | 164 µs |

Os dois perdem em lados opostos, e a causa é a mesma: o `LIMIT 51`. Com termo
denso o `LIKE` enche a página nas primeiras linhas e sai cedo; o índice, não —
precisa materializar as 125 mil linhas que casam antes de ordenar. Com termo
esparso é o inverso: o `LIKE` só prova a ausência varrendo tudo. **E "não achei
nada" é o resultado mais comum de quem procura um erro específico.**

- [x] Trait `LogSearch`
- [x] Implementação FTS5 (SQLite), conteúdo externo + gatilhos
- [x] Implementação `tsvector` + índice GIN (PostgreSQL)
- [x] `LIKE` como fallback, escolhido em tempo de execução por sondagem do
      catálogo — índice ausente não vira erro de SQL em toda busca
- [x] **Escolha por densidade**: até 10 000 casamentos o filtro é a lista de
      ids do índice; acima disso o termo é denso e o `LIKE` volta. Os dois
      ramos ficam abaixo de 1 ms

O que muda para o usuário: a busca passa a casar por **token com prefixo**, não
por substring. `ethe` acha `ether1`; `ther1` não. É a troca que compra os
847 ms, é a mesma semântica de qualquer ferramenta de log com índice, e está
fixada em teste.

### Fase 6 — Alertas por padrão 🟢 Concluída

- [x] Dataset `log_pattern` ao lado de `interface_state`, `monitor_result` e
      `vpn_peer`, com cinco campos novos no vocabulário das regras
- [x] `matcher.rs`: regex compilada uma vez por recarga, casamento na ingestão
- [x] Contador de janela deslizante em memória por `(rule_id, device_id)`, com
      teto de 1 000 casamentos por chave
- [x] Disparo alimentando o `manager` existente pelo ciclo do scheduler —
      herda histerese, flapping e higiene de notificação
- [x] Catálogo com os 7 padrões: falha de login, `system started` inesperado,
      OSPF/BGP down, PPPoE caindo, pool DHCP esgotado, `out of memory`,
      alteração de configuração
- [x] Categoria "Padrões no log (syslog)" e os campos `logMatchCount` /
      `logSeverity` no seletor de regras do frontend

**A configuração do matcher mora na mesma `condition` da regra.** O
`AlertRuleCondition::from_json` ignora chaves que não conhece, então `pattern`,
`minSeverity` e `windowSeconds` convivem com `field`/`operator`/`value` — sem
coluna nova, sem migration na base principal.

**Zero casamentos na janela é a recuperação.** É o que permite ao alerta
resolver: sem isso ele ficaria aberto para sempre depois do primeiro disparo.

O catálogo cobrou uma invariante que a implementação inicial violou: severidade
`info` tem cooldown zero. `log_config_changed` é rastro de auditoria, não
problema — e um teste já existente pegou.

### Fase 7 — Origem mascarada e ativação automática 🟢 Concluída

Duas coisas que só apareceram com o sistema em produção. A primeira era o risco
número um do projeto desde a Fase 1; a segunda é o passo que sobrou de manual.

#### O IP de origem atrás do Docker

**A publicação `514:5514` reescreve o remetente.** Era a pergunta 3 do spike,
nunca medida, marcada como o item de maior risco — e a resposta é a ruim. O
pacote chega vindo do gateway da bridge, e o parque inteiro vira uma origem só.

O que quebrava junto, e que a leitura inicial não previa: não era só o vínculo.
O limitador por fonte passava a valer para todos os aparelhos somados, e o
*bind* manual — o escape que a Fase 4 criou justamente para casos assim —
virava uma armadilha, porque vincular o gateway atribuiria todo o parque a um
dispositivo. **O escape era mais perigoso que o problema.**

- [x] `nat.rs`: gateway lido do `/proc/net/route`, faixas que o Docker aloca
      sozinho (`172.16/12` e o `192.168.65/24` do Docker Desktop) e
      `SYSLOG_NAT_GATEWAYS` para o que a heurística não cobre
- [x] `resolver.rs`: origem mascarada **pula** o casamento por IP e o CIDR, e
      resolve pelo `HOSTNAME`. Vínculo manual passa a aceitar chave
      `host:<nome>`, que vence o vínculo por endereço
- [x] `ingest.rs`: chave do limitador inclui o hostname quando há mascaramento
- [x] `sources.rs`: uma linha por hostname em vez de uma por IP — sem isso não
      há como vincular um equipamento sem vincular todos
- [x] `POST /sources/{key}/bind` recusa o IP do gateway com o motivo e o
      caminho certo, em vez de aceitar e contaminar
- [x] `SYSLOG_EXTERNAL_PORT`: a porta dos comandos gerados deixa de ser fixa em
      514. Em `network_mode: host` não há mapeamento e a porta real é a 5514 —
      o snippet apontava para o vazio
- [x] Aviso nas telas `/logs`, no diálogo de origens e na aba do dispositivo,
      antes do aviso de "origem desconhecida": ali a instrução "vincule cada
      endereço" está errada, e segui-la é que faz o estrago
- [x] `docker-compose.yml` documenta `network_mode: host` como a correção de
      raiz

Só a heurística de faixa não bastaria, e só o `/proc/net/route` também não: o
primeiro erra em sub-rede fora do pool, o segundo não existe fora do Linux.
Somados com a variável de escape, os três cobrem o que se sabe cobrir.

**O que isto não conserta.** Equipamento que não manda `HOSTNAME` continua
indistinguível atrás do NAT — e RouterOS sem `bsd-syslog=yes` é exatamente esse
caso. A recomendação da ADR 008 deixou de ser conforto: virou o que separa um
aparelho identificável de um anônimo.

#### Ativação automática do log no equipamento

- [x] `provision.rs`: sessão SSH (`russh`) ou Telnet, shell com PTY, comandos
      enviados um por linha
- [x] `POST /api/logs/devices/{id}/provision`, com a credencial vivendo só na
      requisição
- [x] `snippets.rs` passou a expor `commands_for` — a receita da tela e a do SSH
      são a mesma lista, fixado em teste
- [x] Confirmação de chegada: depois dos comandos, o servidor espera até 12 s
      pela primeira mensagem do dispositivo. "Comandos aceitos" sem log chegando
      é justamente o desfecho que este recurso existe para evitar
- [x] `SyslogAutoSetupDialog.vue` e o botão na aba **Logs** do
      `DeviceDetailPage`

**A credencial não é guardada, e isso é decisão, não pendência.** Guardar a
senha de administrador de trinta roteadores faria deste sistema o alvo mais
valioso da rede que ele monitora, para poupar um trabalho que se faz uma vez por
aparelho. O custo aceito é ter de digitar de novo ao reconfigurar; a tela avisa
antes de pedir.

Duas escolhas que a implementação cobrou:

- **Shell com PTY, não `exec`.** O canal `exec` do SSH roda um comando e fecha,
  e `configure` do EdgeOS é uma sessão com estado que os `set` seguintes
  precisam encontrar aberta. O shell serve aos quatro fabricantes e ao Telnet
  com o mesmo código.
- **O fim de um comando é detectado por silêncio, não por prompt.** Cada
  fabricante tem o seu, ele muda com o hostname, e `>`/`#` aparecem dentro da
  saída. É frouxo de propósito: um falso "acabou" só antecipa o próximo comando,
  enquanto um prompt não reconhecido travaria a sessão até o teto.

#### Ajustes de campo (mesma fase, depois do primeiro uso real)

O recurso subiu e cinco coisas apareceram só com alguém usando:

- [x] **`localhost` ia para dentro do roteador.** A tela mandava
      `window.location.hostname`, e quem abre a interface em
      `http://localhost:3333` gravava `remote=localhost` no equipamento — que
      passava a enviar o syslog para si mesmo. Sem erro, sem aviso, sem nada
      chegando. Agora há campo próprio, preenchido pelo servidor via `connect`
      de UDP até o equipamento (consulta a tabela de rotas sem transmitir
      pacote), e loopback/link-local são recusados nas duas pontas
- [x] **A porta padrão é setada ao trocar de protocolo**, não deixada em branco
      com `placeholder`: campo vazio obriga a saber o padrão de cor para
      conferir se está certo
- [x] **`GET /devices/{id}/provision-hints`**: sonda 22 e 23 em paralelo, lê o
      `sysDescr` por SNMP quando ele está ligado, e busca o MAC. A tela abre
      preenchida e diz **de onde** veio cada palpite — chute apresentado como
      informação é pior que campo vazio
- [x] **Linha de teste depois da configuração.** Era a ambiguidade que fazia o
      desfecho parecer falha: as regras enviadas cobrem tópicos que só falam
      quando algo acontece, então silêncio não distinguia "roteador quieto" de
      "firewall bloqueando". Agora o equipamento é mandado emitir uma linha na
      severidade que ele acabou de encaminhar, e silêncio passa a significar
      caminho bloqueado
- [x] **A senha só é descartada ao fechar o diálogo.** Limpá-la logo após
      aplicar deixava o campo vazio marcado como obrigatório ao lado da
      mensagem de sucesso, e obrigava a redigitar para reaplicar. O botão de
      destaque depois do resultado passou a ser "Concluir"; "Aplicar de novo"
      ficou em segundo plano
- [x] **MAC-Telnet** para MikroTik e OpenWRT — ver abaixo

#### Endereços deste servidor

O `localhost` corrigido acima expôs um problema maior: **cada tela reinventava
um palpite de "o endereço deste servidor"**, e nenhuma delas podia acertar
sempre — porque a resposta certa depende de onde o equipamento está. Um
servidor, várias portas de entrada: LAN, túnel e internet.

- [x] `services/server_addresses.rs` + `GET`/`PUT /api/server-addresses`
- [x] Três entradas **detectadas**, não digitadas: a rota de saída deste
      servidor, o primeiro endereço útil da faixa do WireGuard (mesmo cálculo do
      `server_service`) e o `public_endpoint` que o operador já preencheu para os
      peers funcionarem
- [x] Guarda-se só o que o operador **acrescentou ou corrigiu**. Gravar também
      os detectados os congelaria: o IP da LAN muda e a tela seguiria mostrando
      o antigo com toda a confiança. Correção em branco volta ao detectado
- [x] Cada entrada carrega **quando usá-la** e **de onde veio o valor** — é o
      par que dispensa explicação e impede palpite de ser lido como certeza
- [x] Quando um endereço não é detectado, a entrada continua na lista com o
      motivo concreto. O caso mais comum é o menos óbvio: em container de rede
      bridge o servidor enxerga só a ponte, e dizer isso é melhor do que
      oferecer `172.17.0.2` como "o endereço da rede local"
- [x] `suggest_for`: qual das entradas serve a **este** equipamento, e por quê.
      A evidência é a rota de saída — se o sistema sai por `10.8.0.1` para falar
      com o aparelho, é por `10.8.0.1` que ele volta, e isso resolve LAN e túnel
      sem classificar nada. Peer da VPN e padrão explícito são os desempates
- [x] Seletor nas duas telas de configuração de syslog (automática e manual),
      com "Outro endereço…" para o caso avulso e "Gerenciar endereços" abrindo o
      editor **sem sair do diálogo** — quem percebe a falta no meio do
      preenchimento resolve ali, sem perder o que digitou
- [x] Card em Configurações e entrada no menu Infraestrutura
- [x] Mora em `system_settings`, chave `server.addresses`: poucas linhas, escrita
      manual, sem FK — e a tabela já entra no backup e no `truncate` de teste

O saneamento de `localhost` continua nas duas pontas, agora também na gravação
da lista: um endereço que aponta o equipamento para ele mesmo é recusado com
422 antes de chegar a qualquer roteador.

#### MAC-Telnet

Acesso por MAC, sem IP, sobre UDP/20561: é o que alcança o equipamento cujo IP
está errado, ausente ou fora do alcance deste servidor.

- [x] `mactelnet.rs`: moldura, pacotes de controle, desafio MD5 e sessão
- [x] `Protocol::MacTelnet` no mesmo laço de comandos do SSH e do Telnet
- [x] O MAC vem de `device_interfaces` (SNMP) ou de `discovery_results` (ARP) —
      **`devices` não tem coluna de MAC**, o que o `macAddress` da aba "Visão
      Geral" mascarava mostrando "Não cadastrado" para todo mundo
- [x] Sem IP deixa de ser impedimento para este protocolo, e passa a ser para os
      outros dois com a mensagem certa

Duas ressalvas que precisam sobreviver a este documento:

- **Difusão não atravessa a ponte do Docker.** No arranjo padrão do compose o
  pacote não chega ao equipamento. `nat.rs` sabe dizer se está nesse caso
  (`bridged_container`) e a tela avisa antes de deixar escolher o meio. Só
  `network_mode: host` torna o MAC-Telnet utilizável.
- **O protocolo não é documentado pelo fabricante**, e o que está aqui vem da
  implementação de referência de código aberto. A montagem e a leitura dos
  pacotes têm teste unitário; o *handshake* completo contra um RouterOS real
  **não foi verificado** — não há equipamento neste ambiente. Falha aparece como
  sessão que não autentica, não como configuração errada gravada no aparelho.

**A chave de host não é verificada.** Não há `known_hosts` a consultar numa
conexão única, e recusar por não conhecer tornaria o recurso inútil. O que torna
o custo aceitável: credencial de uso único, alvo na rede local que este mesmo
sistema já monitora, e quem consegue se interpor ali tem caminhos mais curtos.
`russh` entra com `default-features = false` e o backend `ring` — o padrão
(`aws-lc-rs`) exigiria cmake e clang no `builder` do Dockerfile, e o `ring` já
está na árvore pelo `rustls`.

### Fase 8 — A forma de acesso do equipamento 🟢 Concluída

A Fase 7 criou o catálogo de endereços deste servidor e ainda assim deixou uma
pergunta na tela de ativação: *qual deles?* Perguntar era honesto enquanto não
havia como saber — mas o sistema **tem** como saber, e o que faltava era ligar
cada equipamento a uma das situações do catálogo.

#### `devices.access_mode`

Três valores — `local`, `vpn`, `remote` — que correspondem um a um às entradas
"Rede local", "Túnel VPN" e "Internet" da lista de endereços.

- [x] Coluna **anulável**, e `NULL` significa "automático". Sem backfill
- [x] `services::devices::access`: a dedução, da evidência mais forte para a
      mais fraca — peer registrado da VPN, rede do túnel, faixa do túnel, rede
      cadastrada pelo nome, faixa privada (RFC 1918 **e** CGNAT), IP público
- [x] A declaração vence a dedução. É o caso que justifica a coluna: a filial
      atrás de outra VPN tem IP privado e é indistinguível de um vizinho de LAN
- [x] `AccessContext` carrega em três consultas e julga a lista inteira —
      `present_many` não podia voltar a ter um N+1
- [x] Peer criado pelo assistente de VPN nasce com `access_mode = "vpn"`: ali
      não é dedução, o dispositivo está nascendo **por causa** do túnel
- [x] `suggest_for` passa a ter quatro degraus: declaração → rota de saída →
      dedução → padrão explícito. A rota continua acima da dedução porque é
      observação, não inferência
- [x] Seletor opcional no cadastro, com a conclusão do sistema no subtítulo do
      "Automático". Esconder a conclusão faria o operador declarar no escuro
- [x] O `auto` é uma palavra do vocabulário da API, não a ausência do campo: a
      tela manda o formulário inteiro, e sem ela voltar ao automático seria
      indistinguível de "não mexi neste campo"
- [x] Na ativação de log o seletor de endereço vira **resumo com motivo** —
      "vai enviar para 10.8.0.1:514 · Túnel VPN — o cadastro diz que este
      equipamento acessa por túnel vpn" — e o seletor fica a um clique em
      "Alterar". Cair no primeiro endereço da lista por falta de sugestão
      mantém o seletor aberto: escolha arbitrária não vira afirmação

O teste que importa não é o de ida e volta, e sim o que verifica que declarar
"acesso remoto" muda o endereço oferecido na tela de log
(`tests/requests/device_access_mode.rs`).

#### A configuração da VPN passa a ler o catálogo

O `public_endpoint` do servidor WireGuard **é** a origem do endereço "Internet"
da lista. Enquanto os dois pudessem discordar em silêncio, o syslog apontaria
para um lado e os túneis para o outro.

- [x] Combobox alimentado pelos endereços do servidor, com digitação livre — o
      endpoint pode ser um nome que não está no catálogo
- [x] Aviso de divergência quando existe uma correção manual em "Internet"
      diferente do que está gravado aqui, com o botão que adota a da lista
- [x] `forget_override_if`: gravar o endpoint apaga a correção que virou cópia
      dele. Mantê-la seria guardar uma bomba-relógio — na mudança seguinte ela
      venceria em silêncio
- [x] Substituir um endpoint já em uso pede confirmação, e **só quando há
      peers**: cada um deles guarda o endereço antigo no próprio arquivo. Sem
      peers não há o que quebrar, e perguntar seria cerimônia
- [x] Sub-rede da VPN com sugestões de faixas privadas que não colidem com
      nenhuma rede cadastrada, mais aviso (não bloqueio) de sobreposição —
      dois caminhos para o mesmo endereço não dão erro, dão pacote sumido

**Ressalva.** A sub-rede não sai do catálogo de endereços, e não tinha como
sair: aquilo guarda endereços, não faixas. O que ela ganhou foi a fonte que
realmente serve para escolhê-la — as redes já cadastradas, que são exatamente
com quem ela não pode colidir.

### Fase 9 — Um catálogo de sistemas para todas as telas 🟢 Concluída

A mesma pergunta era feita em três lugares com três vocabulários:

| tela | o que oferecia | o que a chave significava |
|---|---|---|
| assistente da VPN | `mikrotik`, `openwrt`, `linux`, `windows`, `mobile` | nome do gerador de configuração |
| ativação de log | `routeros`, `openwrt`, `ubiquiti`, `linux` | chave da receita de syslog |
| cadastro do dispositivo | texto livre em "Fabricante" | o OUI do MAC, quase sempre |

O operador que digitava "MikroTik" no cadastro, escolhia `mikrotik` na VPN e
via `routeros` na tela de log não tinha como saber que eram a mesma escolha.

- [x] `services::devices::systems`: sete entradas — `routeros`, `openwrt`,
      `ubiquiti`, `linux`, `windows`, `mobile`, `other` — cada uma com o que
      **suporta**: receita de syslog, MAC-Telnet, perfil de VPN e os apelidos
      que a identificam num `sysDescr`
- [x] O `id` nomeia o **sistema**, não o fabricante: `routeros`, porque a
      MikroTik também vende aparelho com SwOS. O assistente da VPN continua
      falando `mikrotik` — é o nome do gerador registrado lá — e a tradução mora
      no `vpnProfile` do catálogo
- [x] Um teste percorre os dois lados: toda receita de syslog e todo perfil de
      VPN precisa ter dono no catálogo, e todo `supportsSyslog` precisa ter
      receita. É o que impede o catálogo de virar um quarto vocabulário
- [x] `devices.operating_system`, anulável, com a mesma semântica do
      `access_mode`: guarda a **declaração**, e `NULL` é automático
- [x] Backfill a partir de `vpn_peers.device_profile` — quem nasceu pelo
      assistente já tinha respondido
- [x] `hints::deduz_vendor` deixou de existir: a tabela de apelidos era uma
      cópia, e a dedução agora é `systems::deduce`, com a declaração acima do
      `sysDescr` e o `sysDescr` acima do texto livre
- [x] `GET /api/devices/systems` serve o catálogo; a tela não tem cópia dele
- [x] "Fabricante" na ativação de log virou "Sistema", com o mesmo catálogo.
      Os sistemas sem receita aparecem **desabilitados**, com o motivo no
      subtítulo — omiti-los faria a lista parecer incompleta
- [x] O `MAC_TELNET_VENDORS` escrito na tela saiu: quem responde é o
      `supportsMacTelnet` do catálogo
- [x] Renomeado no contrato, e não só no rótulo: `vendor` → `operatingSystem`
      nos três DTOs de log e `SetupSnippet.vendor` → `system`. Trocar só o texto
      da tela deixaria a unificação por fazer, com dois nomes para uma coisa

O campo "Fabricante / Vendor" do cadastro **continua existindo** e não foi
substituído: ele diz quem fez a placa, e ainda alimenta a dedução quando não há
SNMP nem declaração. O que mudou é que ele deixou de ser a resposta a uma
pergunta que não era a dele.

#### O `sysDescr` genérico, e o desempate pelo `dropbear`

Um OpenWrt real ficou identificado como Linux. O agente dele responde:

```
sysDescr:     Linux bpi-r3-assistencia 6.12.87 #0 SMP ... aarch64
sysObjectId:  1.3.6.1.4.1.8072.3.2.10
```

Nada ali diz OpenWrt — é o `uname`, e o OID é o do **net-snmp**, que é o agente
e não o sistema. A palavra "Linux" casava com o apelido de `linux` e a dedução
encerrava aí, sem nunca consultar evidência mais específica.

- [x] `OperatingSystem::generic`: só `linux` é marcado. Genérico decide **por
      último**, depois que todo o resto se calou — a palavra que quase todo
      firmware embarcado diz não pode encerrar a busca
- [x] `sys_object_ids`: prefixo de empresa da IANA, que é registro e não prosa —
      14988 (MikroTik), 41112 e 10002 (Ubiquiti), 311 (Microsoft). O 8072
      (net-snmp) fica **fora** de propósito: mapeá-lo para `linux` reintroduziria
      o mesmo erro por outro caminho
- [x] `ssh_banners`: o `dropbear` é o desempate. É o servidor SSH padrão do
      OpenWrt, e a linha de identificação chega **antes de qualquer
      autenticação** — o mesmo `connect` que já sondava a porta 22 a traz de
      graça, sem uma ida à rede a mais
- [x] `systems::detect(&Evidence)` no lugar do `deduce(a, b, c)`: cada chamador
      tem um subconjunto diferente de evidência, e `Default` deixa a chamada
      legível em vez de uma fila de `None` posicionais
- [x] Toda conclusão passa a vir com `reason` — a frase que permite conferir.
      Foi a falta dela que deixou o erro passar: o campo afirmava "Linux" e não
      havia onde ver que a razão era um `uname`

A ordem final: declaração → `sysObjectId` → apelido específico no `sysDescr` →
banner do SSH → apelido genérico no `sysDescr` → cadastro → padrão. O salto que
importa é o banner vir **antes** do genérico.

#### `POST /api/devices/identify`

- [x] Botão ao lado do seletor de Sistema no cadastro, que consulta o
      equipamento agora — SNMP e sonda SSH em paralelo — e **adota** o
      resultado. Adotar e não só informar: quem clicou queria acertar o campo
- [x] A resposta carrega a evidência crua (`sysDescr`, `sysObjectId`,
      `sshBanner`) junto da conclusão, e a tela mostra as duas
- [x] `probed` separa "detectei" de "recaí no cadastro". Sem ele a tela
      anunciaria uma detecção que não aconteceu
- [x] Recebe o **formulário**, não um id: identificar precisa funcionar antes de
      salvar, e num cadastro novo não há dispositivo para consultar
- [x] O campo Sistema passou a ocupar a linha inteira (os rótulos são longos e o
      subtítulo carrega a conclusão com o motivo), e Fabricante desceu para
      junto de Modelo — os dois descrevem o hardware

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
