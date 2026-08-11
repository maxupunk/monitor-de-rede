# Roadmap de Implementação — Backend Rust (Loco.rs)

> **Documento de execução.** Descreve, sem lacunas, tudo que precisa existir em `backend-rust/`
> para substituir integralmente o backend AdonisJS (`backend/`) mantendo o frontend Vue 3 +
> Vuetify funcionando. Cada item é verificável: nome de arquivo, assinatura de função, rota,
> payload e critério de aceite.
>
> **Fonte da verdade do comportamento atual:** `backend/` (AdonisJS v6). Nenhuma regra de
> negócio pode ser "simplificada" na migração sem estar registrada na seção
> [§17 Não-objetivos e desvios aceitos](#17-não-objetivos-e-desvios-aceitos).

---

## Índice

1. [Objetivo, escopo e princípios](#1-objetivo-escopo-e-princípios)
2. [Mapa de tradução AdonisJS → Loco.rs](#2-mapa-de-tradução-adonisjs--locors)
3. [Stack, crates e decisões técnicas](#3-stack-crates-e-decisões-técnicas)
4. [Estrutura de diretórios](#4-estrutura-de-diretórios)
5. [Convenções obrigatórias do contrato HTTP](#5-convenções-obrigatórias-do-contrato-http)
6. [Modelo de dados — migrations](#6-modelo-de-dados--migrations)
7. [Contrato completo da API](#7-contrato-completo-da-api)
8. [Módulos de domínio — função a função](#8-módulos-de-domínio--função-a-função)
9. [Processos de background](#9-processos-de-background)
10. [Autenticação e autorização](#10-autenticação-e-autorização)
11. [Tempo real (SSE) e streaming](#11-tempo-real-sse-e-streaming)
12. [Ajustes necessários no frontend](#12-ajustes-necessários-no-frontend)
13. [Configuração, ambiente e Docker](#13-configuração-ambiente-e-docker)
14. [Estratégia de testes](#14-estratégia-de-testes)
15. [Fases de execução](#15-fases-de-execução)
16. [Matriz de paridade funcional](#16-matriz-de-paridade-funcional)
17. [Não-objetivos e desvios aceitos](#17-não-objetivos-e-desvios-aceitos)
18. [Critérios de aceite (Definition of Done)](#18-critérios-de-aceite-definition-of-done)

---

## 1. Objetivo, escopo e princípios

### 1.1 Objetivo

Reescrever o backend do NetMonitor em Rust sobre **[Loco.rs](https://loco.rs/) 1.0**, com paridade
funcional total com o backend AdonisJS, substituindo:

- o `PingChecker` baseado em `execFile('ping', …)` por **ICMP nativo com [`surge-ping`](https://crates.io/crates/surge-ping)**;
- o `PortScannerService` baseado em `net.Socket` sequencial-por-lote pela **estratégia do
  [RustScan](https://github.com/RustScan/RustScan) sobre `tokio`** (batching adaptativo + `for_each_concurrent` + timeout adaptativo).

### 1.2 Escopo

**Dentro do escopo:**

| Domínio | Conteúdo |
| :--- | :--- |
| Persistência | 23 tabelas, todos os índices e FKs do esquema atual |
| API HTTP | ~90 endpoints REST + 2 SSE + 1 NDJSON |
| Monitoramento | 5 checkers (ping, http, tcp, dns, snmp), scheduler, processador de resultados |
| Descoberta | 6 scanners (ICMP, ARP, portas, mDNS, SSDP, SNMP), fila persistente, sessão ao vivo |
| SNMP | Cliente v1/v2c/v3, 6 coletores, poll/scan/apply-monitors, templates Zabbix |
| Topologia | LLDP/CDP, inferência por sub-rede, links manuais, grafo |
| Alertas | Motor de regras, catálogo, datasets, silenciamento, recuperação, 4 canais de notificação |
| Probes | Autenticação por token, heartbeat, fila de tarefas, agente, buffer offline, watchdog |
| VPN | WireGuard: chaves X25519 nativas, IPAM, 5 perfis de configuração, telemetria, preflight |
| Tempo real | EventBus + outbox cross-process + SSE |
| Ferramentas | Port scan, DNS benchmark/lookup/performance, teste SNMP avulso |
| Manutenção | Data pruner, resource cleanup |

**Fora do escopo (não regride, não avança):**

- Reescrever o frontend. Só são permitidos ajustes pontuais listados em [§12](#12-ajustes-necessários-no-frontend).
- Novas funcionalidades de produto. Este roadmap é migração, não evolução.
- Manter o `backend/` AdonisJS vivo depois do corte (ver [§15 Fase 9](#fase-9--corte-e-descomissionamento)).

### 1.3 Princípios inegociáveis

0. **O padrão do backend Rust tem precedência.** *(Decisão do responsável pelo projeto,
   Fase 0 — [ADR 006](adr/006-prioridade-do-padrao-rust.md). Lê-se antes do princípio 1 e o
   qualifica.)* O frontend Vue é nosso e fica no mesmo repositório: a diretriz é **aproveitá-lo
   e apenas adaptá-lo**. Onde preservar um formato herdado do AdonisJS custar contorcer o Rust
   — `rename` manual campo a campo, wrapper para imitar um envelope, serialização que o
   `sea-orm` não produz naturalmente —, vale o idiomático do backend e o frontend é ajustado.
   Em ordem: (a) preservar quando não custa nada; (b) adaptar quando custa; (c) **registrar
   sempre** na [§12](#12-ajustes-necessários-no-frontend); (d) adaptação é cirúrgica — mudar o
   tipo lido ou o nome de um campo, nunca redesenhar tela.

1. **O contrato HTTP é sagrado** — dentro do limite do princípio 0. O frontend não sabe que o
   backend mudou. Toda resposta JSON mantém nomes de campo, formato de data, envelope de
   paginação e códigos HTTP. Onde isso for impossível **ou caro**, o desvio vai para
   [§12](#12-ajustes-necessários-no-frontend) com o patch de frontend correspondente.
2. **Padrões Loco.rs.** Nada de "framework dentro do framework". Controllers em
   `src/controllers/`, entidades geradas em `src/models/_entities/`, regras em
   `src/models/*.rs`, migrations em `migration/src/`, jobs em `src/tasks/` e `src/workers/`,
   bootstrap em `src/app.rs`, respostas em `src/views/`.
3. **Domínio fora do controller.** Controller faz: extrair, validar, delegar, serializar.
   Toda regra de negócio vive em `src/services/` (módulos de domínio), testável sem HTTP.
4. **Erros explícitos.** `Result<T, loco_rs::Error>` em todo lugar. `unwrap()`/`expect()`
   apenas em `OnceLock`/constantes provadamente infalíveis.
5. **Sem `panic!` em caminho de rede.** Todo checker/scanner devolve um resultado degradado,
   nunca derruba a task.
6. **Comentários explicam o porquê.** O backend atual documenta decisões não óbvias
   (rollover de contador SNMP, janela de keepalive do WireGuard, truncamento de CIDR).
   Esses comentários **devem** ser portados — são a memória do projeto.

---

## 2. Mapa de tradução AdonisJS → Loco.rs

| Conceito AdonisJS | Equivalente Loco.rs | Observações |
| :--- | :--- | :--- |
| `start/routes.ts` | `Hooks::routes()` em `src/app.rs` + `fn routes()` por controller | Cada controller expõe `pub fn routes() -> Routes` |
| `app/controllers/*.ts` | `src/controllers/*.rs` | Handlers `async fn` com `#[debug_handler]` |
| `app/models/*.ts` (Lucid) | `src/models/_entities/*.rs` (SeaORM, gerado) + `src/models/*.rs` (regras) | `_entities` nunca é editado à mão |
| `database/migrations/*.ts` | `migration/src/m*.rs` (SeaORM Migration) | Registradas em `migration/src/lib.rs` |
| `modules/**` | `src/services/**` | Módulos de domínio puros |
| `commands/*.ts` (`node ace x`) | `src/tasks/*.rs` (`cargo loco task x`) | Registradas em `Hooks::register_tasks` |
| Processo `scheduler:run` | `src/tasks/scheduler_run.rs` (um ciclo por execução) | Scheduler nativo Loco, ver [§9](#9-processos-de-background) |
| `@vinejs/vine` | `validator` crate + structs `#[derive(Validate, Deserialize)]` | Já no `Cargo.toml` |
| `response.ok/created/...` | `format::json`, `format::empty_json`, `loco_rs::Error::*` | Ver [§5.5](#55-erros) |
| `EventBus` singleton | `tokio::sync::broadcast` no `AppContext` (via `Initializer`) | + tabela `event_outbox` cross-process |
| SSE manual (`res.write`) | `axum::response::sse::Sse` | `KeepAlive` nativo |
| `encryption.encrypt/decrypt` | `chacha20poly1305` com chave derivada de `APP_KEY` | Ver [§8.10](#810-vpn--srcservicesvpn) |
| `@adonisjs/auth` (tokens) | `loco_rs::auth::JWT` | Ver [§10](#10-autenticação-e-autorização) |
| `paginate()` do Lucid | Helper próprio `paginate_compat` | Formato do Lucid, não o do Loco. Ver [§5.4](#54-paginação) |
| Serialização camelCase | `#[serde(rename_all = "camelCase")]` em **todo** DTO | Regra global, ver [§5.1](#51-nomes-de-campo) |
| `ts-rs` (não existia) | `#[derive(TS)]` nos DTOs → `frontend/src/bindings` | Ganho novo, opcional para o frontend |

---

## 3. Stack, crates e decisões técnicas

### 3.1 `Cargo.toml` alvo

O scaffold atual já traz `loco-rs 1.0`, `sea-orm 2.0`, `axum 0.8`, `tokio 1.45`, `validator`,
`ts-rs`, `chrono`, `uuid`. **Acrescentar:**

```toml
[dependencies]
# --- runtime / async ---
tokio = { version = "1.45", default-features = false, features = [
  "rt-multi-thread", "net", "time", "sync", "macros", "process", "fs", "signal",
] }
tokio-stream = { version = "0.1", features = ["sync"] }
tokio-util = { version = "0.7" }
futures = { version = "0.3" }

# --- rede: ICMP nativo (substitui execFile('ping')) ---
surge-ping = { version = "0.8" }
# ⚠️ 0.6, e não 0.5: `surge-ping` 0.8 depende de `socket2 ^0.6`, e
# `sock_type_hint` recebe o `socket2::Type` daquela versão. Com as duas majors
# na árvore o tipo não unifica e o ICMP DGRAM não compila (ADR 003).
socket2 = { version = "0.6" }          # sock_type_hint / fallback DGRAM
rand = { version = "0.8" }             # identifier/sequence do ICMP

# --- rede: DNS (wire format + DoH) ---
hickory-proto = { version = "0.24", default-features = false }
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "json", "stream"] }

# --- rede: SNMP (ver spike SPIKE-01) ---
rasn = { version = "0.18" }
rasn-snmp = { version = "0.18" }

# --- rede: discovery e topologia ---
mdns-sd = { version = "0.13" }         # mDNS service discovery assíncrono (224.0.0.251:5353)
ssdp-client = { version = "0.4" }      # SSDP / UPnP device discovery (239.255.255.250:1900)
petgraph = { version = "0.7" }         # Grafo de topologia (ciclos, menor caminho, componentes)
phf = { version = "0.11", features = ["macros"] } # Tabela OUI (MAC vendor) O(1) sem alocação

# --- criptografia ---
x25519-dalek = { version = "2", features = ["static_secrets"] }
chacha20poly1305 = { version = "0.10" }
base64 = { version = "0.22" }
sha2 = { version = "0.10" }
hex = { version = "0.4" }

# --- artefatos VPN ---
qrcode = { version = "0.14", default-features = false, features = ["svg"] }

# --- utilitários ---
anyhow = { version = "1" }             # AppError::Internal (§8.1) — faltava nesta lista
ipnet = { version = "2.9" }            # CIDR IPv4 (discovery + VPN)
serde_yaml = { version = "0.9" }       # fixtures / seeds
thiserror = { version = "2" }
num_cpus = { version = "1.16" }
```

> **Trava de versão:** fixar as versões no `Cargo.lock` na Fase 0 e não subir major durante a
> migração. Atualização de dependência é tarefa pós-corte.

### 3.2 Decisão: **ping via `surge-ping`** (obrigatório)

O checker atual executa o binário `ping` do sistema e faz *regex* na saída — frágil em três
frentes (idioma do SO, BusyBox vs iputils, custo de `fork()` por checagem). A substituição:

```rust
// src/services/monitoring/checkers/ping.rs
pub struct PingConfig {
    pub host: String,
    pub packet_count: u16,   // default 3
    pub packet_size: usize,  // default 56
    pub timeout_ms: u64,     // default 5000
}

pub async fn execute(cfg: &PingConfig) -> CheckResult;
```

Regras de implementação:

1. **Um `surge_ping::Client` por processo**, criado no `Initializer` e guardado no `AppContext`
   (abrir socket raw por checagem esgota descritores e exige privilégio a cada vez).
2. **Identificador/sequência**: `PingIdentifier(rand::random())`, `PingSequence(n)` incremental
   por pacote — o `Client` multiplexa respostas por identificador.
3. **Privilégio**:
   - Linux (produção/Docker): `CAP_NET_RAW` no container, **ou** socket `SOCK_DGRAM` ICMP
     (`Config::builder().sock_type_hint(socket2::Type::DGRAM)`) com
     `sysctl net.ipv4.ping_group_range="0 2147483647"`. **Preferir DGRAM** — dispensa capability.
   - ~~Windows (dev local): raw socket exige processo elevado. Implementar `#[cfg(windows)]`
     *fallback* para o binário `ping.exe`…~~ **Cancelado pelo SPIKE-03 (ADR 003):** medido,
     o `SOCK_DGRAM` funciona no Windows sem elevação. O *fallback* **não deve ser escrito** —
     um caminho alternativo que nunca é exercitado é dívida garantida, e o parsing por idioma
     do SO é justamente o defeito que esta migração remove.
4. **Resultado idêntico ao atual**: `metrics = [latency(ms), packet_loss(%)]`;
   `status = up` (0% perda) / `warning` (perda parcial) / `down` (100%).
   Mensagens em português, mesmo texto.
5. **Concorrência**: o ping é usado tanto no monitor quanto no *sweep* ICMP do discovery.
   O sweep usa o mesmo `Client` com `for_each_concurrent(64, …)` — sem `fork()`, o lote pode
   ser bem maior que os 20 atuais.

**Critério de aceite:** `PingChecker` responde corretamente para host up, host down e host
inexistente, em Linux com DGRAM e sem capability adicional; latência medida bate (±10%) com
`ping` do sistema.

### 3.3 Decisão: **port scanner estilo RustScan sobre `tokio`** (obrigatório)

O `PortScannerService` atual usa `DEFAULT_CONCURRENCY = 16` fixo. A estratégia RustScan:

```rust
// src/services/network_tools/port_scanner.rs
pub struct ScanStrategy {
    /// Portas processadas por lote. Derivado do ulimit (Unix) ou fixo (Windows).
    pub batch_size: usize,
    /// Timeout inicial por porta.
    pub timeout: Duration,
    /// Ajuste adaptativo: média móvel do RTT das conexões bem-sucedidas.
    pub adaptive: bool,
}

pub async fn scan(
    host: IpAddr,
    ports: &[u16],
    protocol: PortProtocol,
    strategy: ScanStrategy,
    on_result: mpsc::Sender<PortScanItem>,
    cancel: CancellationToken,
) -> Result<Vec<PortScanItem>>;
```

Regras:

1. **Batch size derivado do `ulimit -n`** (RustScan §*ulimit adjustment*):
   `batch = clamp(soft_limit.saturating_sub(100), 16, 4096)`. Em Windows, `batch = 512`.
   Nunca exceder o limite de descritores — é a causa nº 1 de falso negativo em varredura.
2. **Concorrência** via `futures::stream::iter(ports).for_each_concurrent(batch, …)`, cada
   item sendo `tokio::time::timeout(t, TcpStream::connect((host, port)))`.
3. **Timeout adaptativo**: começa em `timeout_ms` (default 1500 ms); a cada conexão aberta
   com sucesso, recalcula `t = clamp(rtt_médio * 3, 100ms, timeout_ms)`. Reduz drasticamente
   o tempo total em redes locais sem perder portas em links lentos.
4. **Preservar a semântica de status atual**: TCP → `open` / `closed`.
   UDP → `open` (resposta recebida) / `closed` (`ECONNREFUSED`, isto é, ICMP port unreachable)
   / `open|filtered` (silêncio). Manter o `UdpProbeRegistry` com os **mesmos payloads binários**
   (DNS 53, NTP 123, NetBIOS 137, SNMP 161, SSDP 1900, mDNS 5353, default `0x00`).
5. **Preservar o comentário de projeto**: equipamentos embarcados saturam com concorrência
   alta. Por isso o *cap* de segurança: quando o alvo estiver na LAN e for classificado como
   `router`/`access_point`/`printer`, limitar `batch` a 64. Registrar isso no código.
6. **Cancelamento**: `CancellationToken` ligado ao `on_disconnect` da resposta HTTP — igual
   ao `AbortController` atual. Lote em voo termina; nada novo é iniciado.
7. **Tabela de nomes de serviço** (`TCP_SERVICE_NAMES`, `UDP_SERVICE_NAMES`) portada
   integralmente como `phf`/`match` estático.

> **Sobre usar a crate `rustscan` diretamente:** a crate publicada é orientada ao binário e sua
> API pública não é estável para embutir. **SPIKE-02** (Fase 0) avalia; se a API servir, usar;
> caso contrário, implementar a estratégia acima (que é o algoritmo do RustScan) em
> `src/services/network_tools/port_scanner.rs`. Em ambos os casos o comportamento externo é o
> mesmo e o roadmap não muda.

**Critério de aceite:** varredura de 1024 portas TCP em host da LAN termina em < 3 s (hoje:
dezenas de segundos), sem falso negativo em relação ao `nmap -sT`; NDJSON chega ao frontend
porta a porta.

### 3.4 Spikes obrigatórios da Fase 0 — 🟢 **concluídos**

| ID | Assunto | Resposta | ADR |
| :--- | :--- | :--- | :--- |
| **SPIKE-01** | Cliente SNMP | **Sim**, `rasn-snmp` 0.18 cobre `get` e `walk` com transporte próprio em `tokio`. Achado: `EncodeError`/`DecodeError` não implementam `std::error::Error` — o cliente precisa de um `SnmpError` com `From` explícito. | [001](adr/001-snmp-client.md) |
| **SPIKE-02** | RustScan como crate | **Não embutir.** A crate é `GPL-3.0-only` (o projeto é MIT) e `Scanner::run() -> Vec<SocketAddr>` não entrega resultado incremental nem aceita cancelamento — o NDJSON da §7.15 e o `CancellationToken` da §3.3.6 são impossíveis com ela. Implementar o algoritmo. | [002](adr/002-rustscan-embedding.md) |
| **SPIKE-03** | ICMP sem privilégio | **Sim**, `SOCK_DGRAM` sem `CAP_NET_RAW`, como usuário não-root, com latência a ~3% do `ping` do sistema. O *fallback* Windows foi cancelado. | [003](adr/003-icmp-dgram.md) |
| **SPIKE-04** | DNS wire | **Sim**, um encoder só serve UDP, TCP e DoH; o `Instant` fica isolado no round-trip. | [004](adr/004-dns-wire.md) |
| **SPIKE-05** | Scheduler | Task de **um ciclo** disparada pelo scheduler nativo, em processo separado — confirma a [§9.1](#91-topologia-de-processos-espelha-o-docker-composeyml). Boot medido: ~25 ms num tique de 5 s (0,5%). | [005](adr/005-scheduler-loco.md) |

Protótipos executáveis em `backend-rust/examples/spikes/`:

```sh
cargo run --example spike_snmp_v2c      # offline; SNMP_TARGET=host:161 para ao vivo
cargo run --example spike_dns_wire      # UDP + TCP + DoH
cargo run --example spike_icmp_dgram -- 1.1.1.1
docker compose -f docker-compose.icmp-spike.yml run --rm icmp-dgram     # SPIKE-03 em Linux
docker compose -f docker-compose.icmp-spike.yml run --rm icmp-restrito  # contraprova
```

Além dos cinco, a Fase 0 registrou o [ADR 006](adr/006-prioridade-do-padrao-rust.md), que
define a precedência entre o padrão do backend Rust e o contrato herdado do AdonisJS.

---

## 4. Estrutura de diretórios

```
backend-rust/
├── Cargo.toml
├── config/
│   ├── development.yaml
│   ├── test.yaml
│   └── production.yaml
├── migration/
│   └── src/
│       ├── lib.rs                     # registra as 23 migrations
│       ├── m20220101_000001_users.rs  # (do scaffold, estendido)
│       └── m2026…_*.rs                # uma por tabela, ordem de FK
└── src/
    ├── app.rs                         # Hooks: routes, workers, tasks, initializers, seed
    ├── lib.rs
    ├── bin/main.rs
    ├── controllers/
    │   ├── mod.rs
    │   ├── auth.rs                    # (scaffold) + /auth/me compat
    │   ├── sites.rs
    │   ├── networks.rs
    │   ├── devices.rs
    │   ├── monitors.rs
    │   ├── discovery.rs
    │   ├── topology.rs
    │   ├── snmp.rs
    │   ├── probes.rs
    │   ├── alerts.rs
    │   ├── events.rs
    │   ├── vpn_servers.rs
    │   ├── vpn_peers.rs
    │   ├── zabbix_templates.rs
    │   ├── port_scan.rs
    │   ├── dns.rs
    │   ├── dns_servers.rs
    │   └── dashboard.rs
    ├── models/
    │   ├── mod.rs
    │   ├── _entities/                 # SeaORM gerado — NÃO editar
    │   └── <tabela>.rs                # regras: computed, hooks, queries nomeadas
    ├── dtos/                          # DTOs de entrada e saída (serde + validator + ts-rs)
    │   ├── mod.rs
    │   ├── common.rs                  # PaginationMeta, Page<T>, ApiError
    │   ├── monitor.rs
    │   ├── device.rs
    │   ├── alert.rs
    │   ├── vpn.rs
    │   ├── discovery.rs
    │   ├── dns.rs
    │   └── …
    ├── views/                         # serializadores (equivalente aos `serialize*` atuais)
    ├── services/                      # ← todos os `modules/` do Adonis
    │   ├── mod.rs
    │   ├── shared/
    │   │   ├── errors.rs
    │   │   ├── crypto.rs              # APP_KEY, encrypt/decrypt, sha256
    │   │   └── pagination.rs
    │   ├── monitoring/
    │   │   ├── contracts.rs           # CheckResult, CheckMetric, MonitorStatus
    │   │   ├── runner.rs
    │   │   ├── result_processor.rs
    │   │   ├── device_status.rs
    │   │   ├── interface_monitoring.rs
    │   │   ├── link_speed.rs
    │   │   ├── presenter.rs
    │   │   └── checkers/{ping,http,tcp,dns,snmp}.rs
    │   ├── network_tools/
    │   │   ├── port_scanner.rs        # RustScan/tokio
    │   │   ├── udp_probes.rs
    │   │   └── dns/{latency,registry,wire}.rs
    │   ├── discovery/
    │   │   ├── service.rs, queue.rs, session.rs, merger.rs,
    │   │   ├── cidr_range.rs, device_identifier.rs, oui_lookup.rs
    │   │   └── scanners/{icmp,arp,ports,mdns,ssdp,snmp}.rs
    │   ├── snmp/
    │   │   ├── client.rs, session.rs, service.rs
    │   │   └── collectors/{system,interface,traffic,cpu,memory,lldp,value}.rs
    │   ├── topology/{service,link_resolver,builder,confidence}.rs
    │   ├── alerts/
    │   │   ├── manager.rs, evaluator.rs, repository.rs,
    │   │   ├── recovery.rs, silence.rs, fields.rs, contracts.rs
    │   │   ├── catalog/{service,templates}.rs
    │   │   └── datasets/{monitor_result,interface_state,vpn_peer}.rs
    │   ├── events/{bus,relay}.rs
    │   ├── notifications/{service,formatter,channels/*}.rs
    │   ├── probes/{dispatcher,receiver,liveness,agent,buffer}.rs
    │   ├── vpn/
    │   │   ├── server_service.rs, peer_service.rs, ip_allocator.rs,
    │   │   ├── key_generator.rs, config_builder.rs, config_writer.rs,
    │   │   ├── peer_status.rs, traffic_recorder.rs, state_watcher.rs,
    │   │   ├── monitor_provisioner.rs, preflight.rs, secret_store.rs,
    │   │   ├── probe_registrar.rs, access_control.rs, cidr.rs, peer_hints.rs
    │   │   └── profiles/{contract,registry,mikrotik,openwrt,wg_conf,variants}.rs
    │   ├── zabbix/{parser,collector,monitor_sync}.rs
    │   └── maintenance/{data_pruner,resource_cleanup}.rs
    ├── tasks/
    │   ├── mod.rs
    │   ├── scheduler_run.rs
    │   ├── probe_run.rs
    │   ├── probe_register.rs
    │   ├── vpn_probe_register.rs
    │   ├── network_scan.rs
    │   ├── snmp_poll.rs
    │   └── monitor_test.rs
    ├── workers/                       # jobs pontuais assíncronos (BackgroundAsync)
    │   ├── mod.rs
    │   ├── snmp_poll_worker.rs        # poll inicial após salvar device
    │   └── discovery_worker.rs        # varredura ao vivo desacoplada da request
    ├── initializers/
    │   ├── mod.rs
    │   ├── event_bus.rs               # broadcast channel no AppContext
    │   ├── ping_client.rs             # surge_ping::Client compartilhado
    │   ├── alert_rules.rs             # ensure_defaults do catálogo
    │   ├── dns_servers.rs             # seed dos resolvedores públicos
    │   └── vpn_probe.rs               # registro idempotente do vpn-probe
    ├── mailers/                       # (scaffold: auth)
    └── fixtures/
```

---

## 5. Convenções obrigatórias do contrato HTTP

Estas regras existem porque o frontend não muda. Violar qualquer uma delas quebra tela.

### 5.1 Nomes de campo

**Todo** struct serializado para a API leva:

```rust
#[derive(Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MonitorDto { … }
```

O banco continua `snake_case`; a fronteira HTTP é `camelCase`. Idem para **entrada**:
`#[serde(rename_all = "camelCase")]` no DTO de request, porque o frontend envia
`intervalSeconds`, `snmpCommunity`, `zabbixTemplateId` etc.

**Exceção documentada:** `POST /api/topology/links` recebe `source_device_id`,
`target_device_id`, `source_interface_id`, `target_interface_id` em **snake_case** (o
controller Adonis valida assim e o frontend envia assim). Manter snake_case só nesse DTO.
Idem `GET /api/topology?site_id=`.

### 5.2 Datas

- Padrão: **ISO-8601 com offset** (`2026-08-10T14:32:11.482-03:00` ou `…Z`).
  `chrono::DateTime<Utc>` com `serde` já produz RFC-3339 — compatível com `DateTime.toISO()`
  do Luxon.
- `null` quando a coluna é nula. Nunca string vazia.
- **Exceção obrigatória:** `GET /api/devices/:id/metrics` e `GET /api/devices/:id/events`
  devolvem `createdAt` **formatado** como `dd/MM/yyyy HH:mm:ss` (o frontend exibe direto).
  Implementar `fn format_br(dt: DateTime<Utc>) -> String` e usar só nesses dois endpoints.

### 5.3 Tipos numéricos

- `bytes_rx`/`bytes_tx` são `bigint` no banco. O Postgres devolve como string em alguns
  drivers; o modelo atual normaliza para `number`. Em Rust: `i64` serializado como número
  JSON. **Não** usar string.
- `latency_ms` é `float` nullable → `Option<f64>`.
- Percentuais arredondados com a mesma precisão do backend atual (ex.: `successRate` com
  1 casa, `avgLookupTimeMs` com 3 casas).

### 5.4 Paginação

O frontend (`useInfiniteList`) espera **o envelope do Lucid**, não o do Loco:

```jsonc
{
  "data": [ … ],
  "meta": {
    "total": 137,
    "perPage": 20,
    "currentPage": 3,
    "lastPage": 7,
    "firstPage": 1,
    "firstPageUrl": "/?page=1",
    "lastPageUrl": "/?page=7",
    "nextPageUrl": "/?page=4",
    "previousPageUrl": "/?page=2"
  }
}
```

Campos **usados** pelo frontend: `total`, `currentPage`, `lastPage`. Os demais entram por
compatibilidade defensiva. Implementar em `src/services/shared/pagination.rs`:

```rust
pub struct LucidMeta { pub total: u64, pub per_page: u64, pub current_page: u64,
                       pub last_page: u64, pub first_page: u64, … }
pub struct LucidPage<T> { pub data: Vec<T>, pub meta: LucidMeta }

pub async fn paginate_compat<E, T>(
    db: &DatabaseConnection,
    query: Select<E>,
    page: u64,
    limit: u64,
    map: impl Fn(E::Model) -> T,
) -> Result<LucidPage<T>>;
```

**Regra do `limit`:** `min(limit, 100)`, default 20 — igual ao Adonis.

**Regra do modo dual:** vários endpoints (`GET /api/alerts`, `/api/discovery/runs`,
`/api/devices/:id/metrics`, `/api/devices/:id/events`) devolvem **array cru** quando `?page`
está ausente e **envelope paginado** quando presente. Isso é comportamento observável do
frontend — replicar exatamente. Representar com:

```rust
#[serde(untagged)]
pub enum MaybePaged<T> { Page(LucidPage<T>), List(Vec<T>) }
```

Limites do modo array (do Adonis): alertas 100, métricas 1000, eventos 50, runs sem limite.

### 5.5 Erros

Mapear `loco_rs::Error` para as respostas que o frontend entende (`apiService.handleResponse`
lê `message` ou `errors[].message`):

| Situação | HTTP | Corpo |
| :--- | :---: | :--- |
| Validação | 422 | `{"message": "…"}` ou `{"errors":[{"field":"cidr","message":"…"}]}` |
| Não encontrado | 404 | `{"message":"…"}` |
| Conflito (ex.: DNS duplicado) | 409 | `{"message":"…"}` |
| Não autorizado | 401 | `{"message":"…"}` — o frontend limpa `auth_token` |
| Rate limit (VPN) | 429 | `{"message":"…"}` + header `Retry-After` |
| Regra de negócio | 400 | `{"message":"…"}` |
| Aceito/assíncrono | 202 | payload específico |

**Todas as mensagens em português**, texto idêntico ao do backend atual (são exibidas em
snackbars). Implementar `src/services/shared/errors.rs` com um `AppError` `thiserror` +
`impl IntoResponse`.

### 5.6 Prefixo e CORS

- Todas as rotas de negócio sob `/api`. `AppRoutes::with_default_routes().prefix("/api")`
  para o grupo de negócio; `GET /` mantém `{"status":"online","service":"Network Monitor API","version":"1.0.0"}`.
- CORS liberado para a origem do frontend (`config/*.yaml`, middleware `cors` do Loco).
- **`server.port` = 3333** em todos os ambientes (o proxy do Vite e o docker-compose apontam
  para 3333). O scaffold vinha com 5150 — 🟢 corrigido na Fase 0 nos três ambientes.

---

## 6. Modelo de dados — migrations

23 tabelas. Ordem de criação obrigatória (dependência de FK). Cada migration é um arquivo em
`migration/src/` registrado em `migration/src/lib.rs`.

> **Regra:** tipos, nulabilidade, defaults, `unique` e **todos os índices nomeados** devem ser
> idênticos aos do Adonis. Os índices existentes foram escolhidos com justificativa escrita nas
> migrations atuais — portar os comentários.

| # | Migration | Tabela | Pontos de atenção |
| :-: | :--- | :--- | :--- |
| 01 | `m…_users` | `users` | Do scaffold Loco (pid, api_key, tokens de verificação/reset/magic-link). Estender com `active bool NOT NULL DEFAULT true` |
| 02 | `m…_sites` | `sites` | `name`, `description?`, `location?`, `active` |
| 03 | `m…_probes` | `probes` | `site_id? → sites CASCADE`, `token_hash` (índice **não** único: `DEFAULT_VPN_PROBE_TOKEN` é compartilhado), `status` default `pending`, `configuration jsonb?` |
| 04 | `m…_networks` | `networks` | `site_id?` (opcional de propósito), `probe_id? SET NULL`, `cidr`, `dns_servers jsonb?`, `scan_enabled`, `scan_interval` default 3600, `last_scan_at?`, `next_scan_at?` |
| 05 | `m…_zabbix_templates` | `zabbix_templates` | `zabbix_uuid?`, `raw_export jsonb NOT NULL`, `imported_at` |
| 06 | `m…_zabbix_template_items` | `zabbix_template_items` | `template_id CASCADE`, `key`, `snmp_oid`, `value_type`, `units?`, `multiplier?`; índice `template_id` |
| 07 | `m…_devices` | `devices` | FKs `site_id CASCADE`, `network_id SET NULL`, `parent_id SET NULL` (auto-referência), `zabbix_template_id SET NULL`; **UNIQUE `(network_id, ip_address)`**; índices `ip_address`, `name`, `site_id`, `zabbix_template_id` |
| 08 | `m…_device_interfaces` | `device_interfaces` | `device_id CASCADE`, `snmp_index?`, `speed bigint?`, `admin_status?`, `oper_status?`; índice `(device_id, snmp_index)` |
| 09 | `m…_device_links` | `device_links` | 2 FKs para `devices` CASCADE + 2 para `device_interfaces` SET NULL; `confidence` default 100; índices `(source,target)` e `target` |
| 10 | `m…_monitors` | `monitors` | `device_id? CASCADE`, `probe_id? SET NULL`, `configuration jsonb NOT NULL`, `interval_seconds` 15, `timeout_seconds` 10, `retry_count` 3, `next_run_at?`, `last_run_at?`, `status` default `unknown`; índices `(enabled, next_run_at)` e `(device_id, enabled)` |
| 11 | `m…_monitor_results` | `monitor_results` | Tabela de maior volume. Só 2 índices: `(monitor_id, started_at)` e `(created_at)` |
| 12 | `m…_metrics` | `metrics` | 4 índices: `(device_id, interface_id, name, recorded_at)`, `(device_id, name, recorded_at)`, `(interface_id, recorded_at)`, `(created_at)` |
| 13 | `m…_discovery_runs` | `discovery_runs` | índices `(status, id)`, `(network_id, status)`, `(created_at)` |
| 14 | `m…_discovery_results` | `discovery_results` | Cache do último scan (sem histórico). índice `discovery_run_id` |
| 15 | `m…_alert_rules` | `alert_rules` | `template_key?` (idempotência do catálogo) + índice; `condition jsonb` |
| 16 | `m…_alert_events` | `alert_events` | `scope_key?` + índices `(scope_key)`, `(alert_rule_id, scope_key, status)`, `(device_id, created_at)`, `(monitor_id, created_at)` |
| 17 | `m…_vpn_servers` | `vpn_servers` | `private_key_encrypted text NOT NULL` (cifrado em repouso), `network_id CASCADE` |
| 18 | `m…_vpn_peers` | `vpn_peers` | `public_key UNIQUE`, `preshared_key_encrypted?`, `device_id UNIQUE`, `bytes_rx/tx bigint`, `last_connection_status?`; índice `(vpn_server_id, enabled)` |
| 19 | `m…_dns_servers` | `dns_servers` | UNIQUE `(address, protocol)` |
| 20 | `m…_event_outbox` | `event_outbox` | `bigserial`, `origin` (id do processo emissor), `payload jsonb`; índice `created_at` |
| 21 | `m…_probe_tasks` | `probe_tasks` | `monitor_id UNIQUE` (uma tarefa pendente por monitor); índice `(probe_id, id)` |
| 22 | `m…_system_settings` | `system_settings` | `key varchar(100) UNIQUE`, `value text?` |
| 23 | `m…_auth_tokens` | *(opcional)* | Só se a Fase 6 optar por tokens opacos em vez de JWT puro. Ver [§10](#10-autenticação-e-autorização) |

Depois de cada migration: `cargo loco db entities` para regenerar `src/models/_entities/`
— **contra o PostgreSQL**, nunca contra o SQLite (ver a nota de portabilidade na
[Fase 1](#fase-1--esquema-e-entidades-)). Em seguida,
`cargo run --example schema_parity` para conferir a paridade com o esquema do AdonisJS.

### 6.1 Regras de modelo (`src/models/*.rs`)

Campos **computados** que o frontend consome e que hoje vêm de `@computed()` do Lucid — devem
ser calculados no DTO/view, nunca perdidos:

| Modelo | Computado | Regra |
| :--- | :--- | :--- |
| `Monitor` | `target` | `configuration.host ?? url ?? domain ?? ""` |
| `Monitor` | `port` | `configuration.port` |
| `Monitor` | `isEnabled` | espelho de `enabled` |
| `AlertRule` | `isEnabled` | espelho de `enabled` |
| `VpnPeer` | `connectionStatus` | máquina de estados de janela temporal — ver [§8.10.3](#8103-status-do-túnel-porte-literal) |
| `Network` | `scannable`, `usableHosts` | derivados de `cidr` |

---

## 7. Contrato completo da API

Todas as rotas sob `/api`. **Legenda:** ✅ = paridade obrigatória byte-a-byte no payload.

### 7.1 Raiz e Dashboard

| Método | Rota | Handler | Payload / Resposta |
| :--- | :--- | :--- | :--- |
| GET | `/` | — | `{status, service, version}` |
| GET | `/api/dashboard/layout` | `dashboard::get_layout` | `{layout: Json[] \| null, updatedAt: string \| null}` — lê `system_settings.key='dashboard_layout'` ✅ |
| POST | `/api/dashboard/layout` | `dashboard::save_layout` | in `{layout: any[], clientId?: string}` → `{success:true, updatedAt}` + evento SSE `dashboard:layout_updated` ✅ |

### 7.2 Autenticação

| Método | Rota | Handler | Observação |
| :--- | :--- | :--- | :--- |
| POST | `/api/auth/login` | `auth::login` | in `{email, password}` → `{token, user:{id,email,fullName,role}}` — ver [§10](#10-autenticação-e-autorização) |
| POST | `/api/auth/logout` | `auth::logout` | → `{message}` |
| GET | `/api/auth/me` | `auth::me` | JWT → objeto `User` **plano** |
| *(novas, do scaffold Loco — manter)* | `/api/auth/register`, `/verify/:token`, `/forgot`, `/reset`, `/magic-link` | | Não usadas pelo frontend hoje; manter registradas |

### 7.3 Sites

| Método | Rota | Notas |
| :--- | :--- | :--- |
| GET | `/api/sites` | array de sites |
| POST | `/api/sites` | `{name, description?, location?, active?}` → 201 |
| GET | `/api/sites/:id` | |
| PUT | `/api/sites/:id` | |
| DELETE | `/api/sites/:id` | 204 — usa `ResourceCleanupService::delete_site` (cascata manual) ✅ |

### 7.4 Networks

| Método | Rota | Notas |
| :--- | :--- | :--- |
| GET | `/api/networks` | cada item **enriquecido** com `site`, `scannable`, `usableHosts` ✅ |
| POST | `/api/networks` | campos: `siteId, probeId, name, cidr, gateway, vlan, dnsServers, scanEnabled, scanInterval, active` → 201 enriquecido |
| GET | `/api/networks/:id` | enriquecido |
| PUT | `/api/networks/:id` | enriquecido (a store do frontend substitui a linha pela resposta) |
| DELETE | `/api/networks/:id` | 204 |
| POST | `/api/networks/:id/scan` | **Não varre no processo HTTP.** Enfileira `DiscoveryRun` pendente → 202 `{message, alreadyQueued, run, usableHosts, truncated}`; 422 se CIDR inválido ✅ |

### 7.5 Devices

| Método | Rota | Notas |
| :--- | :--- | :--- |
| GET | `/api/devices` | preload `site`, `parent` |
| POST | `/api/devices` | 16 campos aceitos; efeitos: `sync_device_monitor`, `sync_zabbix_template_monitor`, apaga `discovery_results` com mesmo IP, agenda poll SNMP em background ✅ |
| GET | `/api/devices/:id` | preload `site`, `parent`, `vpnPeer`, `zabbixTemplate.items` |
| PUT | `/api/devices/:id` | mesmos efeitos do POST |
| DELETE | `/api/devices/:id` | 204 via `ResourceCleanupService::delete_device` |
| GET | `/api/devices/:id/monitors` | **mesmo payload de `GET /api/monitors`** filtrado — inclui `recentResults`, `gaugeMetric`, `gaugeHistory` ✅ |
| GET | `/api/devices/:id/metrics` | modo dual (array ≤1000 ou paginado); filtra métricas de interface com `adminStatus != 'up'`; campos `{id, deviceId, interfaceId, interfaceName, metricName, metricValue, unit, createdAt(dd/MM/yyyy HH:mm:ss)}` ✅ |
| GET | `/api/devices/:id/events` | modo dual (array ≤50 ou paginado); campos `{id, deviceId, eventType(=status), severity, message, createdAt(dd/MM/yyyy HH:mm:ss)}` ✅ |
| GET | `/api/devices/:id/interfaces` | via `snmp::interfaces` — devolve campos duplicados `ifIndex/ifName/ifDescr/ifAdminStatus/ifOperStatus/ifSpeed/ifType` + os originais ✅ |

### 7.6 Monitors

| Método | Rota | Notas |
| :--- | :--- | :--- |
| GET | `/api/monitors` | `presentMonitors`: `recentResults` (até 30 por monitor, **ordem cronológica crescente**, via window function), `gaugeMetric`, `gaugeHistory` (20), `target`, `port`, `latencyMs` ✅ |
| POST | `/api/monitors` | aceita `target`/`port` e monta `configuration` por tipo (`buildConfiguration`) ✅ |
| GET | `/api/monitors/:id` | + `recentResults` (100, crescente), `gaugeMetric`, `gaugeHistory`, `stats{avgLatency,minLatency,maxLatency,lastLatency,uptimePercentage,totalChecks,upChecks}` ✅ |
| PUT | `/api/monitors/:id` | ao desabilitar, resolve alertas abertos do monitor |
| DELETE | `/api/monitors/:id` | resolve alertas + `ResourceCleanupService::delete_monitor` |
| POST | `/api/monitors/:id/run` | execução síncrona + `process_result` → `{message, result}` |
| POST | `/api/monitors/:id/enable` | + recalcula status do device |
| POST | `/api/monitors/:id/disable` | + resolve alertas + recalcula status do device |
| GET | `/api/monitors/:id/results` | paginado (envelope Lucid) |
| GET | `/api/monitors/:id/alerts` | paginado; `where monitorId = :id OR scopeKey = 'monitor::id'`; payload serializado do alerta ✅ |

### 7.7 Discovery

| Método | Rota | Notas |
| :--- | :--- | :--- |
| GET | `/api/discovery/scan-state` | `{data: ScanSessionState}` (estado em memória do processo HTTP) |
| GET | `/api/discovery/scan-stream` | **SSE**: `data: <ScanSessionState>` a cada mudança + estado imediato ao conectar |
| POST | `/api/discovery/scan` | `{networkId}` → 202 `{runId, status:"running"}`; roda em background desacoplado da request; 400 se CIDR não varredurável |
| POST | `/api/discovery/scan-cancel` | → `{status:"cancelled"}` |
| GET | `/api/discovery/runs` | modo dual; cada run com `devicesFound` (count), `cidr`, `networkName` ✅ |
| GET | `/api/discovery/runs/:id` | run + `results` |
| DELETE | `/api/discovery/cleanup?olderThanDays=7` | → `{removedRuns, message}` |

`ScanSessionState`: `{runId, networkId, status, phase, progressCurrent, progressTotal, hosts[], logs[] (últimos 20), error, startedAt, finishedAt}`.
`phase ∈ {icmp, discovery, ports, snmp, idle}`; `status ∈ {idle, running, completed, cancelled, failed}`.
`hosts[]`: `{ipAddress, macAddress, hostname, mdnsName, vendor, deviceType, openPorts, confidence, data}` (nulos explícitos).

### 7.8 Topologia

| Método | Rota | Notas |
| :--- | :--- | :--- |
| GET | `/api/topology?site_id=` | `{nodes[], edges[]}`; inclui aresta virtual para `parentId` com `id` negativo `-(dev.id*1000 + parentId)` ✅ |
| POST | `/api/topology/links` | **snake_case**: `{source_device_id, target_device_id, source_interface_id?, target_interface_id?}` → 201 |
| DELETE | `/api/topology/links/:id` | `{message}` ou 404 |
| POST | `/api/topology/recalculate` | `{message, inferredCount}` |

### 7.9 SNMP

| Método | Rota | Notas |
| :--- | :--- | :--- |
| POST | `/api/snmp/test` | `{host, port?, version?, community?, autoDetect?}` → resultado de `testConnection` ou `detectConnection` |
| POST | `/api/devices/:id/snmp/poll` | `{host?, version?, community?, port?}` → `{message, result}` |
| POST | `/api/devices/:id/snmp/scan` | → `{systemInfo, cpuInfo, memoryInfo, interfaces[], hasCpuMonitor, hasMemoryMonitor, zabbixTemplateItems[], snmpResponded}` ✅ |
| POST | `/api/devices/:id/snmp/apply-monitors` | `{enableCpuMonitor?, enableMemoryMonitor?, monitoredIfIndexes?[]}` → `{message}`; cria/atualiza interfaces + monitores, apaga métricas de itens desmarcados, roda poll inicial ✅ |
| GET | `/api/devices/:id/interfaces` | ver §7.5 |

### 7.10 Probes

| Método | Rota | Auth | Notas |
| :--- | :--- | :--- | :--- |
| GET | `/api/probes` | usuário | lista |
| POST | `/api/probes` | usuário | gera `tokenHash` se ausente |
| GET/PUT/DELETE | `/api/probes/:id` | usuário | PUT emite `probe:status` só na transição |
| POST | `/api/probes/:id/revoke` | usuário | `status=revoked`, `revokedAt` |
| POST | `/api/probes/:id/test` | usuário | `{message}` |
| POST | `/api/probes/heartbeat` | **token de probe** | header `X-Probe-Token` ou body `token`; → `{status:"ok", probeId}` |
| GET | `/api/probes/tasks` | **token de probe** | entrega e remove tarefas; descarta > `TASK_TTL_SECONDS=120` → `{tasks:[…]}` |
| POST | `/api/probes/results` | **token de probe** | `{results:[{monitorId, taskId, result}]}` → `{status:"processed", count}` |

Autenticação de probe: `sha256(token)` comparado com `probes.token_hash`, excluindo
`status='revoked'`. **Não é único** — o `DEFAULT_VPN_PROBE_TOKEN` é compartilhado.

### 7.11 Alertas

| Método | Rota | Notas |
| :--- | :--- | :--- |
| GET | `/api/alert-rules` | array ordenado por id |
| POST | `/api/alert-rules` | normaliza `condition` para `{field, operator, value}`; 422 se inválida; emite `alert_rule:created` |
| PUT | `/api/alert-rules/:id` | emite `alert_rule:updated` |
| DELETE | `/api/alert-rules/:id` | 204 + `alert_rule:deleted` |
| GET | `/api/alert-rules/catalog` | `{categories: {chave: rótulo}, templates: [...{applied, ruleId}]}` |
| POST | `/api/alert-rules/catalog` | `{keys:[…]}` → 201 `{created[], skipped[{key, reason}]}`; idempotente |
| GET | `/api/alerts` | modo dual (array ≤100); `SerializedAlertEvent` com `title` derivado e `silencedUntil` ✅ |
| POST | `/api/alerts/:id/acknowledge` | **re-executa o monitor** antes; se recuperou → `{resolved:true}` |
| POST | `/api/alerts/:id/verify` | idem, sem reconhecer |
| POST | `/api/alerts/verify-all` | `{message, totalChecked, resolvedCount}` |
| POST | `/api/alerts/:id/silence` | `{minutes \| durationMinutes}` default 60 |

`SerializedAlertEvent`: `{id, alertRuleId, deviceId, monitorId, scopeKey, status, severity,
message, data, startedAt, resolvedAt, createdAt, updatedAt, title, alertRule{id,name}|null,
device{id,name}|null, monitor{id,name}|null, silencedUntil}`.
`title = data.title || "<ruleName> — <alvo>" || "Alerta do sistema"`.

### 7.12 Eventos

| Método | Rota | Notas |
| :--- | :--- | :--- |
| GET | `/api/events` | paginado (envelope Lucid), `AlertEvent` com `device` e `monitor` |
| GET | `/api/events/stream` | **SSE**. Primeiro evento `{"type":"stream:connected",…}`; `retry: 3000`; keep-alive a cada 25 s; sem `event:` nomeado — o cliente despacha pelo campo `type` ✅ |

### 7.13 VPN

| Método | Rota | Notas |
| :--- | :--- | :--- |
| GET | `/api/vpn/server` | `{configured, server, cidr, serverAddress, peersTotal, peersConnected, bytesRx, bytesTx, persistentKeepalive, profiles[]}`; sincroniza telemetria antes ✅ |
| PUT | `/api/vpn/server` | `{cidr, siteId, networkId, listenPort, publicEndpoint, mtu, dnsServers, allowPeerToPeer, active}` → aplica sem derrubar túneis |
| POST | `/api/vpn/server/preflight` | `{publicEndpoint?, listenPort?}` → `PreflightResult` |
| POST | `/api/vpn/server/detect-endpoint` | `{detected, publicEndpoint, message}` |
| GET | `/api/vpn/peers` | peers + `needsFirewallHint`, `pingOutsideTunnel`, `pingMonitorId`, `device` ✅ |
| GET | `/api/vpn/peers/next-ip` | `{ipAddress, cidr}` |
| POST | `/api/vpn/peers` | `{name, profile, ipAddress?, siteId?, snmpEnabled?, snmpCommunity?, snmpVersion?, description?}` → 201 `{peer, device, artifact}` |
| PATCH | `/api/vpn/peers/:id` | `{name}` → peer + device |
| GET | `/api/vpn/peers/:id/config` | 🔒 rate-limited; artefato **com QR** quando aplicável |
| GET | `/api/vpn/peers/:id/qrcode` | 🔒 rate-limited; 400 se perfil não suporta; 409 se chave já consumida |
| POST | `/api/vpn/peers/:id/rotate` | 🔒 `{message, peer, artifact}` |
| POST | `/api/vpn/peers/:id/firewall-hints` | `{profile, label, content, message}` |
| DELETE | `/api/vpn/peers/:id` | `{message}` — revoga, apaga device (libera IP), reescreve `wg0.conf` |

Rate limit dos endpoints 🔒: janela deslizante **10 req / 60 s** por `user:<id>` ou `ip:<addr>`,
com `Retry-After`. Toda ação sensível gera log de auditoria estruturado.

### 7.14 Templates Zabbix

| Método | Rota | Notas |
| :--- | :--- | :--- |
| GET | `/api/zabbix-templates` | + `deviceCount` e `items[]` resumidos |
| POST | `/api/zabbix-templates` | `{content: string}` (JSON de export) → 201 `{templates:[{id,name,itemCount,skippedItems}]}`; reimport por `uuid` **substitui itens preservando o id** |
| GET | `/api/zabbix-templates/:id` | + items |
| DELETE | `/api/zabbix-templates/:id` | 204; desvincula devices |

### 7.15 Ferramentas de rede

| Método | Rota | Notas |
| :--- | :--- | :--- |
| POST | `/api/port-scan` | **NDJSON streaming** `application/x-ndjson`: `{"type":"result",…}` por porta, depois `{"type":"done"}` ou `{"type":"error","message"}`. Cancela ao fechar a conexão. Validação: `ports` 1–1024 itens, cada uma 1–65535; `timeoutMs` 100–5000 ✅ |
| POST | `/api/dns/benchmark` | `{servers?[≤12], hostnames?[≤10], recordType?, timeoutMs? 200–15000, rounds? 1–5}` → `{hostnames, recordType, measuredAt, ranking[]}` |
| POST | `/api/dns/lookup` | `{hostname, server?, protocol?, dohUrl?, recordType?, timeoutMs?}` → `DnsLookupSample` |
| GET | `/api/dns/performance?hours=24` | agrega `monitor_results` dos monitores DNS → `{windowHours, monitorCount, ranking[]}` |
| GET/POST/PUT/DELETE | `/api/dns/servers[/:id]` | CRUD com validação de endereço por protocolo e 409 em duplicata |

---

## 8. Módulos de domínio — função a função

### 8.1 `shared` — `src/services/shared/`

```rust
// errors.rs
pub enum AppError { Validation(String), NotFound(String), Conflict(String),
                    Unauthorized(String), RateLimited{msg:String, retry_after:u64},
                    BusinessRule(String), Internal(anyhow::Error) }
impl IntoResponse for AppError;                  // §5.5
pub fn error_message(e: &dyn std::error::Error) -> String;

// crypto.rs
pub fn app_key() -> &'static [u8; 32];           // deriva de APP_KEY via SHA-256
pub fn encrypt(plain: &str) -> Result<String>;   // XChaCha20-Poly1305, saída base64(nonce||ct)
pub fn decrypt(cipher: &str) -> Result<String>;
pub fn sha256_hex(input: &str) -> String;        // tokens de probe

// pagination.rs  → §5.4
```

### 8.2 `monitoring` — `src/services/monitoring/`

#### 8.2.1 Contratos (`contracts.rs`)

```rust
pub enum MonitorStatus { Up, Down, Warning, Unknown, Disabled }  // serde lowercase
pub struct CheckMetric { pub name: String, pub value: f64, pub unit: String }
pub struct CheckResult {
    pub success: bool, pub status: MonitorStatus,
    pub started_at: DateTime<Utc>, pub finished_at: DateTime<Utc>,
    pub duration_ms: i64, pub message: Option<String>,
    pub metrics: Vec<CheckMetric>, pub data: serde_json::Value,
}
#[async_trait] pub trait Checker { type Config; async fn execute(&self, c: Self::Config) -> CheckResult; }
```

#### 8.2.2 Checkers

| Arquivo | Assinatura | Métricas emitidas | Regras |
| :--- | :--- | :--- | :--- |
| `checkers/ping.rs` | `execute(PingConfig)` | `latency(ms)`, `packet_loss(%)` | **surge-ping** — [§3.2](#32-decisão-ping-via-surge-ping-obrigatório) |
| `checkers/tcp.rs` | `execute(TcpConfig{host,port,timeout_ms})` | `connect_time(ms)` | `TcpStream::connect` + `timeout`; erro/timeout → `down` |
| `checkers/http.rs` | `execute(HttpConfig{url,method,accepted_status_codes,validate_certificate,timeout_ms,headers})` | `response_time(ms)`, `status_code` | default aceita `[200,201,202,204,301,302]`; fora da lista → `warning`; falha de rede → `down`; `data{statusCode,statusText}` |
| `checkers/dns.rs` | `execute(DnsConfig{domain,domains,record_type,dns_server,protocol,doh_url,timeout_ms,warning_threshold_ms})` | `dns_lookup_time`, `resolution_time` (alias histórico), `dns_lookup_time_min/max`, `dns_success_rate` | consultas **em série**; `status`: 0 sucesso→`down`, parcial→`warning`, acima do limiar→`warning` |
| `checkers/snmp.rs` | `execute(SnmpCheckerConfig{host,version,community,port,timeout_ms,metric,if_index,if_name})` | `if_oper_status`,`if_speed` \| `inBps`,`outBps`,`ifHCInOctets`,`ifHCOutOctets` \| `snmp_uptime` | 3 modos: tráfego de interface, status de interface, uptime. Tabelas `IF_OPER_STATUS_LABELS`/`IF_ADMIN_STATUS_LABELS` (RFC 2863) portadas; `adminStatus==2` → `disabled` |

**`runner.rs`**

```rust
pub fn merge_timeout(cfg: &Value, timeout_ms: Option<u64>) -> Value; // config próprio tem prioridade
pub async fn run_monitor(ctx:&AppContext, kind:&str, cfg:&Value, opts:RunOptions) -> Result<CheckResult>;
// match: ping | http|https | tcp | dns | snmp | _ => Err("Tipo de monitor desconhecido…")
```

**`result_processor.rs`** — `process_result(ctx, monitor_id, result, probe_id)`:

1. `pick_latency_metric` com precedência `["latency","response_time","dns_lookup_time","resolution_time","connect_time"]`.
2. Insere `monitor_results`.
3. Atualiza `monitors.status` e `last_run_at`.
4. `DeviceStatusService::refresh_from_monitors` com `observedStatus` mapeado
   (`up→online`, `down→offline`, `warning→warning`).
5. `AlertManager::evaluate_monitor_result` — **falha aqui não pode derrubar o ciclo** (log e segue).
6. Emite `monitor:result` com `{monitorId, id, name, type, deviceId, deviceName, status,
   previousStatus, statusChanged, latencyMs, durationMs, message, startedAt, finishedAt}`.

**`device_status.rs`** — ponto único de escrita de `devices.status`:

```rust
pub fn aggregate(statuses: &[MonitorStatus], fallback: DeviceStatus) -> DeviceStatus;
// unknown/disabled são inconclusivos; up+down → warning; down → offline;
// warning → warning; senão online; nenhum conclusivo → fallback
pub async fn refresh_from_monitors(..., observed: Option<DeviceStatus>, seen_at: Option<DateTime<Utc>>) -> DeviceStatusTransition;
pub async fn apply(...) -> DeviceStatusTransition;  // emite device:status SÓ na transição
```

**`link_speed.rs`** — `IF_SPEED_SATURATED = 4_294_967_295`; `normalize_speed` devolve `None`
para 0, negativo ou saturado; `format_speed` → `"1 Gbps"`, `"100 Mbps"`, `"Desconhecido"`.

**`interface_monitoring.rs`** — `evaluate_interface_state(device, iface, prev_oper, prev_speed)`:
constrói o dataset, publica `interface:status_change` / `interface:speed_change` /
`interface:speed_downgrade` quando houver transição, e entrega ao `AlertManager` com
`scopeKey = interface:<id>`.

**`presenter.rs`** — o payload de listagem:

```rust
pub const RECENT_RESULTS_LIMIT: u64 = 30;
pub const GAUGE_HISTORY_LIMIT: u64 = 20;
pub const GAUGE_METRIC_NAMES: [&str;2] = ["cpu_usage","memory_usage"];
pub fn gauge_metric_name(m:&monitors::Model) -> Option<String>;
pub async fn monitor_list_with_results(db, filter) -> Result<Vec<(Model, Vec<ResultModel>)>>;
pub async fn fetch_gauge_metrics(db, monitors) -> (HashMap<i32,GaugeReading>, HashMap<i32,Vec<Sample>>);
pub async fn present_monitors(db, monitors) -> Result<Vec<MonitorListDto>>;
```

> ⚠️ **Ponto crítico de performance/correção:** o `recentResults` usa
> `ROW_NUMBER() OVER (PARTITION BY monitor_id ORDER BY started_at DESC)` — **não** um `LIMIT`
> global. Em SeaORM isso exige `Statement::from_sql_and_values` (raw SQL) ou subquery
> lateral. Um `LIMIT` simples faz poucos monitores consumirem toda a cota e os demais ficarem
> sem linha do tempo — regressão silenciosa e visível na tela. Testar explicitamente.
> Resultados devolvidos em **ordem cronológica crescente**.

### 8.3 `network_tools` — `src/services/network_tools/`

- `port_scanner.rs` — [§3.3](#33-decisão-port-scanner-estilo-rustscan-sobre-tokio-obrigatório).
- `udp_probes.rs` — payloads binários idênticos por porta.
- `dns/wire.rs` — encode/decode de mensagem DNS. Usar `hickory-proto` para
  `Message::to_vec()`/`from_vec()`, mantendo:
  - `DNS_RCODE_MESSAGES` em português;
  - detecção de `truncated` para o fallback UDP→TCP;
  - `DnsAnswer { name, type, ttl, value }` com `MX` formatado `"<prio> <exchange>"` e `TXT` concatenado.
- `dns/latency.rs`:
  ```rust
  pub const DEFAULT_DNS_PORT: u16 = 53;
  pub const DEFAULT_DNS_TIMEOUT_MS: u64 = 3000;
  pub const DEFAULT_DNS_SERVERS: [&(…)] = [Cloudflare 1.1.1.1, Google 8.8.8.8, Quad9 9.9.9.9, OpenDNS 208.67.222.222, AdGuard 94.140.14.14];
  pub const DEFAULT_BENCHMARK_HOSTNAMES: [&str;3] = ["google.com","cloudflare.com","globo.com"];
  pub fn parse_server_address(raw:&str) -> Result<(String,u16)>;   // preserva IPv6 entre []
  pub async fn measure_dns_lookup(opts: DnsLookupOptions) -> DnsLookupSample;  // NUNCA falha: erro vira success:false
  pub async fn benchmark_dns_servers(opts) -> Vec<DnsServerRanking>;           // em SÉRIE, de propósito
  pub fn sort_by_latency<T>(items: Vec<T>) -> Vec<T>;                          // null vai para o fim
  ```
  Cronômetro: `std::time::Instant` (monotônico), 3 casas decimais, cobrindo **só** a resolução.
- `dns/registry.rs` — `ensure_defaults()` semeia os 5 resolvedores quando a tabela está vazia;
  `list()`, `benchmark_targets()` (usa `is_default`; se nenhum marcado, usa todos).

### 8.4 `discovery` — `src/services/discovery/`

**`cidr_range.rs`**

```rust
pub const MAX_SCAN_HOSTS: usize = 1024;                 // /22 completo
pub struct CidrRange { network_address, prefix, usable_hosts, truncated }
pub fn parse_cidr_range(cidr:&str) -> Result<CidrRange, InvalidCidrError>;  // aceita host único; /8..=/32
pub fn expand_cidr(cidr:&str, max: usize) -> Result<Vec<Ipv4Addr>>;         // trunca no início do bloco
pub fn is_scannable_cidr(cidr:&str) -> bool;
```
Regras portadas: `/31` e `/32` sem rede/broadcast reservados (RFC 3021); prefixo < /8 rejeitado.

**Scanners:**

| Arquivo | Estratégia Rust |
| :--- | :--- |
| `scanners/icmp.rs` | `surge-ping` compartilhado, `for_each_concurrent(64)`, `packet_count=1`, `timeout=1500ms`; PTR reverso via `hickory-resolver` (best effort); `confidence: 50` |
| `scanners/arp.rs` | **Linux: ler `/proc/net/arp`** (mais confiável que parsear `arp -a`); Windows: `arp -a` via `tokio::process`. Antes, *probe* TCP:80 em lote de 20 (timeout 800 ms) para popular o cache. Filtra broadcast/multicast. `confidence: 80` |
| `scanners/ports.rs` | Reusa `network_tools::port_scanner` com `COMMON_PORTS=[80,443,22,445,8080,8000,3389,161]`; `+20` de confiança quando há porta aberta (cap 100) |
| `scanners/mdns.rs` | `mdns-sd` (0.13) + `tokio::net::UdpSocket` multicast `224.0.0.251:5353`, query PTR `_services._dns-sd._udp.local`, janela 2 s. Decode com `hickory-proto` (substitui o parser manual binário) — extrai nomes `.local` e registros A. `confidence: 70` |
| `scanners/ssdp.rs` | `ssdp-client` (0.4) + UDP multicast `239.255.255.250:1900`, `M-SEARCH ST: ssdp:all`, janela 2 s; extrai `SERVER`, `LOCATION`, `USN`, `ST`; mapa de vendors por substring. `confidence: 60` |
| `scanners/snmp.rs` | `rasn-snmp` (0.18) assíncrono; só hosts com 161/162 abertos; `detect_connection` (v2c/v1 × public/private); extrai vendor de `sysDescr` (22 fabricantes mapeados). `confidence: 95` |

**`device_identifier.rs`** — heurística de tipo (`router`, `switch`, `access_point`, `printer`,
`camera`, `server`, `web_device`, `unknown`) por hostname + vendor + portas. Portar a ordem
exata das regras (é significativa: `router` antes de `server` etc.).

**`oui_lookup.rs`** — mapa OUI embutido (~120 entradas). Implementar com a crate `phf` (`phf_map!`), garantindo busca O(1) em tempo de compilação sem alocação dinâmica.

**`merger.rs`** — funde listas por IP: `macAddress`/`hostname`/`mdnsName`/`vendor` do mais
recente prevalecem; `openPorts` são união; `confidence` é o máximo; `data` é merge; reclassifica
`deviceType`; resolve vendor por OUI quando ausente.

**`service.rs`** — `run_discovery(cidr, network_id, probe_id, existing_run, callbacks, cancel)`:
4 fases (`icmp` → `discovery` (ARP‖mDNS‖SSDP em paralelo) → `ports` → `snmp`), com
`onProgress(phase, current, total)` e `onResult(host)`; ao final **apaga todos os
`discovery_results`** (é cache do último scan) e grava os novos; marca a run
`completed`/`failed`; emite `discovery:started|completed|failed`.

**`queue.rs`** — fila persistente em `discovery_runs`:

```rust
pub const MIN_SCAN_INTERVAL_SECONDS: i64 = 300;
const RUNS_PER_CYCLE: u64 = 1;                 // uma por ciclo, de propósito
const RUNNING_RUN_TIMEOUT_MINUTES: i64 = 15;   // run abandonada
pub async fn enqueue_network_scan(db, network) -> Result<(Run, bool /*alreadyQueued*/)>;
pub async fn schedule_due_networks(db) -> Result<u64>;
pub async fn process_pending_runs(ctx) -> Result<u64>;
```
Regra crítica portada: quando há run `pending` e o CIDR da rede mudou, **atualizar o
`configuration.cidr`** da run — senão o scheduler varre a faixa antiga.

**`session.rs`** — `ScanSessionService` singleton (uma sessão por vez) com `CancellationToken`,
lista de assinantes (`tokio::sync::broadcast`) e log circular de 20 mensagens. Em Rust:
`Arc<RwLock<ScanSessionState>>` no `AppContext`.

### 8.5 `snmp` — `src/services/snmp/`

**`client.rs`** — decidido pelo SPIKE-01.

```rust
pub struct SnmpConfig { host, version: SnmpVersion, community, username, auth_protocol,
                        auth_key, priv_protocol, priv_key, port: u16, timeout_ms: u64 }
pub struct SnmpWalkEntry { pub oid: String, pub value: SnmpValue }
impl SnmpClient {
    pub async fn get(&self, oids:&[&str]) -> HashMap<String, Option<SnmpValue>>;
    pub async fn walk(&self, base_oid:&str) -> Vec<SnmpWalkEntry>;  // GETBULK, para na subárvore
}
```
Regras portadas: `retries = 2` (UDP perde pacote), timeout default 4000 ms, **normalização de
varbind**: buffer de 6 bytes → MAC `aa:bb:…`; 8 bytes → Counter64 numérico; string ASCII
limpa → texto *trim*; binário → hex separado por `:`.

**Coletores** (todos `async fn collect(&SnmpClient) -> …`):

| Coletor | OIDs | Saída |
| :--- | :--- | :--- |
| `system` | `1.3.6.1.2.1.1.{1,2,3,4,5,6}.0` | `sysDescr, sysObjectID, sysUpTime, sysContact, sysName, sysLocation` |
| `interface` | walk `1.3.6.1.2.1.2.2.1` + `1.3.6.1.2.1.31.1.1.1` | `ifIndex, ifName, ifDescr, ifAlias, ifType, ifSpeed, ifAdminStatus, ifOperStatus, macAddress`; `ifHighSpeed`(col 15, Mbps) sobrescreve `ifSpeed` |
| `traffic` | cols 10/14/16/20 + HC 6/10 | `inOctets, outOctets, inErrors, outErrors`; `calculate_rates` com **rollover 2³² e 2⁶⁴** e detecção de reboot |
| `cpu` | walk `1.3.6.1.2.1.25.3.3.1.2` + UCD `2021.11.{9,10,11}.0`, `2021.10.1.3.{1,2,3}` | `usagePercent, userPercent, systemPercent, idlePercent, load1/5/15min, cores[]` |
| `memory` | UCD `2021.4.{5,6,11}.0` | `totalKb, availKb, freeKb, usedKb, usedPercent` |
| `lldp` | `1.0.8802.1.1.2.1.4.1.1` + CDP `1.3.6.1.4.1.9.9.23.1.2.1` | `LldpNeighbor{localPort, remotePort, remoteSysName, remoteMgmtAddress, protocol}` |

**`service.rs`**

```rust
pub async fn scan_device(ctx, device, cfg) -> Result<SnmpScanResult>;
// coleta system+interfaces+cpu+memory+zabbix_preview EM PARALELO (join!);
// snmpResponded = houve QUALQUER resposta; isMonitored por interface = adminStatus atual
pub async fn poll_device(ctx, device, cfg) -> Result<SnmpPollResult>;
// 1 system (status só se respondeu) → 2 interfaces (preserva adminStatus do usuário,
// avalia transições) → 3 tráfego (só interfaces monitoradas; grava ifHCIn/Out + in/outBps)
// → 4 cpu/memória (se houver monitor correspondente OU nenhum monitor) → 5 LLDP → 6 Zabbix
// → emite metric:recorded com o lote coletado
pub async fn test_connection(cfg) -> Result<SnmpTestResult>;
pub async fn detect_connection(host, port, preferred) -> Result<SnmpDetectResult>;
// candidatos: preferred, depois v2c/v1 × public/private, em PARALELO; timeout 2500ms
```

### 8.6 `topology` — `src/services/topology/`

Uso da crate [`petgraph`](https://crates.io/crates/petgraph) (`0.7`) para construção do modelo em memória (`Graph<DeviceNode, LinkEdge>`). Permite detecção de ciclos, verificação de caminhos e agrupamento de componentes conectados antes de serializar o grafo de saída.

```rust
pub async fn get_topology(db, site_id: Option<i32>) -> Result<TopologyGraph>;
pub async fn resolve_discovered_neighbors(ctx, device, neighbors) -> Result<Vec<DeviceLink>>;
pub async fn infer_subnet_links(db) -> Result<Vec<DeviceLink>>;   // infra ↔ end devices, confidence 60
pub async fn create_manual_link(db, s, t, si, ti) -> Result<DeviceLink>;
pub async fn delete_link(db, id) -> Result<bool>;
// link_resolver.rs
pub fn resolve_links(raw: Vec<NetworkLink>) -> Vec<NetworkLink>;  // dedup por par ordenado, maior confiança vence
pub async fn persist_resolved_links_detailed(db, links) -> Result<PersistedLinks{links,created,updated}>;
// confidence: manual 100 | lldp 95 | cdp 90 | snmp 80 | traceroute 60 | _ 50
```
Regra portada: `last_seen_at` avança sempre e **não conta como alteração** — sem isso, toda
coleta LLDP publicaria `topology:updated` com o mapa idêntico.

### 8.7 `alerts` — `src/services/alerts/`

**`fields.rs`** — vocabulário fechado (`ALERT_FIELDS`), 24 chaves + as constantes de transição
(`INTERFACE_STATUS_TRANSITION`, `INTERFACE_SPEED_TRANSITION`, `VPN_STATUS_TRANSITION`).
Os rótulos em português vivem no frontend (`utils/alertPresentation.ts`) e **espelham estas
chaves** — não renomear nada.

**`evaluator.rs`**

```rust
pub struct AlertRuleCondition { field: String, operator: Operator, value: serde_json::Value }
pub enum Operator { Eq, Neq, Gt, Gte, Lt, Lte, Contains }
pub fn evaluate(cond:&AlertRuleCondition, dataset:&Map<String,Value>) -> bool;
// ausente/null → false; gt/gte/lt/lte comparam como f64; contains compara como string
```
⚠️ `eq`/`neq` no JS comparam com `===` **sem coerção** — por isso o template
`snmp_interface_oper_down` usa `value: "2"` (string). Em Rust, comparar `Value == Value`
mantém essa semântica. Não "consertar".

**`repository.rs`** — `find_enabled_for_scope(scope)`: cada dimensão (`site`, `device`,
`monitor`) filtrada independentemente — a regra vale quando **não delimita** aquela dimensão
**ou** aponta exatamente para o alvo.

**`manager.rs`**

```rust
pub async fn evaluate(ctx, context: AlertEvaluationContext) -> Result<()>;
pub async fn evaluate_monitor_result(ctx, monitor, result) -> Result<()>;
// pendingSince: HashMap<(rule_id, scope_key), Instant> em estado compartilhado (OnceLock<Mutex<…>>)
// durationSeconds: só dispara quando a condição se mantém pelo tempo configurado
// dedupe: um evento aberto por (rule, scopeKey) — status in (active, acknowledged, silenced)
// ao disparar: cria AlertEvent + notifica canais + emite alert:triggered
// se NENHUMA regra bateu e context.recovered → RecoveryManager::resolve_scope
```

**`contracts.rs`** — `AlertScopeKey::monitor(id)` → `"monitor:<id>"`,
`::interface(id)` → `"interface:<id>"`, `::vpn_peer(id)` → `"vpn_peer:<id>"`.

**`datasets/`** — três construtores de fatos:

- `monitor_result.rs`: mapa métrica→campo (`latency|response_time → latencyMs`,
  `packet_loss → packetLoss`, `status_code → statusCode`, `connect_time → connectTimeMs`,
  `resolution_time|dns_lookup_time → resolutionTimeMs`, `if_oper_status`, `if_speed`,
  `snmp_uptime`, `inBps`, `outBps`). `latency` tem precedência sobre `response_time`.
  Campos extras de `result.data` entram se não colidirem.
- `interface_state.rs`: transições de status e de velocidade; compara **velocidade formatada**
  para não tratar `999.999.999 vs 1.000.000.000 bps` como renegociação; `interfaceSpeedDropPercent`.
- `vpn_peer.rs`: transições `connected_to_disconnected`, `connected_to_unstable`,
  `reconnected`; **sem estado anterior não há transição** (o primeiro ciclo só estabelece
  a linha de base); degradação em cadeia `unstable → disconnected` conta como queda.

**`catalog/templates.rs`** — os **18 templates** portados literalmente (chave, nome, descrição
em português, categoria, tipo, condição, severidade, `durationSeconds`, `recommended`).
6 categorias: `disponibilidade`, `desempenho`, `servicos`, `interfaces`, `equipamento`, `vpn`.
7 são `recommended: true` (provisionados em instalação nova).

**`catalog/service.rs`** — `describe()`, `apply(keys)` idempotente (por `templateKey` **ou** por
assinatura `field|operator|value|site|device|monitor`), `ensure_defaults()` (**só** age quando
não existe nenhuma regra — uma regra apagada de propósito não ressuscita no restart).

**`recovery.rs`** / **`silence.rs`** — resolução por `scopeKey` (+ `monitorId` quando a chave é
`monitor:<n>`), notificação `✅ [RESOLVIDO]`, evento `alert:resolved`; silenciar grava
`data.silencedUntil` (ISO) e status `silenced`.

### 8.8 `events` — `src/services/events/`

```rust
pub struct SystemEvent { pub r#type: String, pub timestamp: String, pub data: Value }
pub static PROCESS_ORIGIN: Lazy<String>;   // "<pid>-<8 hex aleatórios>"

pub struct EventBus { tx: broadcast::Sender<SystemEvent>, publish_to_outbox: AtomicBool, db: DatabaseConnection }
impl EventBus {
    pub fn subscribe(&self) -> broadcast::Receiver<SystemEvent>;
    pub fn emit(&self, kind:&str, data: Value);   // dispatch local + INSERT em event_outbox (fire-and-forget rastreado)
    pub async fn flush(&self);                    // aguarda escritas pendentes (processos curtos)
    pub fn dispatch(&self, ev: SystemEvent);      // só ouvintes locais
}
// relay.rs
const POLL_INTERVAL_MS: u64 = 1000; const RETENTION_MINUTES: i64 = 10;
const PRUNE_INTERVAL_MS: u64 = 60_000; const BATCH_LIMIT: u64 = 200;
pub async fn start(ctx);  // só roda enquanto houver assinante SSE; cursor começa no MAX(id)
pub fn stop();            // zera o cursor
```
Regra portada: eventos com `origin == PROCESS_ORIGIN` são ignorados no relay (já foram
entregues no `emit`).

**Catálogo completo de eventos** (o frontend despacha por `type` — nomes imutáveis):

`stream:connected`, `monitor:result`, `device:status`, `device:updated`, `alert:triggered`,
`alert:resolved`, `alert:acknowledged`, `alert:silenced`, `alert_rule:created`,
`alert_rule:updated`, `alert_rule:deleted`, `probe:status`, `discovery:started`,
`discovery:completed`, `discovery:failed`, `interface:status_change`,
`interface:speed_change`, `interface:speed_downgrade`, `metric:recorded`,
`vpn:peers_updated`, `vpn:peer_status_change`, `topology:updated`,
`dashboard:layout_updated`.

### 8.9 `notifications` — `src/services/notifications/`

```rust
pub struct NotificationMessage { title, body, severity: Severity, metadata: Value }
#[async_trait] pub trait NotificationChannel { fn name(&self)->&str;
    async fn send(&self, m:&NotificationMessage) -> bool; }
pub struct HttpNotificationChannel;  // base: is_configured() + build_request() + POST JSON
```
4 canais: `email` (SMTP via mailer do Loco), `telegram`, `discord`, `webhook`.
Canal não configurado devolve `false` **sem tentar**. Falha de um canal **nunca** propaga.

### 8.10 `vpn` — `src/services/vpn/`

#### 8.10.1 Chaves e IPAM

```rust
// key_generator.rs — X25519 nativo, sem depender do binário `wg`
pub const WG_KEY_BYTES: usize = 32;
pub fn generate_key_pair() -> WireGuardKeyPair;      // x25519-dalek + base64
pub fn derive_public_key(private_b64:&str) -> Result<String>;
pub fn generate_preshared_key() -> String;           // 32 bytes aleatórios em base64
pub fn is_valid_key(k:&str) -> bool;                 // ^[A-Za-z0-9+/]{43}=$ e 32 bytes

// cidr.rs — parse_cidr, first_usable_address, is_ip_in_cidr, iterate_usable_addresses (iterator)
// ip_allocator.rs
pub const MAX_ATTEMPTS: u32 = 10;
pub fn is_unique_violation(e:&DbErr) -> bool;        // 23505 (pg) / SQLITE_CONSTRAINT_UNIQUE
pub async fn find_next_free(db, network_id, cidr, reserved) -> Result<Ipv4Addr>;
pub async fn allocate<T,F>(db, network_id, cidr, op: F) -> Result<T>;  // retenta em colisão
pub async fn assert_available(db, network_id, cidr, ip) -> Result<()>;
```
A unicidade real é do índice `UNIQUE(network_id, ip_address)`; o alocador apenas **sugere**.

#### 8.10.2 Configuração e telemetria

```rust
// config_builder.rs — função pura
pub fn build(server: &ServerInterfaceInput, peers: &[PeerEntryInput]) -> String;
// [Interface] Address/ListenPort/PrivateKey/MTU + PostUp/PostDown de isolamento
// (allow_peer_to_peer decide ACCEPT vs. ACCEPT ao servidor + DROP entre peers)
// [Peer] # nome / PublicKey / PresharedKey? / AllowedIPs = <ip>/32

// config_writer.rs — escrita ATÔMICA (tmp + rename) no volume compartilhado
pub fn resolve_config_dir() -> PathBuf;   // WG_CONFIG_DIR || (windows ? ./tmp/wireguard : /config)
#[async_trait] pub trait VpnConfigSink { async fn write(&self, f:&str, c:&str)->Result<()>;
                                         async fn read(&self, f:&str)->Result<Option<String>>; }

// peer_status.rs — o servidor NUNCA executa `docker exec`
pub fn parse_wg_dump(dump:&str) -> Vec<WgPeerStatus>;   // linhas de 8 colunas separadas por TAB
pub async fn sync_peers(iface:&str, vpn_server_id:i32) -> Result<u64>;
// dedup de sincronizações em voo (Mutex<HashMap<String, Shared<…>>>);
// aviso único quando <iface>.status está ilegível (falha silenciosa é o pior cenário)
```

#### 8.10.3 Status do túnel (porte literal)

Constantes e a máquina de estados de `VpnPeer` são regra de negócio delicada — porta linha a
linha, **com os comentários**:

```rust
pub const REJECT_AFTER_SECONDS: i64 = 180;
pub const KEEPALIVE_TIMEOUT_SECONDS: i64 = 10;
pub const STATUS_PIPELINE_SLACK_SECONDS: i64 = 45;
pub const KEEPALIVE_MISSES_ALLOWED: i64 = 3;
pub const HANDSHAKE_CONNECTED_SECONDS: i64 = REJECT_AFTER_SECONDS + STATUS_PIPELINE_SLACK_SECONDS;
pub const HANDSHAKE_DISCONNECTED_SECONDS: i64 = 600;
pub fn effective_keepalive_seconds(pk:i64) -> i64 { pk + KEEPALIVE_TIMEOUT_SECONDS }

impl PeerStatusExt for vpn_peers::Model {
    fn last_activity_at(&self) -> Option<DateTime<Utc>>;      // max(lastSeenAt, lastHandshakeAt)
    fn has_keepalive_heartbeat(&self) -> bool;
    fn connected_window_seconds(&self) -> i64;
    fn disconnected_window_seconds(&self) -> i64;             // 2× com keepalive; 600s sem
    fn proof_of_life_window_seconds(&self) -> i64;            // janela CURTA, para diagnóstico
    fn has_fresh_proof_of_life(&self) -> bool;
    fn connection_status(&self) -> VpnPeerConnectionStatus;   // connected|unstable|disconnected|awaiting
}
```

#### 8.10.4 Serviços

```rust
// server_service.rs
pub const DEFAULT_VPN_CIDR: &str = "10.8.0.0/24";
pub const DEFAULT_LISTEN_PORT: u16 = 51820;
pub const DEFAULT_MTU: i32 = 1420;
pub const DEFAULT_INTERFACE: &str = "wg0";
pub async fn find(db) -> Result<Option<Server>>;
pub async fn sync_telemetry(ctx) -> Result<()>;       // ANTES de qualquer leitura de status
pub async fn create_or_update(ctx, payload) -> Result<Server>;
pub async fn apply_configuration(ctx, server) -> Result<String>;
pub async fn get_state(ctx) -> Result<VpnServerState>;

// peer_service.rs
pub async fn list(ctx) -> Result<Vec<PeerListItem>>;   // sincroniza telemetria antes
pub async fn create(ctx, payload) -> Result<(Peer, GeneratedArtifact)>;   // TRANSAÇÃO: device+peer+monitores
pub async fn rename(ctx, id, name) -> Result<Peer>;    // renomeia só monitores com nome gerado
pub async fn rotate_keys(ctx, id) -> Result<(Peer, GeneratedArtifact)>;
pub async fn build_artifact(ctx, id) -> Result<GeneratedArtifact>;   // consome a chave privada
pub async fn firewall_hints(ctx, id) -> Result<(Profile, String)>;
pub async fn revoke(ctx, id) -> Result<()>;

// secret_store.rs — chave privada do cliente NUNCA vai ao banco
pub struct EphemeralSecretStore { ttl: Duration }   // put / consume (lê e descarta) / has
pub const PRIVATE_KEY_UNAVAILABLE: &str = "<CHAVE-PRIVADA-INDISPONIVEL-ROTACIONE-AS-CHAVES>";

// monitor_provisioner.rs
pub const VPN_PROBE_NAME: &str = "vpn-probe";   // env VPN_PROBE_NAME
pub async fn provision(txn, device, opts) -> Result<Vec<Monitor>>;   // ping + snmp opcional, atribuídos ao vpn-probe

// probe_registrar.rs
pub const DEFAULT_VPN_PROBE_TOKEN: &str = "default_vpn_probe_token";   // ⚠️ NUNCA remover o fallback
pub async fn register(db, raw_token: Option<&str>) -> Result<VpnProbeRegistration>;
pub async fn register_with_generated_token(db) -> Result<VpnProbeRegistration>;

// preflight.rs
pub fn is_cgnat_address(ip:&Ipv4Addr) -> bool;   // 100.64.0.0/10 (RFC 6598)
pub fn is_private_address(ip:&Ipv4Addr) -> bool;
pub async fn detect_public_ip() -> Option<Ipv4Addr>;   // api.ipify.org → ifconfig.co
pub async fn run(endpoint: Option<&str>, port: u16) -> PreflightResult;
// status: reachable | port_forward_required | cgnat | unknown ; `verified` é honesto (false quando não há prova externa)

// peer_hints.rs
pub fn compute_peer_hints(peer, ping_monitor: Option<&Monitor>) -> PeerHints;
// régua = has_fresh_proof_of_life (NÃO connection_status) — evita "túnel conectado mas sem ping" falso

// access_control.rs
pub struct SlidingWindowRateLimiter { limit: 10, window: 60s }
pub fn audit(entry: VpnAuditEntry);   // config_download|qrcode_download|key_rotation|peer_revoked|peer_created
```

#### 8.10.5 Perfis (`profiles/`)

5 geradores implementando `VpnProfileGenerator`: **mikrotik**, **openwrt**, **linux**,
**windows**, **mobile**. Cada um produz `GeneratedArtifact { profile, label, delivery,
fileName, language, content, instructions[], supportsQrCode, summary[], variants[] }`.

O conteúdo dos scripts (RouterOS, UCI/OpenWrt, `wg-quick`, PowerShell, `wg0.conf` móvel) e as
`variants` por gerenciador de pacotes (winget, apt, dnf, opkg, apk) devem ser **portados
literalmente** dos arquivos `backend/modules/vpn/profiles/*.ts` — são texto testado em
hardware real. Portar também:

- `asciiSafe()` — consoles RouterOS/OpenWrt são ASCII; acento trunca comando;
- `artifactHeader()` — aviso quando a chave privada já foi consumida;
- `artifactSummary()` — 10+ pares rótulo/valor, **sem** a chave privada;
- `PERSISTENT_KEEPALIVE_SECONDS = 25`, `WG_TUNNEL_NAME = "netmonitor"` (≤15 chars).

QR Code: `qrcode` crate → SVG 320 px, margem 1; **só** quando `supportsQrCode` **e** o
conteúdo não contém `PRIVATE_KEY_UNAVAILABLE`. Renderizado na **mesma resposta** do artefato
(a chave só existe até a primeira leitura).

### 8.11 `probes` — `src/services/probes/`

```rust
pub const TASK_TTL_SECONDS: i64 = 120;
pub const PROBE_OFFLINE_AFTER_SECONDS: i64 = 90;
const DELIVERY_BATCH_LIMIT: u64 = 100;

pub struct ProbeTask { id: String, monitor_id: i32, r#type: String, timeout_ms: u64, payload: Value }
pub async fn dispatch_task(db, probe_id, task) -> Result<()>;   // DELETE where monitor_id + INSERT
pub async fn get_pending_tasks(db, probe_id) -> Result<Vec<ProbeTask>>;   // entrega e remove; descarta vencidas
pub async fn clear_tasks_for_probe(db, probe_id) -> Result<()>;
pub async fn receive_batch_results(ctx, probe_id, payloads) -> Result<()>;
pub fn is_probe_alive(p: Option<&Probe>) -> bool;
pub async fn mark_stale_probes_offline(ctx) -> Result<u64>;     // emite probe:status
```

**Agente (`agent.rs` + task `probe_run`)** — loop de `PROBE_INTERVAL_MS` (5000):
heartbeat → flush do buffer offline → busca tarefas → executa → reporta; falha de rede grava
em `tmp/probe_buffer.json` e reenvia depois. Token: `PROBE_TOKEN` → `VPN_PROBE_TOKEN` →
`DEFAULT_VPN_PROBE_TOKEN`.

### 8.12 `zabbix` — `src/services/zabbix/`

```rust
pub fn parse_zabbix_template_export(input:&Value) -> Result<Vec<ParsedZabbixTemplate>, ZabbixParseError>;
// só itens SNMP_AGENT com snmp_oid e key; value_type default UNSIGNED;
// multiplier vem do preprocessing MULTIPLIER; demais itens vão para skippedItems;
// template sem nenhum item SNMP_AGENT → erro explicativo
const OID_BATCH_SIZE: usize = 6;   // agentes embarcados não respondem a GET grande
pub async fn collect(ctx, device, client) -> Result<u64>;     // grava Metric por item numérico
pub async fn preview(device, client) -> Result<Vec<ZabbixTemplateItemReading>>;
pub const ZABBIX_TEMPLATE_MONITOR_NAME: &str = "Coleta de Template Zabbix";
pub async fn sync_zabbix_template_monitor(db, device) -> Result<()>;   // autocorretivo
```

### 8.13 `maintenance` — `src/services/maintenance/`

```rust
// data_pruner.rs — roda a cada 1h no scheduler
pub async fn prune_all(ctx) -> Result<PruneStats>;
// outbox > 30min | monitor_results > RETENTION_MONITOR_RESULTS_DAYS (14)
// metrics > RETENTION_METRICS_DAYS (30) | discovery_runs > RETENTION_DISCOVERY_DAYS (7)

// resource_cleanup.rs — remoção completa com histórico
pub async fn delete_monitor(db, id) -> Result<()>;        // results, metrics, alert_events, alert_rules, monitor
pub async fn delete_device(db, id) -> Result<()>;         // monitores, interfaces+métricas, alertas, links, device
pub async fn delete_site(db, id) -> Result<()>;           // devices, alert_rules, desvincula probes, site
pub async fn delete_probe(db, id) -> Result<()>;
pub async fn delete_zabbix_template(db, id) -> Result<()>;
```

---

## 9. Processos de background

### 9.1 Topologia de processos (espelha o `docker-compose.yml`)

| Processo | Comando | Papel |
| :--- | :--- | :--- |
| `server` | `backend_rust-cli start` | HTTP + SSE + sessão de scan ao vivo |
| `migration` | `backend_rust-cli db migrate` | Roda uma vez |
| `scheduler` | `backend_rust-cli scheduler --config config/scheduler.yaml` | Dispara `scheduler_run` a cada 5 s — monitores, VPN, discovery, pruner, watchdog |
| `probe` | `backend_rust-cli task probe_run` | Agente da LAN |
| `vpn-probe` | `backend_rust-cli task probe_run` | Agente no namespace do WireGuard |

> **Decisão SPIKE-05 (🟢 [ADR 005](adr/005-scheduler-loco.md), confirmada na Fase 0):**
> `scheduler_run` é uma task de **um ciclo**, invocada pelo scheduler nativo do Loco a cada 5 s
> (`run every 5 seconds`). Isso preserva o ciclo de vida do framework, isola falhas no processo
> `scheduler` e evita um loop infinito ou `tokio::spawn` no processo HTTP.
>
> A objeção óbvia — o scheduler do Loco faz `fork`+`exec` a cada disparo — foi **medida**:
> ~25 ms de boot (binário release, SQLite) num tique de 5 000 ms, ou 0,5%. Duas consequências
> que a §9.2 já cobre e que só existem por causa desta decisão: um ciclo que passe de 5 s é
> **atropelado pelo seguinte** (daí gravar `next_run_at` *antes* de executar e o
> `probe_tasks.monitor_id UNIQUE`), e o `EventBus` em memória não atravessa o processo — daí a
> tabela `event_outbox`.

### 9.2 `tasks/scheduler_run.rs` — um ciclo central

```
mark_stale_probes_offline()      // probe caído precisa aparecer como caído
check_due_monitors()             // ← núcleo
sync_vpn_traffic_if_due()        // status 10s / histórico 30s
run_discovery_queue()            // schedule_due_networks + process_pending_runs
run_data_pruner_if_due()         // a cada 1h
```

Cada bloco em `try/catch` próprio: falha de um **não** interrompe os outros.

**`check_due_monitors`**: `enabled = true AND (next_run_at IS NULL OR next_run_at <= now)`,
`LIMIT 50`. Para cada: grava `next_run_at = now + interval_seconds` **antes** de executar, e
dispara `tokio::spawn` (equivalente ao `executeMonitorAsync`).

**Despacho com probe** — regra portada integralmente:

1. Se `probe_id` e `is_probe_alive(probe)` → `dispatch_task` e retorna.
2. Se o probe está offline → **tenta execução local**; se `result.success`, processa e retorna.
   *(Diretriz do AGENTS.md §6: NÃO remover esse fallback.)*
3. Caso contrário → `report_probe_unavailable`: grava um resultado `unknown` (não `down` — o
   alvo pode estar no ar; quem sumiu foi o agente) com
   `data { probeId, reason: "probe_offline" }`.

**Cadências VPN:** `VPN_STATUS_INTERVAL_SECONDS = 10`, `VPN_TRAFFIC_INTERVAL_SECONDS = 30`.
O ciclo com histórico já sincroniza o status (não gravar duas amostras seguidas).

### 9.3 Workers (`BackgroundAsync`)

| Worker | Disparo | Ação |
| :--- | :--- | :--- |
| `snmp_poll_worker` | após `POST/PUT /api/devices` com `snmpEnabled` | poll inicial não bloqueante; falha só loga |
| `discovery_worker` | `POST /api/discovery/scan` | varredura ao vivo desacoplada da request, alimentando a `ScanSession` |

### 9.4 Tasks CLI (paridade com `node ace`)

| Task | Equivalente Adonis | Saída |
| :--- | :--- | :--- |
| `probe_register` | `probe:register` | cria probe e imprime o token cru (uma vez) |
| `vpn_probe_register` | `vpn:probe-register` | ⚠️ **não remover** — gera/reutiliza o token do vpn-probe |
| `network_scan` | `network:scan` | varre um CIDR pela CLI |
| `snmp_poll` | `snmp:poll` | poll pontual de um device |
| `monitor_test` | `monitor:test` | executa um monitor e imprime o `CheckResult` |

### 9.5 Initializers

| Initializer | Responsabilidade |
| :--- | :--- |
| `event_bus` | cria o `broadcast::Sender` e injeta no `AppContext` (via extensão/`Arc`) |
| `ping_client` | cria o `surge_ping::Client` compartilhado (ICMPv4, DGRAM quando possível) |
| `alert_rules` | `AlertRuleCatalogService::ensure_defaults()` — falha **não** impede o boot |
| `dns_servers` | `DnsServerRegistry::ensure_defaults()` |
| `vpn_probe` | `VpnProbeRegistrar::register()` — falha **não** impede o boot |

---

## 10. Autenticação e autorização

### 10.1 Situação atual

O `AuthController` do Adonis é **stub**: devolve `token: 'sample-token'` e `me → {user:null}`.
O frontend, porém, já está preparado para JWT (`Authorization: Bearer`, `localStorage`,
limpeza em 401, `?token=` no SSE). O Loco traz autenticação real de fábrica.

### 10.2 Decisão

Usar **`loco_rs::auth::JWT`** (`config.auth.jwt`), com adaptação de contrato:

| Endpoint | Resposta obrigatória |
| :--- | :--- |
| `POST /api/auth/login` | `{ "token": "...", "user": { "id": 1, "email": "...", "fullName": "...", "role": "admin" } }` |
| `GET /api/auth/me` | objeto `User` **plano** (o frontend atribui direto a `user.value`) |
| `POST /api/auth/logout` | `{ "message": "Sessão encerrada com sucesso" }` |

> `id` é numérico para o frontend. Expor `users.id` (não o `pid`) nesse campo; o `pid` continua
> sendo o *subject* do JWT internamente.

### 10.3 Proteção das rotas

- Todas as rotas `/api/*` de negócio exigem JWT, **exceto**:
  `POST /api/auth/login`, `POST /api/auth/register`, `/api/auth/forgot`, `/api/auth/reset`,
  `/api/auth/magic-link*`, `/api/auth/verify/*`, e os 3 endpoints de probe
  (`heartbeat`, `tasks`, `results`), que usam o **token de probe**.
- **SSE**: `EventSource` não envia headers. Implementar extractor `JwtFromHeaderOrQuery` que
  aceita `Authorization: Bearer` **ou** `?token=` — o frontend já manda na query.
  Aplicar a `/api/events/stream` e `/api/discovery/scan-stream`.
- **Rollout:** ligar a exigência de JWT só na **Fase 6**. Antes disso as rotas ficam abertas
  (como hoje), para não bloquear a migração dos módulos.

### 10.4 Seed do usuário inicial

`Hooks::seed` + `src/fixtures/users.yaml`: usuário `admin@monitor.local` com senha `admin123`
em `development`/`test` (é o que a `LoginPage` pré-preenche). Em `production`, criar via
`cargo loco task user_create` — **nunca** semear credencial fixa.

---

## 11. Tempo real (SSE) e streaming

### 11.1 `/api/events/stream`

```rust
async fn stream(auth: JwtFromHeaderOrQuery, State(ctx): State<AppContext>) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    // 1. incrementa o contador de assinantes; se for o 1º, EventRelay::start()
    // 2. primeiro evento: {"type":"stream:connected","timestamp":…,"data":{}}
    // 3. BroadcastStream do EventBus → Event::default().data(json)
    // 4. KeepAlive::new().interval(Duration::from_secs(25)).text("keep-alive")
    // 5. no drop do stream: decrementa; se chegar a 0, EventRelay::stop()
}
```
Headers obrigatórios: `Content-Type: text/event-stream`, `Cache-Control: no-cache, no-transform`,
`Connection: keep-alive`, `X-Accel-Buffering: no`, e um `retry: 3000` inicial.
**Sem `event:` nomeado** — o cliente escuta `onmessage` e despacha pelo campo `type` do JSON.

⚠️ `broadcast::Receiver` **descarta** mensagens quando o buffer estoura (`RecvError::Lagged`).
Buffer ≥ 1024 e, ao detectar `Lagged`, emitir um evento sintético de "recarregue" ou apenas
logar `WARN` — nunca encerrar o stream.

### 11.2 `/api/discovery/scan-stream`

Mesmo mecanismo, mas a fonte é o `ScanSessionService`: envia o estado completo ao conectar e a
cada mudança (`broadcast` de notificação + leitura do `RwLock`).

### 11.3 `/api/port-scan` (NDJSON)

```rust
let (tx, rx) = mpsc::channel::<PortScanItem>(256);
let stream = ReceiverStream::new(rx).map(|item| Ok::<_, Infallible>(Bytes::from(format!("{}\n", json!({"type":"result", ...})))));
Response::builder()
    .header(CONTENT_TYPE, "application/x-ndjson")
    .header(CACHE_CONTROL, "no-cache")
    .body(Body::from_stream(stream))
```
Ao final: `{"type":"done"}`; em erro: `{"type":"error","message":"…"}`. Cancelamento pelo
`CancellationToken` amarrado ao fim do stream.

---

## 12. Ajustes necessários no frontend

A regra é **mexer o mínimo**, e sempre registrado aqui. Pelo princípio
[§1.3.0](#13-princípios-inegociáveis) ([ADR 006](adr/006-prioridade-do-padrao-rust.md)), quando
reproduzir o comportamento atual custaria contorcer o backend Rust, quem se adapta é o
frontend — e a linha entra nesta tabela.

| # | Arquivo | Mudança | Motivo | Fase |
| :-: | :--- | :--- | :--- | :---: |
| **F1** | `src/stores/auth.ts` | Em `fetchMe()`, aceitar tanto `User` plano quanto `{user: User}`. Em `logout()`, chamar `POST /auth/logout` antes de limpar. | O `/auth/me` passa a devolver usuário real; hoje devolve `{user:null}`. Defensivo, não quebra o backend antigo. | 6 |
| **F2** | `src/stores/auth.ts` | Guardar também `user` em `localStorage` e reidratar no boot. | Com JWT real, um F5 hoje perde o usuário até o `fetchMe`. | 6 |
| **F3** | `src/router/index.ts` | Guard de rota redirecionando para `/login` em `!isAuthenticated`. | Hoje a autenticação é decorativa; ao ligar o JWT vira necessária. **Verificar se já existe** antes de alterar. | 6 |
| **F4** | `src/services/apiService.ts` | Em 401, além de limpar o token, redirecionar para `/login`. | Sessão expirada hoje deixa a tela em estado morto. | 6 |
| **F5** | `src/composables/useInfiniteList.ts` | Nenhuma — mantido o envelope Lucid. | Registrado aqui para deixar explícito que o **backend** é que se adapta ([§5.4](#54-paginação)). | — |
| **F6** | `src/stores/portScan.ts` | Nenhuma esperada. **Validar** que o parser NDJSON tolera chegada muito mais rápida (RustScan é ordens de grandeza mais veloz). | Risco de *race* no acúmulo reativo. | 4 |
| **F7** | `src/types/` *(novo, opcional)* | Consumir `frontend/src/bindings/*.ts` gerados por `ts-rs`. | Ganho de tipagem ponta a ponta. Opcional, não bloqueia o corte. | 8 |
| **F8** 🟢 | `src/bindings/*.ts` *(novo)* | Destino dos bindings `ts-rs` passa a ser `frontend/src/bindings/`. Gerados: `LucidMeta`, `LucidPage`, `ApiError`, `ApiFieldError`, `ServiceInfo`. | O scaffold exportava para `backend-rust/frontend/`, diretório que ninguém consome. Agora o struct Rust é a fonte da verdade do tipo TS. | 0 |
| **F9** 🟢 | `src/composables/useInfiniteList.ts` | `PaginatedResponse.meta` passa a usar o `LucidMeta` gerado, em vez do tipo redigitado à mão. Comportamento em runtime **inalterado**. | Se o backend mudar um campo do `meta`, o `vue-tsc` acusa — em vez de a lista infinita parar sozinha em produção. Substitui a nota "nenhuma mudança" do F5. | 0 |

> **F5 revisado:** a linha original dizia "nenhuma mudança — o backend é que se adapta". O
> envelope Lucid continua sendo do backend (o princípio 1 se aplica: reproduzi-lo custou um
> struct); o que mudou foi só a **origem do tipo** no TypeScript. Ver F9.

**Nada além disso.** Qualquer outra divergência encontrada durante a migração é **bug do
backend Rust** — a não ser que se enquadre no princípio [§1.3.0](#13-princípios-inegociáveis),
e nesse caso vira uma linha nova nesta tabela, com motivo escrito. Mudança silenciosa no
frontend, nunca.

---

## 13. Configuração, ambiente e Docker

### 13.1 `config/*.yaml`

```yaml
server:
  port: 3333            # 🟢 Fase 0 — aplicado nos três ambientes
  binding: 0.0.0.0
  middlewares:
    cors:
      enable: true
      allow_origins: ["http://localhost:5173", "http://localhost:8081"]
      allow_headers: ["*"]
      allow_methods: ["*"]
    compression: { enable: true }
database:
  uri: {{ get_env(name="DATABASE_URL", default="postgres://netmonitor:secret@postgres:5432/netmonitor") }}
  auto_migrate: false   # em produção quem migra é o serviço `migration`
  max_connections: 20
workers:
  mode: BackgroundAsync
auth:
  jwt:
    secret: {{ get_env(name="JWT_SECRET") }}
    expiration: 604800
```

`test.yaml`: SQLite em arquivo, `dangerously_truncate: true`, mailer `stub`.

### 13.2 Variáveis de ambiente

Manter compatibilidade com o `.env` atual e acrescentar as novas:

| Variável | Uso | Default |
| :--- | :--- | :--- |
| `DATABASE_URL` | conexão (Loco) | derivada de `DB_*` no compose |
| `APP_KEY` | chave de cifra em repouso (VPN) | — (obrigatória) |
| `JWT_SECRET` | assinatura do token | — (obrigatória) |
| `PORT`, `HOST`, `LOG_LEVEL` | servidor | 3333 / 0.0.0.0 / info |
| `WG_CONFIG_DIR` | volume compartilhado WireGuard | `/config` (win: `./tmp/wireguard`) |
| `VPN_PROBE_NAME` | nome do probe dedicado | `vpn-probe` |
| `VPN_PROBE_TOKEN` | token do probe dedicado | `default_vpn_probe_token` ⚠️ |
| `PROBE_TOKEN`, `PROBE_SERVER_URL`, `PROBE_INTERVAL_MS` | agente | — / `http://server:3333` / 5000 |
| `RETENTION_MONITOR_RESULTS_DAYS` | pruner | 14 |
| `RETENTION_METRICS_DAYS` | pruner | 30 |
| `RETENTION_DISCOVERY_DAYS` | pruner | 7 |
| `SCAN_MAX_HOSTS` *(novo)* | teto do discovery | 1024 |
| `PORTSCAN_BATCH_SIZE` *(novo)* | override do batch RustScan | auto (ulimit) |
| `PING_SOCKET_KIND` *(novo)* | `dgram` \| `raw` | `dgram` |

### 13.3 Dockerfile (multi-stage)

```dockerfile
FROM rust:1-slim AS builder
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
COPY . .
RUN cargo build --release --bin backend_rust-cli

FROM debian:stable-slim
RUN apt-get update && apt-get install -y ca-certificates iproute2 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/backend_rust-cli /usr/local/bin/
COPY --from=builder /app/config /app/config
WORKDIR /app
CMD ["backend_rust-cli", "start"]
```

Ajustes no `docker-compose.yml` (Fase 9):

- `build: ./backend` → `build: ./backend-rust` nos serviços `server`, `migration`, `scheduler`,
  `probe`, `vpn-probe`;
- comandos: `db migrate`, `start`, `task scheduler_run`, `task probe_run`;
- **`sysctls: net.ipv4.ping_group_range: "0 2147483647"`** nos serviços que pingam
  (`server`, `scheduler`, `probe`) — habilita ICMP DGRAM sem `CAP_NET_RAW`.
  Se o SPIKE-03 concluir que não basta, adicionar `cap_add: [NET_RAW]`;
- **manter** os volumes `wg-config` em `server`, `scheduler` **e** `vpn-probe`
  (sem isso a telemetria de túnel congela em silêncio);
- **manter** `network_mode: "service:wireguard"` no `vpn-probe`;
- `ulimits: nofile: {soft: 8192, hard: 65536}` — o batch do port scanner deriva daí.

---

## 14. Estratégia de testes

O scaffold já traz `loco-rs[testing]`, `rstest`, `insta`, `serial_test`.

| Camada | Ferramenta | O que cobre |
| :--- | :--- | :--- |
| Unidade | `#[test]` / `#[rstest]` | Funções puras: `parse_cidr_range`, `expand_cidr`, `format_speed`/`normalize_speed`, `calculate_rates` (rollover 32/64 bits e reboot), `parse_wg_dump`, `build` do `config_builder`, `evaluate` do `RuleEvaluator`, `aggregate` do `DeviceStatusService`, `parse_zabbix_template_export`, `compute_peer_hints`, `connection_status` do peer, `parse_server_address` |
| Snapshot | `insta` | Artefatos VPN dos 5 perfis (o `.conf`/script inteiro), `wg0.conf` do servidor, `SerializedAlertEvent`, payload de `present_monitors` |
| Integração (rede) | `#[tokio::test]` | Checkers contra `127.0.0.1`: TCP em porta aberta/fechada, HTTP com servidor `axum` efêmero, DNS contra resolvedor local, ping em loopback. **Timeout 5 s** por teste (AGENTS.md §4) |
| Requisição | `loco_rs::testing::request` | Todos os endpoints: status, forma do JSON, paginação, modo dual array/paginado, erros 4xx |
| Modelo | `loco_rs::testing` + `truncate` | `Hooks::truncate` deve zerar **as 23 tabelas**, não só `users` (o scaffold só zera `users`) |

**Regras:**

- Somente `127.0.0.1`/`localhost` em testes de rede — nada de alvo externo.
- SNMP: mock do `SnmpClient` por trait (o cliente atual já tem `setMockGet`/`setMockWalk`;
  em Rust, `trait SnmpTransport` + implementação `MockTransport`).
- `#[serial]` nos testes que tocam o `ScanSessionService` singleton ou o `EphemeralSecretStore`.
- Alvo de cobertura: **100% das funções puras listadas acima** + 1 teste de requisição por
  endpoint. Isso é critério de aceite de fase, não meta aspiracional.

---

## 15. Fases de execução

> Marcar `[x]` + badge 🟢 conforme conclusão, seguindo a convenção de `docs/roadmap.md`
> (AGENTS.md §5).

### Fase 0 — Fundação e spikes (🟢 **Concluída** — 2026-08-10)

- [x] Executar **SPIKE-01..05** e publicar os ADRs em `docs/adr/`
      — [001](adr/001-snmp-client.md), [002](adr/002-rustscan-embedding.md),
      [003](adr/003-icmp-dgram.md), [004](adr/004-dns-wire.md),
      [005](adr/005-scheduler-loco.md). Protótipos em `backend-rust/examples/spikes/`,
      executáveis por `cargo run --example spike_{icmp_dgram,snmp_v2c,dns_wire}`.
- [x] Fechar o `Cargo.toml` e travar o `Cargo.lock`
      — bloco da [§3.1](#31-cargotoml-alvo) aplicado, com duas correções registradas
      ali: `socket2` 0.6 (ADR 003) e `anyhow` (exigido pelo `AppError` da [§8.1](#81-shared--srcservicesshared)).
- [x] Corrigir `server.port` para 3333 nos 3 ambientes; configurar CORS
      — `config/{development,test,production}.yaml`.
- [x] `src/services/shared/{errors,crypto,pagination}.rs` + testes
      — 21 testes unitários; `AppError` com `IntoResponse` próprio (ver nota abaixo).
- [x] `LucidPage`/`LucidMeta`/`MaybePaged` validados contra o `useInfiniteList` real
      — testes replicam o laço `currentPage >= lastPage` do composable, em memória e
      contra banco (`tests/models/pagination.rs`).
- [x] Convenção `#[serde(rename_all="camelCase")]` aplicada e verificada por um teste que
      falha se um DTO esquecer o atributo
      — `tests/conventions/camel_case.rs`. Já pegou dois DTOs do scaffold (`LoginResponse`,
      `CurrentResponse`), corrigidos.
- [x] `Hooks::truncate` cobrindo as 23 tabelas
      — lista única em `src/models/tables.rs` (`CREATION_ORDER`), limpeza na ordem inversa,
      pulando tabelas ainda não migradas. Nenhuma tabela nova precisa ser lembrada no `app.rs`.

**Extras entregues nesta fase** (não estavam na lista, viraram pré-requisito):

- `GET /` e o prefixo `/api` da [§5.6](#56-prefixo-e-cors) — `src/controllers/root.rs`;
  o prefixo saiu do controller de auth e passou para o `AppRoutes` (padrão Loco).
- `Dockerfile` (multi-estágio, usuário não-root, sem `CAP_NET_RAW`) e
  `docker-compose.icmp-spike.yml` — exigidos por SPIKE-03.
- Bindings `ts-rs` passam a ser gerados em `frontend/src/bindings/` (antes iam para um
  diretório órfão dentro de `backend-rust/`).

> **Desvio consciente da [§1.3.4](#13-princípios-inegociáveis).** Os handlers devolvem
> `Result<_, AppError>`, não `Result<_, loco_rs::Error>`. O `IntoResponse` do Loco serializa
> `{"error","description"}`; o frontend lê `message` (`apiService.handleResponse`), então todo
> erro viraria o texto genérico `"Erro HTTP 422: ..."` nos snackbars. `AppError` converte nos
> dois sentidos (`From<loco_rs::Error>` e `From<AppError> for loco_rs::Error`), então tasks e
> workers continuam com a assinatura do framework. É o que a [§5.5](#55-erros) e a
> [§8.1](#81-shared--srcservicesshared) já mandavam; a §1.3.4 continua valendo no que importa —
> nada de `unwrap()` fora de `OnceLock`/constantes.

**Validação:** `cargo build --all-targets` limpo, `cargo test` com 63 testes verdes,
`cargo fmt --check` e `cargo clippy -- -D warnings` limpos, `npm run typecheck` do frontend
limpo.

### Fase 1 — Esquema e entidades (🟢 **Concluída** — 2026-08-10)

- [x] 23 migrations SeaORM com **todos** os índices, uniques e FKs de [§6](#6-modelo-de-dados--migrations)
      — 23 migrations registradas em `migration/src/lib.rs`: a `users` do scaffold, mais 22
      novas (21 tabelas de negócio + a coluna `active` da §6 #01). A #23 `auth_tokens` não é
      criada; ver abaixo.
      FKs declaradas à mão em `migration/src/shared.rs`: o helper `refs` do Loco deriva a ação
      da nulabilidade (anulável → `SET NULL`), e seis FKs do esquema são **anuláveis com
      `CASCADE`** (`probes.site_id`, `networks.site_id`, `devices.site_id`, `monitors.device_id`
      e as três de `alert_rules`).
- [x] `cargo loco db entities` gerando `src/models/_entities/`
      — geradas **a partir do PostgreSQL**, não do SQLite. Ver a nota de portabilidade abaixo.
- [x] `src/models/*.rs` com computados, cifra de campo (`private_key_encrypted`,
      `preshared_key_encrypted`) e queries nomeadas
      — `Network::{scannable,usable_hosts,scan_truncated}`, `Monitor::{target,port,is_enabled}`,
      `AlertRule::is_enabled`, a máquina de estados de `VpnPeer::connection_status`
      ([§8.10.3](#8103-status-do-túnel-porte-literal), porte literal com os comentários),
      `VpnServer::{private_key,set_private_key}`, `VpnPeer::{preshared_key,set_preshared_key}`,
      `Probe::find_by_token`, `Monitor::find_due`, `Device::find_by_ip_or_name`,
      `DnsServer::find_by_address`, `SystemSetting::{get,set}`.
- [x] Migração testada em **SQLite e PostgreSQL**
- [x] Script de verificação comparando o esquema gerado com o do Adonis (colunas, tipos, índices)
      — `cargo run --example schema_parity`. Ele **parseia as migrations `.ts` do AdonisJS** e
      compara com o catálogo do banco vivo; não é uma transcrição minha conferida contra ela
      mesma. Resultado: **21 tabelas, 0 divergências não declaradas.**

**Divergências deliberadas** (todas declaradas no `schema_parity`, que falha se aparecer
qualquer outra):

| Onde | Divergência | Por quê |
| :--- | :--- | :--- |
| todas | PK e FK em `bigint` (i64), não `integer` | Padrão do Loco 0.17+. `metrics` e `monitor_results` são séries temporais — o teto de 2³¹ linhas é alcançável, e migrar o tipo depois exige parada. |
| todas | `updated_at` `NOT NULL DEFAULT now()` | O `timestamps_tz` do Loco. Uma linha sempre tem instante de última escrita; `null` ali só produz ramo morto em quem lê. |
| `monitor_results.latency_ms` | `double precision`, não `real` | A [§5.3](#53-tipos-numéricos) define `latencyMs` como `f64`. Ler um `real` como f64 injetaria ruído de precisão num número exibido com casas decimais. |
| `devices.is_monitored`, `devices.snmp_enabled` | `NOT NULL DEFAULT false` | O Adonis esqueceu o `.notNullable()` e o knex deixou anulável. `NULL` não tem significado distinto de `false`; a coluna vira `bool` em vez de `Option<bool>`. |
| `users` | fica a do scaffold Loco + `active` | [§6 #01](#6-modelo-de-dados--migrations). |
| `auth_access_tokens` / `auth_tokens` | não criada | A [§10.2](#102-decisão) optou por `loco_rs::auth::JWT`, que não guarda token no banco. O nome segue em `CREATION_ORDER` caso a Fase 6 volte atrás. |

> **Nota de portabilidade — gere as entidades do PostgreSQL.** O SQLite é dinamicamente tipado e
> reporta todo inteiro como `INTEGER`; o `db entities` rodado contra ele produz `i64` para
> colunas que em Postgres são `INT4`, e aí o `sqlx` recusa a leitura em produção. O caminho
> inverso é seguro (o SQLite aceita ler `i32`), então as entidades saem do Postgres e os testes
> continuam em SQLite. Isso vale para toda regeneração futura.

**Aceite:** `db migrate` + `db entities` idempotentes; diff de esquema vazio. ✅

### Fase 2 — CRUDs e contrato base (🔴)

- [ ] `sites`, `networks`, `devices`, `monitors`, `probes`, `dns_servers`,
      `zabbix_templates`, `dashboard`
- [ ] `ResourceCleanupService` completo (5 funções)
- [ ] Serialização enriquecida: `scannable`/`usableHosts`, `target`/`port`/`isEnabled`
- [ ] Testes de requisição para todos os endpoints desta fase

**Aceite:** o frontend navega em Sites, Redes, Dispositivos, Monitores, Probes, Templates e
Configurações sem erro de console, apontando para o backend Rust.

### Fase 3 — Motor de monitoramento (🔴)

- [ ] `contracts.rs`, `runner.rs`, `result_processor.rs`, `device_status.rs`, `presenter.rs`
- [ ] **`PingChecker` com `surge-ping`** ([§3.2](#32-decisão-ping-via-surge-ping-obrigatório))
- [ ] `TcpChecker`, `HttpChecker`
- [ ] `tasks/scheduler_run.rs` com o laço completo e o **fallback local de probe offline**
- [ ] `presenter` com window function validada (30 resultados **por monitor**)
- [ ] Endpoints `run`/`enable`/`disable`/`results`

**Aceite:** monitores executam, gravam histórico, atualizam status de device; a tela
`/monitors` mostra linha do tempo e sparkline corretos.

### Fase 4 — Ferramentas de rede (🔴)

- [ ] **Port scanner RustScan/tokio** ([§3.3](#33-decisão-port-scanner-estilo-rustscan-sobre-tokio-obrigatório)) + `UdpProbeRegistry`
- [ ] `POST /api/port-scan` com NDJSON e cancelamento
- [ ] DNS: `wire`, `latency`, `registry`, `DnsChecker`
- [ ] `POST /api/dns/{benchmark,lookup}`, `GET /api/dns/performance`, CRUD `/api/dns/servers`
- [ ] Validação F6 no frontend

**Aceite:** `PortScanDialog` e `DnsLatencyCard` funcionam; varredura de 1024 portas < 3 s.

### Fase 5 — SNMP, discovery e topologia (🔴)

- [ ] **Cliente SNMP v1/v2c/v3**: `SnmpClient` assíncrono sobre `tokio::net::UdpSocket` usando `rasn` + `rasn-snmp` (0.18) (SPIKE-01/ADR 001, sem `libsnmp` C e sem `spawn_blocking`) + 6 coletores (`system`, `interface`, `traffic`, `cpu`, `memory`, `lldp`) + `SnmpService` (`scan`/`poll`/`test`/`detect`)
- [ ] **`SnmpChecker`**: 3 modos (status de interface, tráfego e uptime) com mapeamento RFC 2863 e tratamento de rollover 32/64-bits
- [ ] **Scanners de Discovery**: 6 coletores assíncronos desacoplados otimizados para Linux/Docker:
  - ICMP sweep via `surge-ping` (0.8) sobre `SOCK_DGRAM` (`ping_group_range="0 2147483647"`, sem `CAP_NET_RAW` / sem root)
  - ARP via leitura direta de `/proc/net/arp` no Linux após pré-probe TCP porta 80/443
  - Port sweep com concorrência adaptativa (estratégia RustScan sobre `tokio`)
  - mDNS via `mdns-sd` (0.13) + `hickory-proto` (0.24) em `224.0.0.251:5353`
  - SSDP via `ssdp-client` (0.4) em `239.255.255.250:1900`
  - SNMP sweep via `rasn-snmp` (0.18) na porta 161
- [ ] **Reconciliação e Identificação**: `merger`, `oui_lookup` (O(1) sem alocação com `phf`), `device_identifier` (heurística de tipos)
- [ ] **Serviço de Varredura**: `DiscoveryService`, `DiscoveryQueue`, `ScanSessionService` + SSE de progresso ao vivo
- [ ] **Serviço de Topologia**: `TopologyService` construído sobre `petgraph` (`0.7`), com leitura de MIBs LLDP (`1.0.8802...`) / CDP (`1.3.6.1.4.1.9...`) via `rasn-snmp`, inferência de sub-redes, links manuais e deduplicação de grafo
- [ ] **Templates Zabbix**: Parser JSON/XML, collector de métricas customizadas e `zabbix_template_monitor_sync`

**Aceite:** `/discovery` varre uma faixa /24 com progresso ao vivo em Linux no Docker sem privilégio root; `/topology` desenha o grafo com `petgraph`; poll SNMP assíncrono grava métricas e detecta vizinhos LLDP/CDP.

### Fase 6 — Alertas, eventos e autenticação (🔴)

- [ ] `EventBus` + `EventRelay` + `/api/events/stream` (SSE)
- [ ] Motor de alertas completo (manager, evaluator, repository, datasets, recovery, silence)
- [ ] Catálogo com os **18 templates** e `ensure_defaults`
- [ ] 4 canais de notificação
- [ ] JWT ligado em todas as rotas + extractor `JwtFromHeaderOrQuery` para SSE
- [ ] Patches **F1–F4** no frontend
- [ ] Seed do usuário inicial

**Aceite:** alerta dispara, notifica, aparece na tela em tempo real e normaliza sozinho;
login/logout reais funcionando; F5 mantém a sessão.

### Fase 7 — Probes (🔴)

- [ ] Autenticação por token, `heartbeat`, `tasks`, `results`
- [ ] `ProbeTaskDispatcher` com TTL de 120 s e substituição por monitor
- [ ] `ProbeWatchdog` (90 s) com evento de transição
- [ ] `tasks/probe_run.rs` (agente) + buffer offline
- [ ] `tasks/probe_register.rs`

**Aceite:** container `probe` registra, recebe tarefas, executa e reporta; derrubar o probe o
marca `offline` e os monitores caem no fallback local.

### Fase 8 — VPN WireGuard (🔴)

- [ ] `key_generator` (X25519 nativo), `cidr`, `ip_allocator`
- [ ] `config_builder`, `config_writer` (escrita atômica), `peer_status` (`wg show dump`)
- [ ] `server_service`, `peer_service`, `secret_store`, `monitor_provisioner`
- [ ] 5 perfis com scripts **portados literalmente** + `variants` + QR Code
- [ ] `traffic_recorder`, `state_watcher`, `peer_hints`, `preflight`
- [ ] `access_control` (rate limit + auditoria)
- [ ] `probe_registrar` + `tasks/vpn_probe_register.rs` ⚠️
- [ ] `ts-rs` exportando bindings (F7, opcional)

**Aceite:** wizard cria peer, entrega script/QR uma única vez, `wg0.conf` é aplicado por
`syncconf` sem derrubar túneis, telemetria atualiza a tela, rotação invalida o anterior.

### Fase 9 — Corte e descomissionamento (🔴)

- [ ] Suíte de paridade: script que bate **todos** os endpoints nos dois backends e compara os
      JSONs normalizados (ordem de chave e timestamps ignorados)
- [ ] Plano de migração de dados (o esquema é idêntico → `pg_dump`/`pg_restore` direto;
      validar `jsonb` e `bigint`)
- [ ] `docker-compose.yml` apontando para `backend-rust`
- [ ] Rodar os dois em paralelo por 1 ciclo de validação (shadow), comparando alertas gerados
- [ ] Atualizar `AGENTS.md` (comandos de validação viram `cargo fmt`/`clippy`/`test`)
- [ ] Atualizar `docs/roadmap.md` e `docs/arquitetura.md`
- [ ] Arquivar `backend/` (tag git + remoção)

**Aceite:** diff de paridade vazio; frontend inalterado além de F1–F4; `backend/` removido.

---

## 16. Matriz de paridade funcional

Checklist de conferência item a item. Cada linha só é marcada com evidência (teste ou
verificação manual registrada).

| # | Comportamento | Onde vive hoje | Verificação |
| :-: | :--- | :--- | :--- |
| 1 | Ping mede RTT e perda, `warning` em perda parcial | `ping_checker.ts` | teste vs. `ping` do SO |
| 2 | Timeout do monitor sobrepõe o default do checker, salvo `timeoutMs` explícito | `monitor_runner.ts:mergeTimeout` | unitário |
| 3 | `latencyMs` sai do primeiro nome da lista de precedência | `result_processor.ts` | unitário |
| 4 | `device.status` só é escrito pelo `DeviceStatusService` e só emite evento na transição | `device_status_service.ts` | teste de integração |
| 5 | `recentResults` traz até 30 **por monitor** | `monitor_presenter.ts` | teste com 3 monitores × 50 resultados |
| 6 | Scheduler grava `next_run_at` antes de executar | `scheduler_run.ts` | teste |
| 7 | Probe offline → fallback local → resultado `unknown` (não `down`) | `scheduler_run.ts` | teste |
| 8 | Tarefa de probe vencida (>120 s) é descartada, não executada | `probe_task_dispatcher.ts` | unitário |
| 9 | Uma tarefa pendente por monitor (substituição, não acúmulo) | migration + dispatcher | teste |
| 10 | Faixa > 1024 hosts é truncada e a UI é avisada (`truncated`) | `cidr_range.ts` | unitário |
| 11 | `/31` e `/32` sem rede/broadcast reservados | `cidr_range.ts` | unitário |
| 12 | HTTP não varre: `POST /networks/:id/scan` só enfileira | `networks_controller.ts` | teste de requisição |
| 13 | Run `running` há > 15 min é considerada abandonada | `discovery_queue.ts` | unitário |
| 14 | CIDR corrigido atualiza a run `pending` já enfileirada | `discovery_queue.ts` | teste |
| 15 | `discovery_results` é cache: limpo a cada scan concluído | `discovery_service.ts` | teste |
| 16 | Rollover de contador SNMP 2³²/2⁶⁴ e detecção de reboot | `traffic_collector.ts` | unitário com 3 cenários |
| 17 | `ifHighSpeed` (Mbps) prevalece sobre `ifSpeed` saturado | `interface_collector.ts` / `snmp_checker.ts` | unitário |
| 18 | `ifSpeed == 4294967295` → velocidade desconhecida (sem falso downgrade) | `link_speed.ts` | unitário |
| 19 | Poll SNMP só marca `online` se algum OID respondeu | `snmp_service.ts` | teste |
| 20 | `adminStatus` definido pelo usuário é preservado no poll | `snmp_service.ts` | teste |
| 21 | Itens Zabbix lidos em lote de 6 OIDs | `zabbix_template_collector.ts` | unitário |
| 22 | Reimport de template por `uuid` preserva o `id` (e os devices vinculados) | `zabbix_templates_controller.ts` | teste |
| 23 | Monitor "Coleta de Template Zabbix" é autocorretivo | `zabbix_template_monitor_sync.ts` | teste |
| 24 | `durationSeconds` só dispara após condição sustentada | `alert_manager.ts` | unitário com relógio controlado |
| 25 | Um alerta aberto por (regra, `scopeKey`) | `alert_manager.ts` | teste |
| 26 | Catálogo é idempotente por `templateKey` **ou** assinatura | `alert_rule_catalog_service.ts` | teste |
| 27 | `ensure_defaults` não ressuscita regra apagada | idem | teste |
| 28 | `eq` compara sem coerção (template usa `"2"` string) | `rule_evaluator.ts` | unitário |
| 29 | Recuperação fecha alertas por `scopeKey` + `monitorId` | `recovery_manager.ts` | teste |
| 30 | Eventos de background chegam ao SSE via `event_outbox` | `event_relay.ts` | teste com 2 processos |
| 31 | Relay ignora eventos da própria origem | `event_relay.ts` | unitário |
| 32 | Relay só consulta o banco com assinante SSE conectado | `events_controller.ts` | teste |
| 33 | Chave privada do peer entregue **uma única vez** | `secret_store.ts` | teste |
| 34 | QR só quando o perfil suporta e a chave ainda existe (senão 409) | `vpn_peers_controller.ts` | teste |
| 35 | `wg0.conf` escrito atomicamente (tmp + rename) | `config_writer.ts` | teste |
| 36 | Isolamento entre peers via PostUp/PostDown (não `syncconf`) | `config_builder.ts` | snapshot |
| 37 | Status do túnel usa keepalive quando existe; handshake senão | `vpn_peer.ts` | unitário, 6 cenários |
| 38 | Transição de VPN exige estado anterior (1º ciclo é linha de base) | `vpn_peer_dataset.ts` | unitário |
| 39 | `needsFirewallHint` usa `hasFreshProofOfLife`, não `connected` | `peer_hints.ts` | unitário |
| 40 | `pingOutsideTunnel` quando o monitor não roda no `vpn-probe` | `peer_hints.ts` | unitário |
| 41 | IP liberado ao revogar o peer (device removido) | `vpn_peer_service.ts` | teste |
| 42 | Colisão de IP concorrente é retentada (até 10×) | `ip_allocator.ts` | teste concorrente |
| 43 | `DEFAULT_VPN_PROBE_TOKEN` como fallback ⚠️ | `vpn_probe_registrar.ts` | teste |
| 44 | Rate limit 10/60 s + `Retry-After` nos endpoints sensíveis | `access_control.ts` | teste |
| 45 | Pruner respeita as 3 variáveis de retenção | `data_pruner_service.ts` | teste |
| 46 | Modo dual array/paginado nos 4 endpoints | vários controllers | teste de requisição |
| 47 | `createdAt` em `dd/MM/yyyy HH:mm:ss` em metrics/events de device | `devices_controller.ts` | teste |
| 48 | `topology` cria aresta virtual para `parentId` com id negativo | `topology_service.ts` | teste |
| 49 | `last_seen_at` do link não conta como alteração | `link_resolver.ts` | unitário |
| 50 | UDP: `open` / `closed` (ECONNREFUSED) / `open\|filtered` | `port_scanner_service.ts` | teste |

### Índice de comandos CLI (paridade)

| Adonis | Rust |
| :--- | :--- |
| `node ace scheduler:run` | `backend_rust-cli task scheduler_run` |
| `node ace probe:run` | `backend_rust-cli task probe_run` |
| `node ace probe:register` | `backend_rust-cli task probe_register` |
| `node ace vpn:probe-register` ⚠️ | `backend_rust-cli task vpn_probe_register` |
| `node ace network:scan` | `backend_rust-cli task network_scan` |
| `node ace snmp:poll` | `backend_rust-cli task snmp_poll` |
| `node ace monitor:test` | `backend_rust-cli task monitor_test` |
| `node ace migration:run` | `backend_rust-cli db migrate` |

---

## 17. Não-objetivos e desvios aceitos

Desvios conscientes em relação ao backend AdonisJS. Qualquer outro desvio é bug.

| # | Desvio | Justificativa | Impacto no frontend |
| :-: | :--- | :--- | :--- |
| D1 | Ping por socket ICMP nativo em vez de `execFile('ping')` | Requisito explícito; elimina dependência de idioma/variante do SO e o custo de `fork()` | Nenhum (mesmo payload) |
| D2 | Port scan com concorrência adaptativa em vez de 16 fixo | Requisito explícito (RustScan) | Resultados chegam muito mais rápido — validar F6 |
| D3 | Parser mDNS/DNS via `hickory-proto` em vez de parser manual | Menos superfície de bug binário | Nenhum |
| D4 | ARP lido de `/proc/net/arp` no Linux | Mais confiável que parsear `arp -a` | Nenhum |
| D5 | Autenticação real (JWT) substituindo o stub | O stub era placeholder; o frontend já esperava JWT | F1–F4 |
| D6 | Cifra de campo com XChaCha20-Poly1305 em vez do encryption do Adonis | Não há equivalente; formato interno, nunca exposto | Nenhum. **Requer re-cifrar** os campos VPN na migração de dados (Fase 9) |
| D7 | `ts-rs` gerando bindings TypeScript | Ganho novo | Opcional (F7) |
| D8 | Sem "Worker & Queue System" formal | Já era 🔴 no roadmap atual — o scheduler executa inline. Não regride nem avança | Nenhum |
| D9 | `RouteResolver` (traceroute) continua stub | Já é stub hoje (`resolveRoute` devolve `[]`) | Nenhum |
| D10 | `ProbeAuthenticator`/`ProbeConnection` não são portados | São stubs mortos (`return true`) — a autenticação real está no controller | Nenhum |

**Riscos monitorados:**

| Risco | Mitigação |
| :--- | :--- |
| SNMP v3 (USM) sem crate madura | SPIKE-01 decide cedo; plano B: `spawn_blocking` com crate síncrona |
| ICMP exigir privilégio no ambiente de destino | SPIKE-03 + `sysctl ping_group_range`; plano B: `CAP_NET_RAW` |
| Window function do `presenter` divergir entre SQLite e Postgres | Teste rodando nos dois dialetos |
| Re-cifra dos segredos VPN na migração de dados | Script dedicado na Fase 9 + validação de que todo peer gera artefato após a migração |
| `broadcast::Receiver` perdendo eventos sob rajada | Buffer ≥ 1024 + tratamento de `Lagged` |

---

## 18. Critérios de aceite (Definition of Done)

Uma fase só é marcada 🟢 quando **todos** os itens abaixo são verdadeiros:

1. `cargo fmt --check` limpo.
2. `cargo clippy --all-targets -- -D warnings` limpo.
3. `cargo test` verde, incluindo os testes de requisição da fase.
4. `cargo build --release` compila e a imagem Docker sobe.
5. Toda função pública tem doc-comment em português explicando **por que** existe quando a
   razão não é óbvia (regra do projeto — os comentários do Adonis são a memória do sistema).
6. Os itens correspondentes da [matriz de paridade](#16-matriz-de-paridade-funcional) estão
   marcados **com evidência**.
7. Nenhuma mudança no frontend além das listadas em [§12](#12-ajustes-necessários-no-frontend).
8. `docs/roadmap_backend_rust.md` (este arquivo) atualizado com `[x]` e badge 🟢.

**Aceite final do projeto:**

- O frontend roda contra `backend-rust` com as mudanças F1–F4 e nada mais.
- As 50 linhas da matriz de paridade estão verdes.
- `docker compose up` sobe `migration`, `server`, `scheduler`, `probe`, `wireguard`,
  `vpn-probe`, `frontend`, `postgres` — todos saudáveis.
- Um ciclo completo funciona ponta a ponta: **descobrir** uma faixa → **cadastrar** um
  dispositivo → **monitorar** (ping + SNMP) → **alertar** na queda → **notificar** → **resolver**
  na volta — com tudo aparecendo em tempo real via SSE.
- `backend/` arquivado.
