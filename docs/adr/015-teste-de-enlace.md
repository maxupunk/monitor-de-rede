# ADR 015 — Teste de enlace: motores nativos na central, papéis declarados por plugin, orquestração na central

## Status

**Aceito** (2026-10-16). Este documento é o contrato entre as frentes de
implementação (motores, runtime de plugin, plugins, orquestrador, agente e
telas). Mudou o contrato? Mude aqui primeiro.

## Contexto

O operador quer saber **quanto passa** em cada enlace da rede — de um
equipamento a outro, enlace por enlace ao longo de um caminho, e do próprio
servidor até um roteador —, vendo o teste acontecer na topologia.

O que existia:

- o speedtest WAN (Cloudflare) e o LAN navegador↔servidor
  (`network_tools/speedtest.rs`, `controllers/diagnostics.rs`) — nenhum mede
  um enlace entre dois equipamentos;
- plugins (ADR 012): uma execução fala com **um** equipamento, chamada SSH de
  no máximo 30 s, saída só no fim, nada fica aberto entre execuções;
- o agente remoto (ADR 011): só disca para fora, enum fechado de comandos.

Os pares que aparecem nas redes atendidas: MikroTik↔MikroTik,
OpenWrt↔OpenWrt, MikroTik↔OpenWrt e Linux. Escala: 150–800 nós por mapa.

### Ferramenta: por que não "só iperf3"

| Par | Ferramenta | Instala algo? |
|---|---|---|
| MikroTik ↔ MikroTik | `btest` nativo do RouterOS (`/tool bandwidth-test` + `bandwidth-server`) | não |
| OpenWrt ↔ OpenWrt | `iperf3` (pacote oficial) | sim, com o aceite do operador no plano, pelo `openwrt-packages` |
| OpenWrt → MikroTik | `mikrotik-btest` (pacote **oficial** do OpenWrt 23.05/24.10, cliente) contra o `bandwidth-server` | sim, no OpenWrt |
| Linux ↔ OpenWrt/Linux | `iperf3` | Linux: precisa existir (não instalamos pacote em servidor) |
| central ↔ OpenWrt/Linux | central é **cliente iperf3 nativo** (Rust); o equipamento sobe `iperf3 -s` | OpenWrt: iperf3 |
| central ↔ MikroTik | central é **servidor btest nativo** (Rust, porta 2000/TCP) e o MikroTik é o cliente | não |
| agente ↔ equipamento | agente é cliente nativo (iperf3 ou btest sem autenticação) | idem acima |

O RouterOS não roda iperf3 e não instala terceiros; o OpenWrt não tem
servidor btest. Daí a regra: **cada lado declara o que sabe servir e o que
sabe dirigir**, e a central negocia.

Alternativas descartadas:

- **HTTP (`/tool fetch`, `uclient-fetch`) para tudo**: dispensa instalação,
  mas o `fetch` do RouterOS só mede bem o download, com granularidade de 1 s,
  e não há UDP/jitter. Fica fora.
- **Binário `iperf3` dentro do container da central**: dependência externa e
  `execFile`, contra o espírito do ADR 003. Os dois protocolos são pequenos o
  bastante para implementar (cliente iperf3 TCP; servidor e cliente btest TCP).
- **Autenticação EC-SRP5 no cliente btest nativo**: protocolo proprietário
  reconstruído por engenharia reversa, sem como validar aqui. O cliente nativo
  (agente) fala só sem autenticação; quem precisa disso pede ao plugin do
  MikroTik que desligue a autenticação **durante a janela do teste** e a
  restaure no fim (ver §4).

## Decisão

### 1. Motores nativos — `services/network_tools/throughput/`

Rust puro, sem processo externo, testável em loopback.

- `iperf3.rs` — **cliente** do protocolo iperf3 (TCP; `reverse`; `streams`
  1–8; `duration` 1–60 s; `limit_bps` opcional), compatível com o `iperf3 -s`
  3.x. Amostras de 1 s por callback.
- `btest.rs` — protocolo MikroTik Bandwidth Test em TCP (porta 2000):
  **servidor** (central) e **cliente** (agente), mesmo codec. O servidor
  responde "sem autenticação" (`01 00 00 00`); MD5 (RouterOS < 6.43) só no
  cliente.
- `mod.rs` — os tipos comuns:

```rust
pub enum ThroughputEngine { Iperf3, Btest }          // "iperf3" | "btest"
pub enum TestDirection { Send, Receive, Both }        // do ponto de vista de quem DIRIGE
pub enum TestProtocol { Tcp }                         // UDP fica para depois
pub struct ThroughputSpec { engine, direction, protocol, duration_s: u8, streams: u8, limit_mbps: Option<u32> }
pub struct ThroughputSample { t_ms: u64, flow: Flow /* Tx | Rx */, bps: f64 }
pub struct ThroughputReport { engine, tx_bps: Option<f64>, rx_bps: Option<f64>, tx_bytes: u64, rx_bytes: u64,
                              retransmits: Option<u64>, samples: Vec<ThroughputSample> }
```

`tx` = quem dirige envia; `rx` = quem dirige recebe. O orquestrador traduz
para o sentido do enlace (`forward` = origem→destino, `reverse`).

**Servidor btest da central — armado, nunca aberto.** Um initializer escuta em
`BTEST_LISTEN` (padrão `0.0.0.0:2000`; vazio desliga). Conexão só é aceita se
existir uma **sessão armada** para o IP de origem (`BtestServer::arm(peer_ip,
ttl)` devolve um receptor de amostras/relatório); fora disso, fecha na hora. A
sessão é de uso único e expira (TTL padrão 90 s). O compose publica
`2000:2000/tcp`.

### 2. Runtime de plugin — saída ao vivo

- `DeviceTransport::execute_streaming(call, on_line)` com implementação padrão
  (executa e entrega as linhas no fim). `LocalTransport` entrega SSH linha a
  linha de verdade; `AgentTransport` repassa os `Chunk` do agente (§6).
- Script: `device.ssh_live(command, #{ timeout_ms, progress })` — igual a
  `device.ssh`, com prazo próprio (até 300 000 ms) e, opcionalmente,
  `progress: #{ pattern, flow }`: regex com grupos nomeados `value` e `unit`
  (`bps|Kbps|Mbps|Gbps|bits/sec|Kbits/sec|Mbits/sec|Gbits/sec`, com ou sem
  espaço) — cada linha que casa vira uma amostra normalizada em bps.
  `device.progress(#{ flow, bps })` emite à mão.
- Amostras saem como evento efêmero SSE `plugin:run_progress`
  `{ runId, flow, bps, tMs }` e chegam ao orquestrador por um observador.

### 3. Papéis declarados no manifesto — bloco `throughput`

```json
"throughput": {
  "serve": [{ "engine": "btest" }],
  "drive": [{ "engine": "btest", "auth": true }],
  "actions": { "prepare": "tp_prepare", "check": "tp_check", "serve": "tp_serve", "stop": "tp_stop", "run": "tp_run" }
}
```

- `serve`: motores que o equipamento hospeda; `drive`: motores que ele dirige
  (`auth: false` = cliente que só fala com servidor sem autenticação, caso do
  `mikrotik-btest` — que só conhece MD5, anterior ao RouterOS 6.43 — e dos
  clientes nativos). `drive[].maxDurationS` (opcional) é o teto do cliente por
  sentido: o `mikrotik_btest` recusa `-t` ≥ 30, então declara `29`; o plano
  limita a duração pedida e avisa.
- As ações referenciadas existem no manifesto; `prepare`, `serve` e `stop` são
  `write`; `run` e `check` são `read`. `prepare` e `check` são opcionais.
- **Nunca instalar sem aceite** (decisão do dono do produto, 2026-10-19).
  `check` só **confere** se o equipamento tem o programa do papel
  (`command -v iperf3`). O plano roda o `check` de cada ponta (em paralelo,
  10 s de prazo, sempre leitura) e, faltando algo que o plugin instala
  (`canInstall: true`), mostra o passo **Instalar o iperf3 em …**
  (`install_package`): o operador aceita numa confirmação e só então o
  `prepare` roda, pela mesma rota de ação de plugin da tela
  (`POST /api/devices/{id}/plugins/{pluginId}/actions/{prepare}` com
  `confirmWrite: true` — administrador). Sem instalação possível
  (`canInstall: false`, o Linux), o motivo vai por extenso e a perna não roda.
  A primeira etapa da perna (`prepare_a`/`prepare_b`, "Conferir A/B") roda o
  `check` de novo e, faltando, para com a frase do plugin — **nunca** chama o
  `prepare`.
- Plugin sem `check` (o contrato de antes) continua rodando o `prepare` na
  primeira etapa; o plano avisa em `warnings` que ele pode instalar.
  `"prepareChecksOnly": true` (legado) declara um `prepare` que só confere: sem
  aviso de instalação e com o tempo de conferir. Plugin novo usa `check`.
- **Parâmetros fixos** (validados pela plataforma, o script não inventa):

| Ação | Parâmetros | Devolve |
|---|---|---|
| `prepare` | `{ engine, role: "serve"\|"drive" }` | `{ ready: bool, installed: [nomes], message?, warning? }` |
| `check` | `{ engine, role: "serve"\|"drive" }` | `{ ready: bool, missing: [pacotes], canInstall: bool, message: string }` |
| `serve` | `{ engine, port, peer, user, password (secret), requireAuth }` | `{ port, handle }` — `handle` é opaco e volta intacto no `stop` |
| `stop` | `{ engine, handle }` | `{ stopped: bool }` |
| `run` | `{ engine, target, port, direction, durationS, streams, limitMbps, user, password (secret) }` | `ThroughputReport` em JSON (camelCase) |

- `user`/`password` do teste são **gerados pelo orquestrador a cada teste**
  (aleatórios, nunca gravados): não são credenciais de equipamento e não
  atravessam o `{{password}}`.

### 4. Regras de segurança dos plugins de teste

- `serve` do MikroTik: com `requireAuth` cria um usuário temporário
  (`nm-test-…`, grupo `read`, que tem a política `test`) e liga o
  `bandwidth-server`; sem `requireAuth` grava o estado anterior e põe
  `authenticate=no`. `stop` desfaz exatamente o que o `handle` diz.
- **Limpeza garantida**: o orquestrador grava o `handle` pendente no banco
  **antes** de usar o servidor e chama `stop` em toda saída (sucesso, falha,
  cancelamento). No boot, pernas interrompidas com limpeza pendente são
  desfeitas.
- `iperf3 -s` no OpenWrt/Linux sobe em segundo plano numa porta escolhida pelo
  orquestrador (5201–5299) e o `stop` encerra pelo PID devolvido no `handle`.

### 5. Orquestrador — `services/link_tests/`

Tipos de teste:

- **`link`** — um enlace (dois dispositivos);
- **`path`** — caminho da origem ao destino pela topologia (`device_links` +
  hierarquia `parent_id`), **um enlace por vez**, em ordem;
- **`server`** — do servidor (o dispositivo de `system_key = netmonitor`, ou o
  agente do site quando o destino só é alcançável por ele) até o destino:
  uma perna **ponta a ponta** e, depois, o caminho enlace por enlace.

Cada teste tem **pernas** (`legs`) e cada perna, **etapas** (`stages`):
`prepare_a`, `prepare_b`, `serve`, `forward`, `reverse`, `stop`. Etapa tem
`status` (`pending|running|succeeded|failed|skipped|cancelled`), início, fim,
mensagem e `runId` do plugin quando houver (transcript a um clique).

Negociação (função pura, testada): para o par (A, B), escolhe motor e papéis:
prefere `iperf3`, depois `btest`; o servidor é quem sabe servir; `auth` vem do
`drive` do cliente. Sem combinação → a perna fica `skipped` com o motivo.

Plano antes de rodar (`POST /api/link-tests/plan`): pernas, motor de cada uma,
o que falta (plugin ativo, credencial, pacote que a conferência achou
faltando) e quanto tempo leva. A tela mostra o plano e pede confirmação; só então
`POST /api/link-tests` com `confirm: true`.

O que falta e tem conserto ali mesmo vira **passo** (`fixes`), não erro:
equipamento sem plugin de teste ganha a sugestão do plugin certo — o que tem
bloco `throughput`, fica ativo ao instalar e casa com o sistema dele pela
mesma régua que liga um embutido sozinho no cadastro
(`compat::fits_platform`; o exclusivo do equipamento vem antes) e cujos papéis
fecham a negociação com a outra ponta; equipamento sem o acesso que o plugin
usa ganha "cadastrar o acesso" (`credentials::access_gap`); equipamento cujo
`check` diz que falta um pacote instalável ganha "instalar o pacote"
(`link_tests::readiness`). A ordem é sempre plugin → acesso → pacote: ponta
sem acesso não é conferida (o passo do acesso vem antes), e conferência que
não responde vira aviso (a primeira etapa confere de novo). Sistema
desconhecido não ganha sugestão: o motivo segue por extenso em `blocking`. A
tela mostra "Antes de testar, falta" com **Instalar agora**, **Cadastrar
acesso** (o formulário único do acesso, sem sair do assistente) e
**Instalar** (o pacote, depois da confirmação que explica o que acontece no
equipamento; o fim chega pelo SSE `plugin:run_finished`, e a falha mostra o
motivo do plugin com a execução a um clique) e refaz o plano depois de cada
passo. Instalar e cadastrar o acesso são de administrador; o operador vê a
quem pedir.

Limites: duração padrão 5 s por sentido (1–60), um teste por dispositivo por
vez, no máximo 2 testes simultâneos no sistema, sempre cancelável.

Tabelas: `link_tests`, `link_test_legs`, `link_test_schedules` (intervalo em
minutos, janela de horário opcional, ativo/inativo, último/próximo). O
agendador do processo único (ADR 007) dispara os agendamentos vencidos.

Tempo real (só SSE, §9 das diretrizes):

- `link_test:updated` — snapshot completo do teste (pernas e etapas) a cada
  mudança; testes ativos publicados como snapshot quando há assinante;
- `link_test:sample` — efêmero, `{ testId, legSeq, flow, bps, tMs }`.

API:

| Método | Rota |
|---|---|
| `POST` | `/api/link-tests/plan` |
| `POST` | `/api/link-tests` |
| `GET` | `/api/link-tests?deviceId=&linkId=&page=` |
| `GET` | `/api/link-tests/{id}` |
| `POST` | `/api/link-tests/{id}/cancel` |
| `GET/POST/PUT/DELETE` | `/api/link-test-schedules[/{id}]` |

Escrita (rodar, cancelar, agendar) é de `operator`/`admin`; leitura de todos.

#### 5.1 Contrato de dados (camelCase, exportado por `ts-rs`)

`forward` é sempre o sentido **A→B** da perna; `reverse`, B→A.

```ts
type LinkTestKind = 'link' | 'path' | 'server' | 'end_to_end' // §11
type LinkTestStatus = 'queued' | 'running' | 'succeeded' | 'partial' | 'failed' | 'cancelled'
type LinkTestStepStatus = 'pending' | 'running' | 'succeeded' | 'failed' | 'skipped' | 'cancelled'
type LinkTestFlow = 'forward' | 'reverse'

type LinkTestSpecInput = { durationS?: number /* 5; 1–60 */, direction?: 'both' | 'forward' | 'reverse',
                           streams?: number /* 1; 1–8, iperf3 */, limitMbps?: number | null }
type LinkTestRequest = { kind: LinkTestKind, linkId?: number, sourceDeviceId?: number,
                         targetDeviceId?: number, spec?: LinkTestSpecInput, confirm?: boolean }
// `link`: `linkId` (enlace direto) ou, sem ele, `sourceDeviceId` + `targetDeviceId` — o
// trecho entre dois equipamentos com IP que só tem equipamentos sem IP no meio (§5.2).

type LinkTestEndpoint = { kind: 'device' | 'central' | 'agent', deviceId: number | null,
                          agentId: number | null, name: string, ip: string | null, platform: string | null }

type LinkTestPlan = { kind: LinkTestKind, legs: LinkTestPlanLeg[], estimatedSeconds: number,
                      warnings: string[], blocking: string[] /* o que impede e não tem passo */,
                      fixes: LinkTestFix[] /* roda: blocking vazio e alguma perna runnable */ }
// `willInstall` (o palpite de pacotes por motor) saiu: o que falta instalar vem
// da conferência de verdade (`check`), como o passo `install_package`.
type LinkTestPlanLeg = { seq: number, kind: 'link' | 'end_to_end', a: LinkTestEndpoint, b: LinkTestEndpoint,
                         edgeId: number | null /* a aresta quando é uma só */,
                         edgeIds: number[] /* todas, em ordem de A a B (§5.2); vazia na ponta a ponta */,
                         viaDeviceIds: number[], viaNames: string[] /* equipamentos sem IP atravessados */,
                         engine: 'iperf3' | 'btest' | null, serverSide: 'a' | 'b' | null,
                         durationS: number, runnable: boolean, issues: string[], fixes: LinkTestFix[] }
// Um passo que o operador faz no próprio plano; `message` é a frase que também está em `issues`.
type LinkTestFix =
  | { kind: 'install_plugin', deviceId: number, deviceName: string, pluginId: number,
      pluginSlug: string, pluginName: string, message: string }
  | { kind: 'add_credential', deviceId: number, deviceName: string,
      transport: 'ssh' | 'telnet' | 'http', message: string }
  | { kind: 'install_package', deviceId: number, deviceName: string, pluginId: number,
      pluginSlug: string, action: string /* o `prepare` */, engine: 'iperf3' | 'btest',
      role: 'serve' | 'drive', packages: string[], message: string }

// `prepare_a`/`prepare_b` aparecem como "Conferir A/B": rodam o `check` (ou o
// `prepare`, em plugin sem `check`).
type LinkTestStage = { key: 'prepare_a' | 'prepare_b' | 'serve' | 'forward' | 'reverse' | 'stop',
                       label: string, status: LinkTestStepStatus, message: string | null,
                       runId: number | null, startedAt: string | null, finishedAt: string | null }
type LinkTestLeg = LinkTestPlanLeg & { status: LinkTestStepStatus, stages: LinkTestStage[],
                   forwardBps: number | null, reverseBps: number | null, retransmits: number | null,
                   error: string | null, startedAt: string | null, finishedAt: string | null,
                   samples?: { flow: LinkTestFlow, bps: number, tMs: number }[] /* só no GET de detalhe */ }

type LinkTest = { id: number, kind: LinkTestKind, status: LinkTestStatus, origin: 'user' | 'schedule',
                  scheduleId: number | null, linkId: number | null, sourceDeviceId: number | null,
                  targetDeviceId: number | null, spec: Required<LinkTestSpecInput>, legs: LinkTestLeg[],
                  summary: { bottleneckLegSeq: number | null, minForwardBps: number | null,
                             minReverseBps: number | null } | null,
                  error: string | null, requestedBy: { id: number, name: string } | null,
                  createdAt: string, startedAt: string | null, finishedAt: string | null }

type LinkTestPage = { items: LinkTest[], total: number, page: number, perPage: number }

type LinkTestSchedule = { id: number, name: string, kind: LinkTestKind, linkId: number | null,
                          sourceDeviceId: number | null, targetDeviceId: number | null,
                          spec: Required<LinkTestSpecInput>, intervalMinutes: number /* ≥ 15 */,
                          windowStart: string | null /* "HH:MM" */, windowEnd: string | null,
                          enabled: boolean, lastRunAt: string | null, nextRunAt: string | null,
                          lastTestId: number | null, lastStatus: LinkTestStatus | null,
                          createdAt: string, updatedAt: string }
type LinkTestScheduleInput = Omit<LinkTestSchedule, 'id' | 'lastRunAt' | 'nextRunAt' | 'lastTestId' |
                             'lastStatus' | 'createdAt' | 'updatedAt' | 'spec'> & { spec?: LinkTestSpecInput }
```

#### 5.2 Equipamentos sem IP (switch não gerenciável, hub)

Um equipamento **sem IP** é **transparente** (`link_tests/transparent.rs`;
na tela, `utils/linkTests/targets.ts`, a mesma régua): não fala SSH, não
roda plugin e não recebe conexão, mas o tráfego passa por ele. O próprio
NetMonitor nunca é transparente.

- **Nunca é ponta.** Pedir um teste até ele (ou um `linkId` que encosta
  nele) é `422`: "switch-assistencia não tem IP — escolha um equipamento
  depois dele…".
- **É atravessado.** Em `path` e nas pernas enlace por enlace do `server`,
  cada sequência de transparentes vira **uma** perna entre os equipamentos
  com IP de cada lado, com `edgeIds` (todas as arestas do trecho, em ordem) e
  `viaDeviceIds`/`viaNames`. É a medida real, em L2, dos cabos entre os dois.
  No `server`, se o caminho inteiro colapsa numa perna só, ela é a própria
  ponta a ponta.
- **`link` pelas duas pontas.** Sem `linkId`, `sourceDeviceId` +
  `targetDeviceId` pedem "o enlace entre os dois através de switches sem IP":
  o menor caminho no recorte do grafo em que só transparentes ficam no meio.
  A hierarquia declarada (`parentId`, aresta de id negativo) conta como
  enlace: é um `linkId` válido e é atravessada como qualquer cabo.
- **Mesma rota nos dois lados.** O desempate (menos saltos → maior confiança
  somada, confirmado = 100 → menor id) é o algoritmo de
  `services/topology/path.rs`, copiado passo a passo em
  `frontend/src/utils/topology/path.ts`: a opção "até BORDA · passando por
  X" do assistente é a rota que o backend mede. As duas suítes leem as mesmas
  rotas (`backend/tests/fixtures/link_test_routes.json`).
  Sem esse caminho, mas com outro (passando por equipamento com IP), é `422`
  sugerindo **Caminho completo**; sem caminho nenhum, `blocking`.
- `edgeId` fica com a aresta quando a perna tem uma só (e `null` com mais),
  para quem lê o contrato antigo. Destaque no mapa, filtro `?linkId=` (coluna
  `link_test_legs.edge_ids`, `,10,20,`, `LIKE` igual nos dois bancos) e
  velocidade nominal da perna (a menor das arestas) usam `edgeIds`. O switch
  não é ocupado pelo guarda de "um teste por dispositivo"; entra em
  `link_tests.device_ids` para o histórico dele.
- Na tela: o seletor de equipamento não oferece transparentes ("Switches sem
  IP não entram no teste — o enlace passa por eles"); em **Enlace**, um
  vizinho sem IP vira os equipamentos com IP depois dele ("até BORDA ·
  passando por switch-assistencia", vários unidos por "›"). "Testar velocidade
  do enlace" num enlace que encosta num switch sem IP abre o assistente na
  ponta com IP, para escolher o destino. A perna mostra o chip "via …"; o
  mapa marca todas as arestas, com o rótulo do resultado na primeira.

Eventos SSE: `link_test:updated` carrega um `LinkTest` (sem `samples`);
`link_test:sample` carrega `{ testId, legSeq, flow, bps, tMs }`;
`link_test_schedule:updated` carrega `{ id }` (a tela relê a lista).

### 6. Agente — protocolo 4

- `Command::Throughput { target, port, spec, auth }` com permissão própria
  `throughput`, **fora do padrão** do `AGENT_ALLOW`, alvo só em rede privada (e
  em `AGENT_DEVICE_CIDRS`, quando houver). O agente só **dirige** (cliente):
  nunca abre porta. Amostras vão em `Chunk`.
- `Command::DeviceIo` passa a mandar as linhas da saída em `Chunk` enquanto o
  comando roda.
- `PROTOCOL_VERSION` sobe para 4.

### 7. Topologia como superfície de plugin

`surfaces` ganha `topology`, e o manifesto pode declarar
`"topology": { "nodeActions": ["acao", …] }`: as ações aparecem no painel e
no menu de contexto do nó dos dispositivos onde o plugin está instalado, com
o mesmo diálogo de execução de sempre. O teste de enlace é nativo da
topologia (menu do enlace, do nó e da barra), alimentado pelos papéis do §3.

### 8. Validação num RouterOS real (adendo, 2026-10-17)

Com um RouterOS CHR 7.23.8 de verdade (o `bandwidth-test` do próprio
RouterOS como cliente, um servidor de teste do nosso lado):

- **Formato do estado em TCP**, capturado do RouterOS: `07`, `80 | cpu%`,
  `00 00`, sequência u32 **LE** (bytes 4–7), bytes recebidos no último
  segundo u32 **LE** (bytes 8–11). Os dois lados mandam um por segundo. A
  versão anterior (do `btest-opensource`, que só implementa o estado em UDP)
  mandava `00` no byte 1: o RouterOS **derrubava a conexão** ao transmitir.
- Quem transmite mede pelo estado do par (o que de fato chegou), não pelo que
  escreveu no socket — o buffer inflava o número.
- O **"both" simultâneo em TCP** não devolve o envio no RouterOS (tx 0 com
  qualquer posição do estado no fluxo). O teste mede **um sentido por vez**,
  como no iperf3; a sessão armada da central é rearmada entre os sentidos.
- O RouterOS imprime médias **zeradas** quando nem conecta (`status: can not
  connect`): o plugin decide pelo status final, e o orquestrador nunca aceita
  uma medida zerada como sucesso.
- Sem autenticação, o plugin restringe o `bandwidth-server` ao IP do par
  (`allowed-addresses4`, RouterOS 7) durante a janela, e restaura no `stop`.
- Medido: central→MikroTik 42,7 Mbit/s (o RouterOS viu 45,1); MikroTik→central
  1,20 Mbit/s (o RouterOS viu 1,26) — CHR emulado, sem KVM.
- O **cliente btest nativo** (o do agente) contra o `bandwidth-server` do
  RouterOS, sem autenticação: os dois sentidos medidos, com amostras ao vivo
  (teste `btest_contra_routeros_real`, ignorado na suíte; `NM_BTEST_TARGET`).

- **MikroTik↔MikroTik com dois RouterOS reais** ligados por um enlace
  direto: A sobe o `bandwidth-server` com o usuário temporário, B mede contra
  A nos dois sentidos (o RouterOS autentica o usuário por EC-SRP5 sozinho) e
  A desfaz tudo. Achado: o `tp_run` do plugin tinha um valor de exemplo como
  `pattern` do `user` e recusava qualquer usuário gerado — agora um teste de
  contrato confere que os parâmetros do orquestrador (com a credencial
  gerada) passam na validação de todo plugin embutido com papel de teste.

Com um **OpenWrt 24.10.4** real (imagem oficial `openwrt/rootfs`):

- `tp_prepare` instalou o `mikrotik-btest` e o `iperf3` pelo `openwrt-packages`
  (opkg), como pedido.
- `mikrotik_btest` do OpenWrt contra o RouterOS: os dois sentidos. O
  transporte tira o `\r`, e os quadros de segundo chegam colados numa linha
  só — o resumo é o **último** valor (o plugin pegava o primeiro).
- Teste de enlace central↔OpenWrt pelo orquestrador, com iperf3 real: instala,
  sobe o servidor na 5201, mede os dois sentidos e encerra sem sobras.

Com **dois OpenWrt 25.12.5 em produção** (apk) e um switch sem IP entre eles
(2026-10-19, sem mudar configuração dos equipamentos — só o iperf3 instalado,
com o aceite no plano):

- O plano mostrou "Instalar o iperf3 em …" (`tp_check`, nada instalado sem
  aceite); o "Instalar" rodou o `tp_prepare` e o plano foi refeito pelo SSE.
- Achado: instalar o `openwrt-link-test` não ligava o `openwrt-packages`, de
  que ele depende (`uses`) — agora `install` liga as dependências desligadas.
- Achado: um roteador com o **relógio 43 dias atrasado** não valida o HTTPS
  do repositório (`wgetSSL verify error`, o certificado "ainda não vale"). O
  `install_package` tenta **uma vez** o espelho HTTP do mesmo repositório,
  com a lista num arquivo de `/tmp` passado em `--repositories-file` e
  apagado em seguida (a configuração não muda); o apk confere a assinatura do
  OpenWrt no índice e em cada pacote, qualquer que seja o transporte. O aviso
  ("Atenção: … acerte o NTP") chega ao plano como `warning` do `tp_prepare`.
  Falhas viram uma frase (relógio/HTTPS com a data do roteador, sem internet,
  sem espaço, pacote inexistente); a saída inteira fica em "Ver execução".
- Medido: AP↔AP pelo switch 363 / 235 Mbit/s; central→AP 252 / 243 e
  306 / 483 Mbit/s. Nenhum `iperf3 -s` nem arquivo de `/tmp` sobrou.
- O aviso apareceu na tela com o aparelho de verdade: sem o iperf3, o
  AP de relógio atrasado instalou pelo espelho HTTP depois do aceite, e o passo
  do plano mostrou "Atenção: … o relógio dele marca … acerte o NTP".
- **OpenWrt↔MikroTik** (RB2011iL, RouterOS 7.18.2, `bandwidth-server` já
  ligado com `authenticate=yes`): o `mikrotik_btest` 0.5.1 tem `-u/-p`, mas só
  fala o login antigo — contra o EC-SRP5 do RouterOS 7 dá `Auth failed` mesmo
  com a senha certa. O OpenWrt não tem servidor btest e o RouterOS não tem
  iperf3: o único arranjo é o MikroTik servir **sem autenticação, só para o
  IP do AP**, durante a janela — o que o `tp_serve` faz. Com a autorização do
  dono para essa mudança temporária: AP→MikroTik 100,9 Mbit/s, MikroTik→AP
  32,5 Mbit/s (CPU MIPS de 600 MHz servindo), e o `/export` do RouterOS
  depois do `tp_stop` ficou **idêntico** ao de antes (só o "último login"
  do admin mudou).

### 9. Diagnóstico rápido por ping (adendo, 2026-10-19)

**Pergunta.** Equipamento desconhecido, sem SSH nem plugin de teste, não roda
o teste de velocidade. Dá para estimar a banda só com ping?

**Método** (spike `backend/examples/spikes/ping_capacity.rs`): ecos ICMP
pequenos (56 B) e grandes (1472 B, o MTU Ethernet) **intercalados**, um a cada
20 ms, 80 pares (~3,5 s, ~100 pacotes/s, ~70 kbit/s). O RTT do grande é o do
pequeno mais o tempo de serializar a diferença nos dois sentidos; com os
**mínimos** (o menor ruído de fila):

```text
capacidade ≈ 2 · (1472 − 56) · 8 bits / (RTTmin_grande − RTTmin_pequeno)
```

**Evidência** (equipamento real, três rodadas cada):

| Equipamento | Banda real (btest) | Δmin | Estimativa por ping |
|---|---|---|---|
| MikroTik | 135,7 / 175 Mbit/s | 0,16–0,24 ms | 135, 142, 94 Mbit/s |
| OpenWrt BPI-R3 | — | 0,04–0,09 ms | 256, 243, 588 Mbit/s (ruído) |
| OpenWrt RB922 | — | 0,15–0,20 ms | 156, 126, 116 Mbit/s |

O p95 dos pacotes grandes chegou a ~5 ms sobre ~1 ms de mínimo: a fila
aparece. Abaixo de ~0,1 ms o Δmin é do tamanho do ruído de relógio e de
agendamento do sistema, e a conta explode (588 Mbit/s no BPI-R3).

**Limites.** Mede o **caminho todo** da central até o equipamento, não um
enlace; inclui a CPU do equipamento respondendo ao ICMP (que pode ser
priorizado ou limitado); a precisão some acima de ~190 Mbit/s; filtro de ICMP
ou MTU menor no caminho tiram os pacotes grandes.

**Decisão.** É um **diagnóstico**, nunca uma medida de banda:
`POST /api/diagnostics/quick-ping` (`{ deviceId }` ou `{ ip }` de rede
privada, pela régua `plugins::transport::is_reachable_target`; escrita pela
regra padrão — operador e administrador; um por endereço, 409 no segundo)
devolve `QuickPingReport` com latência (mín/p50/p95 dos dois tamanhos),
jitter, perda, sinal de fila e — só quando o sinal passa do ruído — uma
**faixa** da capacidade com confiança "baixa" ou "muito baixa". A regra é
pura (`services/network_tools/ping_capacity/estimator.rs`); a coleta usa o
socket ICMP DGRAM do processo (`PingClientCell`, ADR 003) com **um** pinger
por rodada.

| Regra | Limiar | Por quê |
|---|---|---|
| Piso de ruído do Δmin | 0,12 ms (teto ≈ 190 Mbit/s) | BPI-R3 (0,04–0,09 ms) é ruído; MikroTik e RB922 (≥ 0,15 ms) ficam na ordem certa |
| Consistência | 3 lotes contíguos; maior/menor ≤ 2× | uma rodada que não concorda consigo mesma não dá número |
| Faixa | estimativas dos lotes e do total, alargadas ±20% | a variação entre rodadas do spike foi de ~1,5× |
| Confiança "muito baixa" | lotes > 1,5×, perda ≥ 5% ou fila alta | o número ainda sai, com o aviso |
| Respostas mínimas | 15 por tamanho, 3 por tamanho em cada lote | abaixo disso o mínimo não é mínimo |
| Fila moderada / alta | p95 − mín ≥ máx(2 ms; 0,5×mín) / ≥ máx(10 ms; 2×mín) | os ~4 ms do spike são fila visível; caminho longo tolera mais |
| Perda moderada / alta | ≥ 1% / ≥ 5% | uma perda em 160 ecos ainda é "ok" |

Sem resposta aos grandes (MTU/filtro), a nota diz isso e a perda passa a
contar só os pequenos. Abaixo do piso: "Acima de ~190 Mbit/s — rápido demais
para estimar por ping". Na tela, a ação "Diagnóstico rápido (ping)" entra no
mapa pelo registro de ações (todo nó com IP, inclusive desconhecido) e na
ficha do dispositivo; o diálogo diz o que o número é e oferece "Fazer o teste
de velocidade completo".

### 10. Instalar pacote quando o roteador não baixa sozinho (adendo, 2026-10-19)

**Problema.** Na BORDA (Bananapi BPI-R3, OpenWrt 25.12.5) o "Instalar o
iperf3" falhava sempre com "O equipamento não respondeu em 30 s". Causa real,
lida no aparelho: o AdGuardHome dela atende a LAN (10.0.0.1) mas **não o
próprio roteador** (127.0.0.1/::1 esperam 5 s e falham), então o `apk` não
resolve `downloads.openwrt.org` e o `apk update` esperava o DNS repositório
por repositório até o `device.ssh` estourar os 30 s. O relógio dela também
estava 34 dias atrasado (o HTTPS falharia mesmo com DNS). E o `opkg`/`apk`
cortado pelo prazo continua rodando no roteador, segurando a trava para a
próxima tentativa.

**Decisão.** O `install_package` do `openwrt-packages` (1.3.0):

1. **Confere antes** se o roteador baixa direto: DNS do host do primeiro
   repositório e um `wget --spider` do índice, 10 s cada. Um DNS quebrado custa
   ~10 s, não minutos.
2. Baixando direto: `update` e `add`/`install` por `ssh_live` com prazo de 3 e
   4 min (o índice num link lento passa dos 30 s do `ssh`).
3. Não baixando: instala **pela central**. A central serve o repositório
   oficial em `GET /api/package-mirror/openwrt/{token}/{caminho}`
   (`services/package_mirror/`): sem JWT — quem chama é o gerenciador do
   roteador —, com token de 20 min emitido para a execução
   (`device.package_mirror()`), só `releases/`/`snapshots/` e só índices e
   pacotes (`.adb`, `.apk`, `.ipk`, `Packages*`, `index.json`). Não é proxy
   aberto, e o gerenciador do roteador continua conferindo a assinatura do
   OpenWrt: a central não consegue trocar um pacote. O roteador alcança a
   central pelo IP de `$SSH_CLIENT` (o endereço pelo qual a central chegou a
   ele) e pela porta publicada da API (`APP_EXTERNAL_PORT`) — a mesma premissa
   do servidor btest. A lista (apk) ou a conf (opkg) apontando para a central
   vai para `/tmp` e é apagada no fim: a configuração do roteador não muda.
   Pelo agente remoto não há espelho (o roteador do outro site não alcança
   esta porta).
4. Último recurso para o HTTPS recusado sem espelho: o espelho HTTP do próprio
   repositório (como antes).
5. Falha vira uma frase: trava de outro gerenciador ("espere um minuto"),
   prazo estourado, relógio/HTTPS com a data do roteador, sem espaço, pacote
   inexistente, central inalcançável (com o endereço tentado) — e o que
   corrigir no roteador.

**Evidência real.** BORDA: plano pediu o aceite; a checagem acusou
`dns downloads.openwrt.org` em 10 s, o iperf3 entrou pela central em 5 s, o
aviso "Instalado pela central… Corrija o DNS do próprio roteador" apareceu no
passo, e o teste central→BORDA mediu 349 / 425 Mbit/s. Nada ficou em `/tmp`
e os repositórios dela continuaram os oficiais. OpenWrt 24.10.4 (`opkg`, em
container, DNS quebrado de propósito): o mesmo comando gerado instalou o
iperf3 3.17.1 baixando os `.ipk` pela central. Achado: o `opkg -f <conf>`
**também lê `/etc/opkg/*.conf`** — tenta os feeds originais antes (cada um
espera o DNS, ~40 s no total) e depois instala pelos `nm_*`; funciona, mais
devagar. No container o SSH chega pelo NAT do Docker (172.17.0.1), endereço
que não leva de volta à central — a falha disse exatamente isso.

### 11. Testar de qualquer equipamento, numa tela só (adendo, 2026-10-19)

**Problema.** Na aba "Testes" do equipamento só havia "Testar caminho até
aqui"; não dava para medir de AP-assistencia até RB2011 sem sair do
servidor. O painel do teste ficava espremido entre o inspetor e o mapa, e o
resultado "do servidor" misturava a perna ponta a ponta com todas as pernas
enlace por enlace — muita informação de uma vez.

**Decisão** (com o dono do produto, em entrevista):

- **`end_to_end`** (novo `LinkTestKind`): de um equipamento até outro numa
  medição só, atravessando o caminho inteiro. A perna leva o caminho do mapa
  (`edgeIds` e `viaDeviceIds`/`viaNames` com **todos** os equipamentos do
  meio) — o mapa desenha e anima a perna por eles. Sem caminho conhecido, a
  medição roda do mesmo jeito, com aviso. Com a origem no próprio servidor, é
  o `server`.
- **`server` passa a ser só ponta a ponta.** Achar o gargalo é pedido à
  parte: "Medir enlace por enlace" (`path`, inclusive a partir do servidor),
  oferecido no resultado quando há outro equipamento com IP no meio.
- **Assistente "De / Até"** (`utils/linkTests/destinations.ts`): "De" vem com
  o equipamento aberto (ou o servidor); "Até" lista os vizinhos diretos
  ("Enlace direto" → `link`, inclusive atrás de switch sem IP) e o resto da
  rede ("Ponta a ponta · N saltos · por …"), com "Ponta a ponta / Enlace por
  enlace" para quem não é vizinho. O tipo sai da escolha, não de um cartão.
- **Tela do teste** (`LinkTestRunDialog` + `LinkTestRunView`): abre em
  janela, no meio (nem maximizada nem minimizada, pedido do dono), com
  "Maximizar/Restaurar" (`MaximizeToggle`) e minimizável para uma pílula no
  mapa (`LinkTestMiniCard`); a mesma tela no histórico. O assistente também
  abre em janela, com "Maximizar". Em cima, o
  **recorte do mapa** (`LinkTestPathMap`): o caminho **nas posições do
  próprio mapa** (escolha do dono), crescido por igual até cada par de
  vizinhos ter folga no seu eixo para a pílula dos Mbps (230 lado a lado, 150
  um abaixo do outro) e com a altura acompanhando a forma — os elementos, o
  estilo e a animação são os do mapa (`useMiniTopology`, sobre
  `utils/topology/elementSync.ts` e `ants.ts`, usados também pelo canvas).
- **Mapa de escolha no assistente** (`LinkTestPickerMap`): a rede nas
  posições do mapa, com a origem e o caminho escolhido em destaque; tocar num
  equipamento que pode ser destino o escolhe (a lista "Até" continua).
  Durante a medição: a etapa atual, os números grandes e o gráfico. No fim,
  **resumo primeiro** (`LinkTestResultView`): números, gargalo numa frase, uma
  linha por enlace, o que não rodou com o motivo, e os próximos passos.
  Pernas, etapas e transcrições ficam em "Detalhes técnicos".
- **Aba "Testes"** mostra as ações de teste do registro (`resolveActions`,
  grupo "Testes"): "Testar daqui até…", "Testar do servidor até aqui",
  "Diagnóstico rápido (ping)", "Repetir último teste".

**Achado em equipamento real.** "Do servidor até AP-assistencia" falhava às
vezes na volta com "O servidor iperf3 encerrou a conexão de controle" (a
mesma falha da captura do dono). O `iperf3 -s` do OpenWrt 25.12 (3.20), ainda
liberando a ida, fecha sem resposta a conexão que chega cedo — além do
`ACCESS_DENIED` que o cliente já tratava. Antes do `TEST_RUNNING`, nada foi
medido: o cliente nativo agora trata esse fechamento como "ocupado" e tenta
de novo (`closed_early`, teste `servidor_que_fecha_antes_de_comecar_…`).

**Evidência (rede real, pela tela).** AP-assistencia → AP-terraço (enlace
pelo switch): 366–367 / 223–236 Mbit/s. Servidor → AP-assistencia (ponta a
ponta, atravessando BORDA e o switch): 303–323 / 518–535 Mbit/s. Enlace por
enlace do mesmo par: Servidor → BORDA 300–363 / 563–659 e BORDA →
AP-assistencia 937 / 926 Mbit/s — o gargalo é o trecho até a BORDA.

### 12. CPU e memória durante a medição (adendo, 2026-10-21)

Vazão baixa com o processador no teto é outro diagnóstico que vazão baixa com
ele folgado — e a coleta periódica não vê um teste de 5 s (o `hrProcessorLoad`
do net-snmp é média de 1 min; o monitor roda de minuto em minuto).

- **Quem é lido:** as duas pontas da perna e os equipamentos do caminho
  (`viaDeviceIds`) que têm SNMP ligado e IP; a central pelo próprio `/proc`.
  Agente remoto não é lido. Só leitura, sempre.
- **Como:** `services/link_tests/health.rs`. Um `GET` por segundo com os
  contadores brutos da UCD-SNMP-MIB (`ssCpuRawUser/Nice/System/Idle/Wait` —
  no Linux o `System` já soma interrupção e softirq, onde o tráfego pesa) e a
  memória da mesma MIB; sem a UCD (RouterOS), o `hrProcessorLoad`. O uso vale
  **entre duas leituras diferentes do contador** (`CpuWindow`): o net-snmp
  renova os `ssCpuRaw*` a cada ~5 s, e repetir o último uso medido enquanto o
  contador não anda é mais honesto que zerar. Contrato `HealthReaders` no
  `LinkTestEnv` (o teste do orquestrador usa um dublê).
- **Relógio comum:** a perna zera um relógio ao começar a medir; vazão
  (`samples[].atMs`) e CPU/memória (`health[].atMs`) usam o mesmo, e o
  gráfico os desenha juntos. Perna antiga (sem `atMs`) cai no tempo de cada
  sentido.
- **Contrato:** evento efêmero `link_test:health` `{ testId, legSeq, deviceId,
  atMs, cpu, memory }`; coluna `link_test_legs.health` (migração
  `m20261021_000001`), com teto de 120 leituras por equipamento, só no `GET`
  de detalhe.
- **Tela:** sem cartão novo. No recorte do mapa, a linha sob o nome mostra o
  lido no teste (ao vivo, a última leitura; no fim, "CPU máx 97% · RAM 68%");
  sem leitura, a do mapa. O gráfico (`LinkTestTrafficChart`) junta ida/volta
  (eixo da esquerda) e CPU tracejado / RAM pontilhado de cada equipamento
  (eixo da direita, 0–100%, com a linha de 90%); passar o ponteiro (ou tocar)
  mostra os valores daquele segundo. O resumo ganha uma frase:
  "CPU do AP-Terraço chegou a 97% durante a medição — o processador é o
  provável limite" (≥ 90%) ou "…ficou em no máximo 40% — não é o
  processador" (< 60%).

**Evidência (rede real, leitura SNMP).** BORDA, AP-assistencia e AP-terraço
(OpenWrt, net-snmp) respondem os `ssCpuRaw*` e a memória; o contador anda a
cada ~5 s. O MikroTik (10.0.0.242) não tem a UCD e responde pelo
`hrProcessorLoad`.

## Consequências

- Porta nova publicada na central (2000/TCP), fechada a quem não tem sessão
  armada.
- Três tabelas novas, um evento efêmero e um de snapshot no barramento.
- O plugin do MikroTik (`mikrotik-routeros`) passa a existir; o OpenWrt ganha
  `openwrt-link-test` (usa `openwrt-packages` para instalar `iperf3` e
  `mikrotik-btest`); os papéis de iperf3 do Linux ficam num plugin próprio,
  `linux-link-test` (o `linux-ssh-status` continua só leitura).
- Medir o enlace satura o enlace: confirmação explícita, duração curta e
  cancelamento sempre à vista.
- UDP, jitter e perda ficam para uma segunda etapa (o contrato já reserva
  `TestProtocol`).
- O diagnóstico rápido por ping (§9) cobre o equipamento que não roda o
  teste — e nunca se apresenta como medida de banda.
