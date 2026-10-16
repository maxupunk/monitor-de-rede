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
| OpenWrt ↔ OpenWrt | `iperf3` (pacote oficial) | sim, sob demanda, pelo `openwrt-packages` |
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
  "actions": { "prepare": "tp_prepare", "serve": "tp_serve", "stop": "tp_stop", "run": "tp_run" }
}
```

- `serve`: motores que o equipamento hospeda; `drive`: motores que ele dirige
  (`auth: false` = cliente que só fala com servidor sem autenticação, caso do
  `mikrotik-btest` — que só conhece MD5, anterior ao RouterOS 6.43 — e dos
  clientes nativos). `drive[].maxDurationS` (opcional) é o teto do cliente por
  sentido: o `mikrotik_btest` recusa `-t` ≥ 30, então declara `29`; o plano
  limita a duração pedida e avisa.
- As ações referenciadas existem no manifesto; `prepare`, `serve` e `stop` são
  `write`; `run` é `read`. `prepare` é opcional (só quem instala algo).
- **Parâmetros fixos** (validados pela plataforma, o script não inventa):

| Ação | Parâmetros | Devolve |
|---|---|---|
| `prepare` | `{ engine, role: "serve"\|"drive" }` | `{ ready: bool, installed: [nomes] }` |
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
o que será instalado, o que falta (credencial, plugin ativo) e quanto tempo
leva. A tela mostra o plano e pede confirmação; só então
`POST /api/link-tests` com `confirm: true`.

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

## Consequências

- Porta nova publicada na central (2000/TCP), fechada a quem não tem sessão
  armada.
- Três tabelas novas, um evento efêmero e um de snapshot no barramento.
- O plugin do MikroTik (`mikrotik-routeros`) passa a existir; o OpenWrt ganha
  `openwrt-link-test` (usa `openwrt-packages` para instalar `iperf3` e
  `mikrotik-btest`); o `linux-ssh-status` ganha os papéis de iperf3.
- Medir o enlace satura o enlace: confirmação explícita, duração curta e
  cancelamento sempre à vista.
- UDP, jitter e perda ficam para uma segunda etapa (o contrato já reserva
  `TestProtocol`).
