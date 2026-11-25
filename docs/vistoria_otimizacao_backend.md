# Vistoria Técnica — Auditoria, Otimização e Correções do Backend Rust

> **Objetivo:** Relatório de engenharia consolidado para vistoria manual (*code review*) de todas as auditorias, otimizações de performance, correções de bugs/fluxo, endurecimentos de segurança e limpezas SOLID/DRY aplicados no workspace Rust (`backend/`).
>
> **Escopo:** Todos os 12 crates do workspace (`backend`, `agent-protocol`, `core`, `nucleo`, `servers`, `servers-registry`, `network`, `network-wifi`, `network-link-tests`, `vpn`, `assistant`, `mcp`) e o unit file do serviço do agente (`backend/assets/agent/netmonitor-agent.service`). Nenhum código de frontend foi alterado.

---

## 1. Resumo Executivo & Validação (`AGENTS.md §1`)

### Estatísticas do Diff (`git diff HEAD --shortstat`)
- **Arquivos modificados:** `372` arquivos (após a segunda rodada, §6) (0 arquivos não rastreados no código Rust)
- **Linhas alteradas:** `+21.810` inserções / `-6.976` deleções
- **Compatibilidade de protocolo:** `PROTOCOL_VERSION = 7` mantido intacto (*wire-compatible*)
- **Compatibilidade de banco:** Todas as alterações validadas em **SQLite** e desenhadas com paridade estrita para **PostgreSQL 18** (`begin_write`, `OnConflict`, `NullOrdering`, `information_schema` vs `sqlite_master`, dedup de lotes upsert)

### Portão de Aceite Obrigatório (`AGENTS.md §1`)

Todos os 4 comandos obrigatórios rodados a partir de `backend/` passaram com código de saída `0` (executados com `-j 2` no Windows para evitar exaustão de *pagefile* `os error 1455` ao linkar os 13 binários de teste simultaneamente):

| Etapa | Comando (`backend/`) | Resultado | Tempo | Observações |
| :--- | :--- | :---: | ---: | :--- |
| **Formatação** | `cargo fmt --all && cargo fmt --all --check` | **PASS** | 8,5s | 100% aderente ao `.rustfmt.toml` |
| **Lint / Clippy** | `cargo clippy --workspace --all-targets -j 2 -- -D warnings` | **PASS** | 66,1s | **0 erros, 0 warnings** nos 12 crates |
| **Suíte de Testes** | `cargo test --workspace -j 2 --no-fail-fast` | **PASS** | — | **3.497 passed, 0 failed, 3 ignored** (segunda rodada) |
| **Build Release** | `cargo build --release -j 2` | **PASS** | 478s | Todos os 12 crates e binários (`backend-cli`, `netmonitor-agent`) compilados |

### Detalhamento das 13 Suítes de Teste (`cargo test --workspace -j 2`)

| Crate / Alvo | Passaram | Falharam | Ignorados | Tempo |
| :--- | ---: | ---: | ---: | ---: |
| `backend` (`src/lib.rs`) | 34 | 0 | 0 | 0,02s |
| `backend` (`tests/mod.rs` — convenções, models, requests, tasks) | 555 | 0 | 2 | 581,49s |
| `agent-protocol` (`--lib`) | 99 | 0 | 0 | 0,46s |
| `netmonitor-assistant` (`--lib`) | 327 | 0 | 0 | 2,28s |
| `netmonitor-core` (`--lib`) | 431 | 0 | 0 | 2,29s |
| `netmonitor-mcp` (`--lib`) | 10 | 0 | 0 | 0,11s |
| `netmonitor-network` (`--lib`) | 538 | 0 | 1 | 6,03s |
| `netmonitor-network-link-tests` (`--lib`) | 115 | 0 | 0 | 9,13s |
| `netmonitor-network-wifi` (`--lib`) | 69 | 0 | 0 | 0,09s |
| `netmonitor-nucleo` (`--lib`) | 777 | 0 | 0 | 2,80s |
| `netmonitor-servers` (`--lib`) | 325 | 0 | 0 | 2,03s |
| `netmonitor-servers-registry` (`--lib`) | 56 | 0 | 0 | 0,08s |
| `netmonitor-vpn` (`--lib`) | 161 | 0 | 0 | 1,03s |
| **Total** | **3.497** | **0** | **3** | — |

> **Nota sobre testes de convenção (`tests/mod.rs`):** As 20 guardas arquiteturais (`conventions::architecture`, `conventions::transacoes`, `conventions::camel_case`, `conventions::device_adapters`, `conventions::tela_unificada`) passaram integralmente — confirmando **0** ocorrências de `.begin().await` deferido em produção e 100% de respeito às fronteiras de crates (ADR 017).

---

## 2. Checklist de Pontos Críticos para Vistoria Humana

Itens de maior impacto comportamental, de segurança ou de contrato operacional recomendados para revisão manual prioritária:

- [ ] **Protocolo e túneis do agente remoto (`agent-protocol`, `core/src/agents`, `nucleo/src/services/{agent_runtime,agents}`):**
  - Conferir ordenação estrita de `Body::Welcome` antes de `hub.register(...)` em `nucleo/src/services/agents/connection.rs` e exceção controlada de `Body::Hello` em `Envelope::decode` (`agent-protocol/src/protocol.rs`).
  - Conferir `pump_raw` + `CreditWindow` (`AtomicU64` + `Notify`) em `agent-protocol/src/tunnel.rs`, propagação de `UnexpectedEof`/`RemoteError` em `TunnelBridgeStream` (`core/src/agents/bridge.rs`) e detecção de WebSocket *half-open* (Ping 25s / idle 60s no servidor, 55s no agente) + `spawn_priority_writer` compartilhado.
  - Conferir `backend/assets/agent/netmonitor-agent.service` (`ReadWritePaths=-/srv/netmonitor/stacks`) e permissão `0770` no script de instalação (`install.rs` + snapshot `insta`).
- [ ] **Endurecimentos *fail-closed* de Autenticação, RBAC, IA, MCP e Container Registry:**
  - `nucleo/src/services/users.rs`: escrita de configuração, provedores e modelos da IA restrita a `Admin`; leitura (GET/HEAD/OPTIONS) liberada a todo papel, porque a aba IA do Núcleo abre para todos; `test-connection` só `Admin`; `Viewer` liberado para `/api/registry/tokens*`.
  - `assistant/src/controllers/ai.rs` + `approvals.rs` + `settings.rs` + `drivers/mod.rs`: `require_authenticated_actor` (401) nos 4 endpoints HTTP, `require_admin` nas rotas administrativas, isolamento de aprovações por `(user_id, call_id)`, auto-aceite de dispositivo escopado por `user_id` (`auto_accept::active_for_user`), recusa em revelar `opencode_api_key`/`decider.api_key` se `base_url` mudar.
  - `servers-registry/src/auth.rs` & `api/mod.rs`: exigência de `user.active` no login `/v2`, recusa *fail-closed* (`!origin.is_internal()` -> `Code::Denied`) de tokens pessoais via borda pública, validação de `X-Forwarded-*` em `realm()` apenas quando interno.
  - `mcp/src/scopes.rs` & `protocol.rs`: recusa *fail-closed* de subação desconhecida em `deploy_action`; limite `MAX_BATCH_SIZE = 20` com cobrança de *rate-limit* por item.
  - `nucleo/src/controllers/auth.rs` & `push.rs`: verificação `user.active` em `/auth/current`, *rate-limit* em verify/magic-link/resend, escopo de usuário em assinaturas WebPush.
- [ ] **Migrations editadas diretamente (pré-produção — `AGENTS.md §3`):**
  - `core/src/migrations/m20220101_000001_users.rs` (`users.pid` como `UuidUniq`), `m20260810_000011_monitor_results.rs` (`monitor_results_probe_id_index`), `m20260810_000012_metrics.rs` (`metrics_interface_recorded_index` + `metrics_monitor_id_index`), `m20260810_000016_alert_events.rs` (`alert_events_created_at_index` + `alert_events_status_started_index` + índice único parcial `alert_events_open_rule_scope_unique`, um alerta aberto por regra e alvo). **Lembrete:** recriar banco local de dev para aplicar os novos índices.
- [ ] **Correção de unidade 8× em métricas de rede do host (`core/src/monitoring/health/parsers.rs`):**
  - `IN_BPS`/`OUT_BPS` agora multiplicam `bytes/s * 8.0` corretamente (antes reportavam bytes/s rotulados como bps). Verificar se algum gráfico ou regra de alerta esperava o valor antigo distorcido.
- [ ] **Política de vulnerabilidade *fail-closed* nos Deploys (`servers/src/deploys/images.rs` & `executor.rs`):**
  - `VulnerabilityPolicy::blocks` agora bloqueia quando `ScanStatus` é `Failed` ou `Missing` sob `BlockCritical`/`BlockHigh`, e `verify_images` executa antes do backup pré-deploy.
  - Quando a análise falha, a mensagem cita `REGISTRY_OSV_URL`; a varredura não baixa o OSV de novo por 15 min depois de uma falha (`osv::recently_failed`).
  - `deploys/oidc.rs` exige `jti` não vazio e só o libera depois de erro 5xx da central (retry do curl); `deploys/webhook.rs` aceita só `push`/`workflow_dispatch` (lista de permissão; `event_name` ausente é recusado).
- [ ] **Quarentena e `reveals: true` em Plugins de Dispositivo (`network/src/plugins/`):**
  - `service.rs` bloqueia `set_enabled` e `duplicate` em plugins em `QUARANTINE`; `runtime.rs` bloqueia `reveals: true` em `use_plugin`, restringe `login: true` a HTTP `POST` (inclusive POST montado fora de mapa inline) e mascara a **senha de acesso** em `stdout`/`stderr`/headers/body antes de retornar ao Rhai (AGENTS §7). Segredos da configuração (chave Wi-Fi) chegam ao script e são mascarados na saída, no erro e na transcrição; só `plugins::reveal` (`execute_revealing`) devolve a saída sem máscara.
- [ ] **Retenção do Syslog no PostgreSQL (`nucleo/src/services/syslog/retention.rs`):**
  - Corrigido loop destrutivo no Postgres que podia remover ~640k linhas/hora por conta de orçamento baseado em tamanho total de tabela (incluindo tuplas mortas); agora o orçamento é em linhas: teto ÷ largura amostrada (`pg_column_size` das 1000 mais novas, ×2 para o GIN, +148 B), com linhas vivas de `n_live_tup` (ou `reltuples` depois de `pg_stat_reset`). Converge e independe de `n_dead_tup`; deleção em lotes.
- [ ] **Escrita atômica de `wg0.conf` e alocação VPN (`vpn/src/config_writer.rs`, `cidr.rs`, `ip_allocator.rs`):**
  - Arquivo temporário único `.{name}.{pid}.{rand:016x}.tmp` criado com `create_new(true)` + `0600` (Unix) + `sync_all` + `rename`; `/31` e `/32` continuam sem faixa de peers na VPN (`vpn/src/cidr.rs`) enquanto `core/src/shared/cidr.rs` trata `/31` (RFC 3021) e `/0` IPv6 sem pânico.
- [ ] **Proteção de containers e integridade de restore de banco (`servers/src/docker/`, `databases/`):**
  - `ClearLogs`, `update` e `stacks/deploy` validam `is_protected` (com cache do próprio ID de container via `/proc/self/mountinfo`).
  - Restore MySQL `Replace` baixa a cópia uma vez para um temporário `0600` e valida gzip + SHA-256 **antes** de executar `DROP DATABASE`; escopos `database-backup` e `database-restore` separados, com exclusão mútua por conexão (`acquire_exclusive`), também no agendador, e mensagem conforme a operação em curso.

---

## 3. Detalhamento Completo por Área / Crate

### 3.1 Comunicação com o Agente Remoto
**Escopo:** `backend/crates/agent-protocol/src/**`, `backend/crates/core/src/agents/**`, `backend/crates/nucleo/src/services/{agent_runtime,agents}/**`, `backend/crates/nucleo/src/controllers/agents.rs`, `backend/src/bin/agent.rs`, `backend/assets/agent/netmonitor-agent.service`

- **Performance:**
  - `agent-protocol/src/protocol.rs`: `Command::label` e `DockerCall::label` convertidos em `const fn -> &'static str` (elimina `serde_json::to_value` + cópia de segredos em toda chamada); capacidade exata em `TunnelFrame::encode` (`Ack` 21B, `Eof` 17B); `TunnelFrame::peek_id`, `encode_raw`, `encode_compressed_with`, `decode_with`.
  - `agent-protocol/src/tunnel.rs`: reutilização de `zstd::bulk::Compressor<'static>` em `DataEncoder` (elimina alocação de `ZSTD_CCtx` por quadro e cópia dupla); `pump_raw` com `zstd::bulk::Decompressor` por túnel fora do loop leitor do WebSocket.
  - `agent-protocol/src/telemetry.rs`: `RollupAccumulator` migrado de `Vec`s de amostras para acumuladores O(1).
  - `core/src/agents/session.rs` & `nucleo/src/services/agents/connection.rs`: roteamento *zero-copy* de quadros binários via `handle_tunnel_frame_owned(Vec<u8>)`; bitmask `u16` em `allows()`; eventos com escrita em banco (`MetricsRollup`, `MonitorResult`, `DiscoveryResult`) descarregados para worker limitado por sessão (`mpsc::channel(128)`), mantendo snapshots SSE sem bloqueio.
  - `nucleo/src/services/agents/background.rs`: `touch_loop` unificado em lote a cada 30s (`1 UPDATE ... WHERE id IN (...)` no lugar de 1 query por agente); `pump_loop` com `SELECT DISTINCT probe_id FROM probe_tasks WHERE probe_id IN (...)`; cache de 15s para `areas::is_enabled(Area::Servers)`; semáforo `MAX_INFLIGHT_MONITORS = 16` + `receive_batch_results`.
  - `nucleo/src/services/agent_runtime/{client.rs,session.rs,outbox.rs,sampler.rs,identity.rs}`: `OnceLock<reqwest::Client>` compartilhado, `TCP_NODELAY`, `outbox` com persistência coalescida em `spawn_blocking` (`peek_batch`/`ack_batch`), `HostMetricsReader::sample()` em `spawn_blocking`, `tokio::fs` em `identity::hello`, eliminação de `result.clone()` recuperando payload de `SendError`.
- **Bugs & Fluxo:**
  - `agent-protocol/src/tunnel.rs`: divisão de `pump` em leitor/escritor concorrentes (`tokio::io::split`) + `CreditWindow` (`AtomicU64` + `Notify`, `checked_add` limitado a `WINDOW`) com `cancel.cancelled()` em todo `.await` (corrige *deadlock* AB-BA e escritas não-interrompíveis); retorno de `UnexpectedEof` em fechamento prematuro.
  - `agent-protocol/src/telemetry.rs`: preservação de *baselines* por container entre viradas de minuto (corrige subcontagem de ~16,7% de I/O) e guarda contra recuo de relógio NTP (`bucket < current`).
  - `core/src/agents/{session.rs,bridge.rs,hub.rs}`: `pending`/`tunnels` protegidos por `Mutex<Option<HashMap<...>>>` (elimina TOCTOU pós-`close()` que podia travar por até 12h); `PendingGuard` RAII; `close_token()` exposto; `OpenTunnel` anexa `done` antes do `select!`, envia `Cancel` explícito em `OPEN_TIMEOUT` e aguarda `done` (5s) antes do drop; `TunnelBridgeStream` propaga `UnexpectedEof` e `RemoteError` pós-abertura em vez de EOF truncado silencioso; `LocalBridge` usa `Semaphore(MAX_CONNECTIONS)` e `CancellationToken` compartilhado.
  - `nucleo/src/services/agents/connection.rs` & `controllers/agents.rs` & `agent_runtime/{client.rs,session.rs}`: enfileiramento de `Body::Welcome` + `spawn_priority_writer` **antes** de `hub.register(...)` (corrige corrida no handshake de túneis); `Mutex` fraco por `probe_id` serializando transições `online`/`offline` e rechecando `hub.get(probe_id).is_none()` (corrige corrida de *takeover*); detecção de WS *half-open* (Ping 25s / idle 60s servidor, 55s agente) com `sink.close()` em 2s; separação `MAX_CONCURRENT_REQUESTS = 16` vs `MAX_CONCURRENT_STREAMS = 8`; `exec_cancel` separado para timeout local continuar enviando `ErrorCode::Timeout`; *shutdown* gracioso (`SIGINT`/`SIGTERM`) em `backend/src/bin/agent.rs`.
  - `nucleo/src/services/agents/local.rs`: `ensure` + `adopt_central_resources` em `begin_write`; arquivo de token gravado via `spawn_blocking` **somente após** `commit`.
  - `nucleo/src/services/agents/install.rs` & `backend/assets/agent/netmonitor-agent.service`: `/srv/netmonitor/stacks` com `0770` (grupo `docker`) e `ReadWritePaths=-/srv/netmonitor/stacks` no systemd (snapshot `insta` atualizado).
- **Segurança:**
  - `agent-protocol/src/protocol.rs`: rejeição de `KIND_DATA` vazio e `KIND_ACK(0)`; verificação estrita `envelope.v == PROTOCOL_VERSION` para todos os envelopes pós-`Hello` (mantendo `Hello` decodificável para `read_hello` devolver `HandshakeError::Protocol`).
  - `agent-protocol/src/docker/{images.rs,stacks.rs}` & `device_io.rs`: validação endurecida de `target`/`reference` (rejeita `-`, `..`, `:` malformados), detecção de args/segredos duplicados, `validate_env_secret` (`\0` / teto 64 KiB), `MAX_COMPOSE_FILES = 16`, rejeição de `"."`, redação de headers sensíveis no `Debug` de `HttpRequest`.
  - `core/src/agents/{targets.rs,database_targets.rs,enrollment.rs}`: `DeviceNetworks::parse` *fail-closed* (`deny_all` em CIDR inválido), validação estrita de `[IPv6]:porta`, validação de formato do código antes de SHA-256/mutex em `redeem()`.
  - `nucleo/src/services/agents/{service.rs,install_address.rs}` & `controllers/agents.rs`: autenticação VPN *fail-closed* (`ensure_device?`, rejeita dispositivo ausente quando `enforce_tunnel_ip`), filtro estrito `is_agent(&probe)`, validação de `AGENT_IMAGE`/`AGENT_SERVER_URL`/`X-Forwarded-*` via `TrustedProxies` + `is_safe`, proteção do agente local contra alteração de `device_id`/`enforce_tunnel_ip`, `ImageExport`/`ImageImport` adicionados à guarda de túneis do executor.
- **SOLID / DRY / Limpeza:**
  - `spawn_priority_writer` compartilhado em `nucleo/src/services/agents/mod.rs`; `IpAddr::to_canonical()` da stdlib substituindo duplicata; `controllers/agents.rs` delegando listagem/política/desconexão para `service.rs`; helpers de `registration.rs` genéricos em `C: ConnectionTrait`.
- **Novos testes adicionados:**
  - `agent-protocol` (96 testes): *deadlock* bidirecional AB-BA, cancelamento durante escrita bloqueada, `UnexpectedEof` em fechamento prematuro, delta cross-minute + recuo NTP em `RollupAccumulator`, versão em `Envelope::decode`, rejeição de `DATA` vazio/`ACK(0)`.
  - `core/agents` (33 testes): `UnexpectedEof` & `RemoteError` no meio de `download`, limpeza de `PendingGuard` em abort, `cancel` não bloqueante em fila cheia, `close_token` + `register_tunnel` pós-close, `DeviceNetworks` *fail-closed*, `From<RemoteError>`, `TunnelTarget` IPv6.
  - `agent_runtime` & `agents`: `follow_logs_lotado_nao_trava_pedidos_rapidos`, `timeout_local_ainda_responde_erro_timeout`, testes de `outbox` coalescido e validação de `AGENT_IMAGE`.

---

### 3.2 Core & Infraestrutura (`backend/crates/core`)
**Escopo:** `backend/crates/core/src/**` (exceto `agents/**`) — 43 arquivos modificados

- **Performance:**
  - `events/{mod.rs,bus.rs,relay.rs}`: `EncodedEvent { event, json: Arc<str> }`, `subscribe_encoded()` e `current_encoded_snapshots()` serializando cada evento uma única vez para todos os clientes SSE; `relay.rs` com leitura O(1) do `latest_id` por PK, filtro `Origin != self` e `Id <= latest_id`, e avanço único de cursor.
  - `migrations/`: índices adicionados diretamente (pré-produção, `AGENTS.md §3`): `users.pid` (`UuidUniq`), `metrics_interface_recorded_index`, `metrics_monitor_id_index`, `monitor_results_probe_id_index`, `alert_events_created_at_index`, `alert_events_status_started_index`.
  - `storage/{cloud.rs,local.rs}`: eliminação de cópia dupla de 8 MiB em `cloud::put_file` (buffer movido entre `4 KiB..=5 MiB`), *streaming* com `limit + 1` no `lister_with`, filtragem por `cursor` antes de `entry.metadata().await` no `local.rs`.
  - `telemetry/store.rs`: `container_metrics_1m::insert_many` em lotes de 50 (segurança do limite de 999 parâmetros do SQLite), decodificação JSON de `disks` e formatação RFC3339 adiadas para os buckets de saída.
  - `shared/{rate_limit.rs,client_addr.rs,pagination.rs}`: `VecDeque<Instant>` (O(1) `pop_front`) + fast-path `get_mut` + remoção do mais antigo no teto em `rate_limit`; `OnceLock` em `TrustedProxies::private_networks()` e `rsplit(',')` zero-alloc; `pagination` pula `fetch_page` em página vazia/fora do intervalo.
  - `monitoring/health/parsers.rs` & `network_tools/{public_ip.rs,nat.rs,ping_client.rs}`: zero `format!` por linha em `parse_meminfo`; `HealthCoordinator::collect_async` via `spawn_blocking`; cache `OnceLock` para `reqwest::Client` em `public_ip`, `can_inspect_l2()` em `nat` e fallback `PingClientCell` em `ping_client`; `models/users.rs` descarregando Argon2 para `spawn_blocking`.
- **Bugs & Fluxo:**
  - `monitoring/health/parsers.rs`: **correção do bug de fator 8×** (`entrada_bytes_s * 8.0`, `saida_bytes_s * 8.0` para `IN_BPS`/`OUT_BPS`), validação finita/não-negativa em `parse_loadavg_1m` e aritmética `saturating_*` nos parsers `/proc`.
  - `storage/{cloud.rs,local.rs,sftp.rs,service.rs}`: remoção de prefixo duplicado em `cloud::to_key()`; `local::put_file` atômico com `.tmp` irmão + `rename`; `sftp::connect()` envolvido em `CONNECT_TIMEOUT` e checagem específica de `StatusCode::NoSuchFile`; `SqlErr::UniqueConstraintViolation -> AppError::conflict`.
  - `telemetry/store.rs` & `models/{monitors.rs,system_settings.rs,users.rs}` & `maintenance/resource_cleanup.rs`: `begin_write` em `save`, `rename_host`, `device_status::apply`, `delete_device`, `clear_device_history`, `delete_site`, `delete_probe`; buckets alinhados por tempo (`group_by_time_bucket`) em vez de contagem de linhas; `NullOrdering::First` explícito em `monitors::find_due` (paridade SQLite/Postgres); `OnConflict` atômico em `system_settings::Model::set`; `try_as_ref()` no `Validator` de `users` (corrige pânico em `ActiveModel` parcial); `monitor_results_hourly` incluído no `resource_cleanup` + lotes de 400.
  - `shared/{cidr.rs,rate_limit.rs}` & `network_tools/{icmp_probe.rs,neighbor_cache.rs}`: correção de *overflow* de shift em `Ipv6Cidr::new` com `/0`; canonicalização de IPv4-mapped IPv6; correção de pânico quando `limit == 0` em `rate_limit`; proteção contra *underflow* em `calculate_probe_result` + `select!` de cancelamento; `timeout(5s)` + `kill_on_drop(true)` em `neighbor_cache` e remoção do descarte incorreto de `.ends_with(".255")` no ARP do Windows.
- **Segurança:**
  - `storage/{sftp.rs,local.rs,cloud.rs}`: bloqueio de *path traversal* (`..`, `\0`, `:`, `/` ou `\` inicial) no SFTP; `canonicalize` no ancestral mais profundo existente em `local::resolve_async` (impede escape via *symlink*); guarda `assert_deletable` + `RootDeletion` contra deleção da raiz.
  - `shared/{secret_file.rs,secret_store.rs,runtime.rs,crypto.rs}`: arquivo temporário único com `0600` na criação em `secret_file`; limite `MAX_ENTRIES = 4_096` + expiração O(1) em `secret_store`; `is_production_environment()` unificado (`LOCO_ENV` e `ENVIRONMENT`) ligado ao `crypto.rs` (e remoção de `.expect()` em `cipher()`).
- **SOLID / DRY / Limpeza:**
  - `InFlightSet<K>` genérico com recuperação de *poison* em `execution_guard`; `path_matches` zero-alloc em `areas/policy.rs`; `ai::series::round2` delegando para `shared::units::round_two`; remoção de `.expect()` em `devices/systems.rs`; `cadence::forget_device` + limpeza de entradas obsoletas; `AuditActor::from_headers` e `find_by_pid` genéricos em `C: ConnectionTrait`.
- **Novos testes adicionados (`netmonitor-core`: 417 passed):**
  - *Path traversal* e deleção de raiz SFTP, duplo prefixo na nuvem, 8× bps no health, *bucketing* temporal de telemetria, `rate_limit` com `limit == 0`, CIDR `/0` + `/31`/`/32` + IPv4-mapped, ICMP *underflow*/zero-sent, ARP `.255` no Windows.

---

### 3.3 Núcleo (`backend/crates/nucleo`)
**Escopo:** `backend/crates/nucleo/src/**` (exceto `agent_runtime` e `agents`) — 87 arquivos modificados

- **Performance:**
  - **SSE & Alertas:** `controllers/events.rs` migrado para `subscribe_encoded()` + `current_encoded_snapshots()` + `tokio::select!(sender.closed())`; projeção `(CreatedAt, Severity)` em `hourly_distribution`; `notifications/outbox.rs` com *claim* em lote atômico, `explaining_ancestors` e `mark(&suppressed_ids)` em lote, e *fan-out* concorrente via `join_all`; `alerts/correlation.rs` com Union-Find transitivo e descendentes pré-computados; cache limitado em `baseline.rs`; `.limit(2)` em `adaptive_latency.rs`; `alerts::verify_all` concorrente e deduplicado.
  - **Monitoramento & Probes:** `presenter.rs` com `devices`/`probes` em lote e `fetch_recent_results` em lotes de 400; `result_processor.rs` reutilizando `device` já carregado; `ip_reconciliation.rs` com cache TTL; `saas.rs` em lote; `checkers/{ping,system_health,host_resources}.rs` usando `probe_icmp_with_client`, `collect_async` e `spawn_blocking`; `probes/receiver.rs` com `load_monitor_owners` em lote; `probes/agent.rs` com execução concorrente (`buffer_unordered(8)` recebendo `Vec<ProbeTask>` owned) e `ProbeBuffer` serializando uma única vez.
  - **Syslog, Backup, Tarefas & Pruner:** `syslog/resolver.rs` com cache limitado e `desempata` em lote; `backup/copies.rs` com JSON em `spawn_blocking` e `archive.rs` com streaming monotônico limitado a 64 MiB em espera; `scheduled_tasks` com `Semaphore(8)` e projeção `owner_names`; `data_pruner.rs` removendo `ORDER BY id` e deletando em lotes de 2.000 com isolamento de erro por tabela.
  - **API & Ferramentas de Rede:** `OnceLock<reqwest::Client>` compartilhado em `webpush/client.rs`, `speedtest.rs` e `dns/latency.rs`; `webpush/delivery.rs` com `buffer_unordered(16)`; `speedtest.rs` zero-copy com `Bytes`; `playbook.rs` com sondas TCP concorrentes; `controllers/dns.rs` retirando varredura de monitores de dentro de loop; `host_metrics.rs` via `spawn_blocking`.
- **Bugs & Fluxo:**
  - **Syslog:** correção crítica em `syslog/retention.rs` (orçamento de retenção no PostgreSQL pela fração de linhas vivas evitando remoção de ~640k/h + `by_age` em lotes); `writer.rs` com `timeout_at(deadline)`; `repository.rs` com `date_trunc` em UTC e tratamento de severidade `NULL`; `provision.rs` usando `Notify`; `logs.rs` com `select!(sender.closed())` e split IPv6 seguro; `syslog/hints.rs` aceitando `IP:porta`, `[IPv6]:porta` e `hostname:porta` válidos enquanto continua rejeitando loopback/link-local/`localhost`/porta `0`/metacaracteres de shell.
  - **Alertas & Monitoramento:** `begin_write` em `alerts/{manager,actions}.rs` evitando alertas abertos duplicados e perda de `silencedUntil`/`acknowledgedAt`; `LastRunAt <= finished_at` em `result_processor.rs`; `aggregate_range` movido para fora de `begin_write` em `rollup.rs`; correção de `scope_key` legado em `instability.rs`; método HTTP em maiúsculas em `checkers/http.rs`; `update_many` condicional em `probes/liveness.rs` e heartbeat + 404 em `probes::test`; `managed::ensure_editable` em `monitors::toggle`; cancelamento em timeout de discovery no `ProbeAgent`.
  - **Backup & Restore:** transação única via `LogsTarget::SharedMain` quando logs compartilham banco principal; rejeição de `scope`/`formatVersion` tardios; `RunGuard` em `restore` e `export_full`; compactação SQLite assíncrona; `monitor_results_hourly` em `DEPENDENT_HISTORY`; export via `begin_snapshot`; `OnConflict::do_nothing` em `plan.rs`; `prune` não *fail-fast* em `copies.rs`.
  - **Tarefas Agendadas, Auth, Diagnostics, Devices & Settings:** `recover_stale_runs`, `begin_write`, truncagem segura por bytes UTF-8 e validação alinhada por área em `scheduled_tasks`; `cancel.drop_guard()` e `select!` em `diagnostics.rs`/`traceroute.rs`/`speedtest.rs`; teto de 50 MB em `lan_upload`; validação dual-scope em `maintenance_windows.rs`; verificação de 4079 B (RFC 8188) no WebPush; bind UDP IPv6 e tratamento de `ServFail`/`Refused` em `dns/latency.rs`; `cadence::forget_device(id)`, `batch_parent` atômico e 404 em `controllers/devices.rs`; `table_exists` dialeto-agnóstico (`sqlite_master` / `information_schema.tables`) em `settings/database.rs` para tabelas opcionais de área (`discovery_results`, `discovery_runs`).
- **Segurança:**
  - `services/users.rs`: rotas de configuração/provedor/modelo de IA restritas a `Admin`; `Viewer` autorizado em `/api/registry/tokens*`; `begin_write` em create/update/delete de usuários.
  - `controllers/auth.rs` & `services/auth/setup.rs`: `verify_password_async` (Argon2 fora do worker Tokio), checagem de `user.active` em `/auth/current`, *rate-limit* em verify/magic-link/resend, e-mail normalizado para minúsculas, `begin_write` em magic-link e setup.
  - `controllers/scheduled_tasks.rs`: exigência de `Role::Admin` para tarefas `admin_only` em execução manual/edição/toggle/remoção, bloqueio quando área desabilitada e auditoria do autor real (`author.actor`).
  - `controllers/push.rs` & `services/webpush/**`: isolamento por usuário e `test_push` *fail-closed*.
  - `services/notifications/channels/telegram.rs`: escape Markdown; `views/logs.rs`: escape de injeção CSV; `syslog/{hints,destination}.rs`: validação estrita RFC 1123; `backup/copies.rs`: arquivo temporário `0600` com limpeza RAII.
- **SOLID / DRY / Limpeza:**
  - Remoção de todos os `.unwrap()`/`.expect()` de produção identificados na Fase 1 dentro de `nucleo` (`controllers/logs.rs` via `from_static`, `alerts/adaptive_latency.rs`, `monitoring/ping_diagnostics.rs`, `monitoring/rollup.rs`, `network_tools/playbook.rs`, `network_tools/dns/latency.rs`, `syslog/provision.rs`).
  - Uso canônico de `duration_to_ms` em `checkers/ping.rs` e `dns/latency.rs`; DRY de `run_plan` em backup e de `system_settings` no WebPush; preservação integral do `vpn-probe` (`AGENTS.md §6`).

---

### 3.4 Servidores — Docker & Ponte de Banco (`backend/crates/servers`)
**Escopo:** `backend/crates/servers/src/{docker,databases}/**`, `agent_host.rs`, `container_monitor.rs`, `controllers/{docker,databases}.rs`, `views/databases.rs` — 34 arquivos modificados

- **Performance:**
  - `docker/{mod.rs,source.rs,metrics.rs}`: cache de cliente `bollard::Docker` (`CACHED_CLIENT`); paralelização de `version()` + `info()` em `status_raw`; `MAX_CONCURRENT_STATS` elevado para 16.
  - `docker/remote_engine.rs` & `log_stream.rs`: migração de `operation`, `follow_logs`, `deploy_stack` e `build_images` para `AgentSession::stream` sem *spawn* extra (cancelando remoto no drop do receptor); *flush* antecipado de logs em `BATCH_MAX_CHUNKS = 256` e `interval_at`.
  - `docker/volume_export.rs` & `builds/local.rs` & `stacks/readiness.rs`: gzip `Level::Fastest`, `BufReader` de 256 KiB e `ReaderStream` de 64 KiB; paralelização de `builder_readiness` e `stacks/readiness`.
  - `databases/{postgres.rs,mysql.rs,restore.rs,dump.rs,sql/splitter.rs,backups.rs,discovery.rs}`: `next_line_ref` zero-alloc + `StatementSplitter` por *byte-slice* + `BufReader` 256 KiB + gzip `Level::Fastest`; reutilização de capacidade de `Vec` em `COPY FROM`; `sequences_query()` executada 1× (antes 3×); carregamento em lote de colunas (`load_all_columns` no Postgres e `information_schema.COLUMNS` no MySQL, eliminando 2×N queries de catálogo); cache de `StorageExplorer` por destino (`Entry::Vacant`) + `delete_many` em lote no `prune`; `discovery::resolve` usando `ports_from_inspect` sem listar todos os containers do host; `history()` limitado a 500 linhas; `try_join!` em `controllers/databases.rs`.
- **Bugs & Fluxo:**
  - `databases/{backups.rs,schedule.rs,jobs.rs,reach.rs}`: **pré-verificação de gzip e SHA-256 (`verify_backup_integrity`) antes de `DROP DATABASE` no MySQL `RestoreMode::Replace`**; unificação de `BACKUP_SCOPE`/`RESTORE_SCOPE` em `"database-connection-op"` (impede backup + restore simultâneos no mesmo banco); `recover_stuck_running` no boot do scheduler; reset de progresso do `Job` em retry + `Mutex` único + publicação forçada na troca de estágio; *backoff* na mesma sessão em `Reached::retrying_within`.
  - `databases/sql/splitter.rs` & `mysql.rs`: preservação de `\r` dentro de literais SQL (round-trip CRLF); suporte a comentários condicionais MySQL `/*!...*/;`; erro explícito de `max_allowed_packet` quando `CONCAT_WS` retorna `NULL`; aviso para colunas espaciais; `strip_definer` restrito ao cabeçalho `CREATE` com espaços opcionais em torno de `=`.
  - `docker/{operations.rs,builds/local.rs,builds/artifacts.rs,stacks/{deploy.rs,git.rs,registry.rs},volume_export.rs}`: substituição de `Instant::now() - PROGRESS_THROTTLE` (que podia entrar em pânico em sistemas recém-iniciados) por `Option<Instant>`; guarda de concorrência `(host, kind, target)` (`409 Conflict`); `Mutex` por projeto em `stacks/deploy.rs`; `safe.directory=*` em `git rev-parse HEAD`; correção de erros engolidos e possível *hang* em `import()`; limpeza assíncrona de `WorkDir`/`EphemeralDockerConfig`/`ArtifactGuard` com fallback `spawn_blocking` no `Drop`; remoção de `start_container` desnecessário em `volume_export.rs` e guarda `Handle::try_current()` no `Drop`; `BRIDGE_TIMEOUT = 25s >= OPEN_TIMEOUT`.
- **Segurança:**
  - `agent_host.rs`, `docker/{log_clear.rs,update.rs,protection.rs,stacks/deploy.rs}`: proteção de containers marcados com `com.netmonitor.protected=true` garantida em `ClearLogs` (guarda no agente + defesa em profundidade em `log_clear.rs`), `update.rs` (`is_protected_inspect`) e `stacks/deploy.rs` (não mais pulada quando `working_dir` ausente); identificação do próprio container aprimorada via `/proc/self/mountinfo` (`OnceLock`) com fallback para `HOSTNAME`; `call_within(LIFECYCLE_TIMEOUT, ...)` nas chamadas de ciclo de vida; `validate_identifier` em `resolve_container`.
- **SOLID / DRY / Limpeza:**
  - Remoção de todos os 4 `.expect()` de produção em `databases/sql/{order.rs,splitter.rs}`; unificação de `streamed_with` em `agent_host.rs` e de `cli::{forward_lines,capture_with_envs}` (`kill_on_drop(true)`).
- **Novos testes adicionados (`netmonitor-servers`: 316 passed):**
  - Proteção de `ClearLogs`, CRLF round-trip no splitter SQL, comentários condicionais `/*!...*/;`, `strip_definer` restrito a `CREATE`, `Job::reset_database_progress`, `ports_from_inspect`.

---

### 3.5 Servidores — Deploys Contínuos & Container Registry
**Escopo:** `backend/crates/servers/src/deploys/**`, `controllers/{deploys,deploy_builds,deploy_github}.rs`, `ai_tools.rs` (27 arquivos) e `backend/crates/servers-registry/src/**` (18 arquivos)

- **Performance:**
  - `servers-registry`: `OnceLock<reqwest::Client>` (timeout de leitura 60s) em `scan/osv.rs`; ondas concorrentes limitadas (`16`) no sync incremental do OSV; `advisories_for_ecosystem()` em lote; `catalog::put_manifest` com queries `IN` em lotes de 400 (elimina 2N queries); `browse.rs` com `SUM` em SQL e consulta escopada de usuários; `spawn_blocking` para SHA-256 (`uploads.rs`, `ingest.rs`) e descompressão gzip/tar (`scan/mod.rs` + `SyncIoBridge`); `tokens::Scope::covers`, `scan/advisory::same_ecosystem` e `scan/version::apk` sem alocações; `Exceptions::active` filtrado em SQL; publicação SSE única em `scan::refresh()`.
  - `servers/deploys`: `OnceLock<reqwest::Client>` compartilhado em `github/mod.rs`; `select_without_log()` em `runs.rs` e `builds/` + projeção de 3 colunas em `usage.rs` (evita ler blobs de log de até 256 KiB em listagens); `LogBuffer` em `VecDeque` (O(1)); flag `dirty` em `console.rs` evitando gravações redundantes; consultas em lote `live_by_targets`, `current_by_targets`, `counts_24h` (`GROUP BY`) e `builds::views` (2 queries); `tokio::try_join!` / `try_join_all` em `github/{discovery,publish}.rs`, `deploys/ai_tools.rs` e `servers/ai_tools.rs` (eliminando duplicação do agente local em `list_all_container_names`); pulo de RPC `builder_readiness()` quando `!allows_build`.
- **Bugs & Fluxo:**
  - `servers-registry/src/{api/uploads.rs,api/blobs.rs,api/manifests.rs,storage.rs,tarstream.rs,archive.rs,catalog.rs,ingest.rs,retention.rs,scan/{osv.rs,mod.rs,version.rs}}`:
    - *Lock* `tokio::sync::Mutex` por upload + validação de *offset* com rollback `set_len` em erro + validação de `Content-Range` no `finish` + limpeza de staged file em erro de DB no `start`.
    - HTTP `416 Range Not Satisfiable` + `Content-Range: bytes */{size}` em `api/blobs.rs`.
    - Tratamento de corrida de `rename` no Windows em `LocalDisk::commit` + `spawn_blocking` no `Staged::drop`.
    - Suporte a cabeçalhos PAX `path=` e `size=` (>8 GiB) em `tarstream.rs` + cópia limitada com `.take(size)` em `archive.rs`.
    - `OnConflict::do_nothing()` em `catalog.rs`; `begin_write` em `manifests.rs`, `ingest.rs`, `retention.rs` e `scan/mod.rs`; exclusão de blobs já existentes na checagem de cota em `ingest.rs`.
    - `retention.rs`: `RunGuard` em `collect()`, expansão de referências/referrers encadeados para manifestos sem tag em período de graça, e remoção em disco somente se o registro em banco foi efetivamente deletado sob `begin_write`.
    - `scan/osv.rs`: serialização por *feed* com `Mutex` assíncrono, deduplicação de lotes upsert (paridade PostgreSQL `ON CONFLICT DO UPDATE`), validação de `all.zip` antes do swap transacional, sync incremental cronológico com avanço contíguo do cursor apenas sem erro, e `COUNT(*)` exato em `feed.entries`; comparação SemVer pre-release em `scan/version.rs`.
  - `servers/src/deploys/{executor.rs,orchestrator.rs,runs.rs,builds/*,connections.rs,github/compose.rs,workflow.rs,applications.rs}`:
    - `flush_pending()` a cada 300ms no `select!` de `deliver()` e `builds/runner.rs`; `env_version_id` incluído em `auto_rollback`; 3 tentativas para `runs::transition` final.
    - `orchestrator.rs`: `begin_write` atômico em supersede + insert; cancelamento de downstream pendente quando upstream falha; rechecagem de janela de manutenção ao disparar `Scheduled`; diferenciação `NotFound` vs erro de DB; `finished_at` do upstream considerado em `queued_since`.
    - `builds/`: deduplicação monorepo `(application_id, delivery_id)`; transições condicionais (`update_many` direcionado sem sobrescrever por `SELECT *`); registro de token de cancelamento antes de `start_running`; upsert atômico `OnConflict` em `builds/secrets.rs`.
    - `connections::claim()` resiliente (loga e continua); correção de stripping de porta de registry em `github/compose.rs`; emissão de `target:` em `workflow.rs` quando `image.target` definido; validação de `build_probe_id`.
- **Segurança:**
  - `servers-registry/src/auth.rs` & `api/mod.rs` & `controllers.rs`: exigência de `user.active` em `auth::login`; `realm()` só aceita `X-Forwarded-*` quando origem é interna; recusa *fail-closed* (`!origin.is_internal()` -> `Code::Denied`) de tokens pessoais pela borda pública; registro de auditoria em `sync_advisories`.
  - `servers/src/deploys/{images.rs,oidc.rs,webhook.rs,github/app.rs,ai_tools.rs}`: `VulnerabilityPolicy::blocks` *fail-closed* para `ScanStatus::Failed`/`Missing` e `verify_images` antes do backup pré-deploy; `jti` obrigatório em `oidc::verify()`; rejeição de todos os eventos `pull_request*` (bloqueia `pull_request_target`); limite de 256 entradas no mapa `PENDING` de OAuth; correção de vazamento de escopo MCP em `DeployGuide`.
- **SOLID / DRY / Limpeza:**
  - Remoção do `.expect()` em `servers-registry/src/scan/mod.rs:112`; eliminação de `manifest::parse` duplicado; `validate_repository` delegando para `github::split_repository`; remoção de queries redundantes de `run_label` em `controllers/deploys.rs`; correção de espaços sobrando em mensagens de erro.
- **Novos testes adicionados (`servers-registry`: 51 passed; `servers`: 316 passed):**
  - HTTP `416`, PAX >8 GiB, SemVer pre-release, `VulnerabilityPolicy` *fail-closed*, OIDC `jti` obrigatório, `pull_request_target` bloqueado, `compose.rs` porta de registry, `workflow.rs` `target:`, `DeployGuide` MCP scope.

---

### 3.6 Rede — Discovery, SNMP, Plugins, Topologia, Throughput & Port Scanner (`backend/crates/network`)
**Escopo:** `backend/crates/network/src/**` — 41 arquivos modificados

- **Performance:**
  - `plugins/`: cache `OnceLock` de pacotes *builtin* e checksums (`builtin::cached()`); eliminação de `static_scan::scan` duplicado em `full_review`; cache limitado de AST Rhai por SHA-256 (`compile_cached`) e de `Regex` (`cached_regex`), mais `OnceLock` no `<title>` de `fingerprint.rs`; `LocalTransport::shared` reutilizado; carregamento em lote de bibliotecas (`Slug.is_in`) e plugins em `fingerprint`; `BatchState` em `fleet.rs` sem round-trip JSON por atualização; liberação de lock `ssh_sessions` antes de `ssh::conecta_vendo_host` com poda e teto de 128 sessões.
  - `snmp/`: `detect_connection` com *short-circuit* (perfil preferido primeiro, depois `FuturesUnordered`); `compatible_client: OnceCell<Client>` e cache limitado (512) de `MasterKeys` calculado fora do mutex; `record_metrics_bulk` em lotes de 100 dentro de `begin_write`; `routines.rs` trocando loop N+1 de `next_run_at` por único `update_many`.
  - `topology/` & `discovery/` & `port_scanner.rs` & `btest/` & `networks.rs`: `HashMap` em `link_resolver.rs` (O(E)) e `resolve_discovered_neighbors` em lote; `OnceLock<reqwest::Client>` em `discovery/scanners/http.rs`; `std::sync::Mutex` em `AdaptiveTimeout` e `UdpSocket::bind` fora do loop de retry + `buffer_unordered` em `port_scanner.rs`; *fast-skip* de bytes `0x00` em `StatusScanner::feed` (`btest/codec.rs`); `sites` em lote em `controllers/networks.rs`.
- **Bugs & Fluxo:**
  - `plugins/`: `Drop` em `Prepared` + `CancellationGuard` em `run_ad_hoc`; `execute_with_cancel` / `execute_streaming_with_cancel` propagando `CancellationToken` para `AgentTransport` e `DeviceIoCommand`; teto `MAX_OUTPUT_BYTES` nos buffers SSH/Telnet (`shell.rs`); em `dedicated_access::rotate_on`, nova chave gravada no DB **antes** de aposentar a antiga.
  - `snmp/`: correção de `counter_diff` e `calculate_rates_detailed` (reset vs wrap 64-bit, troca 64↔32-bit, proteção de *underflow* 32-bit); validação IPv4 (`addrType == 1 && addrLen == 4`) em `parse_arp_table`; remoção da deleção destrutiva de monitores/métricas CPU/memória em `scan_device` e guarda `!collector_errors` / `matched_ids()` em `apply_monitors`; `begin_write` no sync de interfaces de `poll_device`; `snmp/client.rs` não faz fallback para `compatible_walk` em `Timeout`/`Auth`; suporte completo a campos SNMPv3 em `SnmpCheckerConfig` + `try_acquire_snmp_device`; filtros `Enabled` + `ProbeId.is_null()` em `poll_device_now`.
  - `topology/`: normalização da orientação `(source_interface_id, target_interface_id)` em `is_material_change` e `Overwrite`; `create_unmanaged_switch` em `begin_write` + `insert_many`.
  - `discovery/`, `port_scanner`, `btest`, `package_mirror`: janela de 24h (`LastSeenAt`) + `begin_write` em `conflicts.rs` (chamado mesmo quando vazio para limpar alertas); `ScanSessionService::try_reserve` atômico + `session.finish` garantido + `begin_write` em `persist_results`/`enqueue_network_scan`/`claim_pending_run`; bloqueio de discovery remoto concorrente por probe; `AbortOnDrop` em multicast e tarefas `btest/duplex.rs`; `CancelOnDrop` no stream NDJSON do `port_scan`; validação de IP do respondedor UDP em `scanners/names.rs`; limite `MAX_BYTES` aplicado por chunk em `controllers/package_mirror.rs`.
- **Segurança:**
  - `plugins/service.rs`: bloqueio de `set_enabled` e `duplicate` para plugins em `status::QUARANTINE`.
  - `plugins/runtime.rs` & `runs.rs` & `review/static_scan.rs`: bloqueio de `reveals: true` em `use_plugin`, interseção de transportes da biblioteca, guarda RAII `NestedGuard`, restrição de downgrade `login: true` apenas para HTTP `POST`, mascaramento de segredos em `stdout`/`stderr`/body HTTP antes de entregar ao Rhai; `static_scan` ignorando strings/comentários no pareamento de chaves e inspecionando cada chamada HTTP individualmente.
  - `plugins/auto_accept.rs` & `plugins/ai_tools.rs`: `active_for_user(db, key, device_id, user_id)` (escopado por usuário), `require_admin` em `RunPluginTests::execute`, mascaramento de parâmetros secretos em `RunPluginAction::preview`.
  - `area_policy.rs`: inclusão de `/api/devices/*/access`, `/api/devices/*/interfaces` e prefixo de evento `interface:`.
- **SOLID / DRY / Limpeza:**
  - Deduplicação de `interface_monitor_status` e `version_label` em `snmp/`; `build_report_with_findings` em `review/`; projeção mínima de `Slug` em `duplicate` e `(Id, Name)` em `runs::recent`.
- **Novos testes adicionados (`netmonitor-network`: 530 passed, 1 ignored):**
  - `auto_accept::active_for_user`, bloqueio de `QUARANTINE`, bloqueio de `reveals` em `use_plugin`, `login: true` restrito a `POST`, mascaramento de segredos em saída para Rhai, `static_scan` com strings/comentários e HTTP por chamada, `counter_diff`/ARP IPv4/`record_metrics_bulk`, orientação de link na topologia.

---

### 3.7 Rede (Wi-Fi & Link Tests) e VPN WireGuard
**Escopo:** `backend/crates/network-wifi/src/**` (8 arquivos), `backend/crates/network-link-tests/src/**` (7 arquivos), `backend/crates/vpn/src/**` (14 arquivos)

- **VPN WireGuard (`crates/vpn` — 14 arquivos):**
  - **Segurança & Confiabilidade:** `config_writer.rs` usa arquivo temporário único (`.{name}.{pid}.{rand:016x}.tmp`) com `OpenOptions::create_new(true)` + `0600` (Unix) antes da escrita, `sync_all`, `rename` atômico e limpeza em erro; `server_service.rs` serializa `create_or_update` e `apply_configuration` com `tokio::sync::Mutex` estático e `begin_write`; `peer_service.rs` executa `PeerAgent` + `assert_available` dentro do `begin_write` de `create_with_device` (conflito -> `409`), envolve `rotate_keys_with_previous` em `begin_write`, checa `supports_qr_code()` antes de consumir chave *one-time* em `build_qr_artifact`, e chama `ResourceCleanupService::delete_device` antes de deletar o peer em `revoke`.
  - **Bugs & Validação:** `cidr.rs` mantém faixa vazia em `iterate_usable_addresses` para `/31` e `/32` (onde WireGuard não tem faixa de peers); `ip_allocator.rs` valida IP manual contra rede/broadcast/`/31`/`/32`/IP do servidor e usa `HashSet<Ipv4Addr>` (zero alocação de `String` por candidato); `peer_status.rs` rejeita contadores não-numéricos/negativos em `parse_wg_dump` e envolve updates em `begin_write`.
  - **Performance & DRY:** `traffic_recorder.rs` reutiliza peers carregados no ciclo inteiro, substitui gravações periódicas no `event_outbox` a cada 10s por snapshots em memória (`publish_snapshot`/`forget_snapshots`), mantém cache de contadores RX/TX e grava `insert_many` em lote sob `begin_write`; `alerts/watcher.rs` faz *short-circuit* de peers estáveis e busca `devices::is_in` em lote só para peers em transição; `preflight.rs` paraleliza via `tokio::join!` com timeout de 5s em DNS e `tokio::net::UdpSocket` assíncrono; `server_service.rs` & `peer_service.rs` com `HashMap` O(N); remoção de SQL bruto nos controllers e de helpers duplicados.
- **Link Tests (`crates/network-link-tests` — 7 arquivos):**
  - **Bugs & Fluxo:** `janitor.rs` (`recover_before`) evita matar testes ativos/recém-criados (`guard::is_running(id) || created_at > boot_time`), usa `begin_write`, publica evento SSE e registra falha no agendamento; `service.rs` usa `UPDATE` condicional transacional (`status IN ('queued','running')`, `409` se já finalizado) fechando pernas pendentes e `finished_at`; `plan.rs` corrige `estimated_seconds` de `btest` bidirecional e protege `is_in` vazios; `routines.rs` dispara `schedules::run_due` em tarefa destacada sob `RunGuard` para SSH lento não travar o scheduler global.
  - **Performance:** `runner.rs` separa `persist` vs `persist_with_samples` (evita serializar blobs grandes de amostras/saúde a cada progresso); `repository.rs` substitui `.reset_all()` por `Set(...)` explícitos, agrupa `page` com `HashMap` O(P+L) e usa `begin_write` por lote no `prune`; `health.rs` move `/proc` para `spawn_blocking` e cacheia community SNMP padrão (`OnceCell`).
- **Wi-Fi (`crates/network-wifi` — 8 arquivos):**
  - **Bugs & Fluxo:** `roaming/detect.rs` corrige `Location::beats` comparando `inactive_ms` somente quando ambos os APs reportam `Some(...)` e limita `MAX_KNOWN_MACS = 10_000`; `roaming/mod.rs` serializa `track_with_settings` com `TRACK_LOCK` e grava em lote sob `begin_write`; `collector.rs` corrige O(N²) em `claim()` via `HashMap`, mescla refresh de AP único com `current_snapshots` antes de detectar roaming; `live.rs` corrige fallback de erro de DB de `0u32` (tempestade de 1s) para `WifiSettings::default().live_interval_s`; `ai_tools.rs` usa tarefa destacada + `oneshot` no timeout de `rounds::run` para não deixar `WifiReadStatus::Reading` preso.
  - **Performance & DRY:** `watch.rs` reutiliza `roaming::detect::locate` (`index_stations`), pré-carrega `devices::is_in` e grava em lote sob `begin_write`; `watchlist.rs` evita `UPDATE` redundante para offline e mapeia violação única para `409`; `routines.rs` checa `live::is_watched` antes de `settings::load` e `areas::is_enabled` antes de `tokio::spawn`.
- **Novos testes adicionados (`vpn`: 159 passed; `network-link-tests`: 115 passed; `network-wifi`: 68 passed):**
  - Escrita concorrente de `wg0.conf`, validação de IP manual, `parse_wg_dump` com contadores inválidos, `btest` bidirecional estimado, `Location::beats` com `inactive_ms` ausente.

---

### 3.8 Assistente IA, MCP e Wiring Raiz (`backend/crates/assistant`, `backend/crates/mcp`, `backend/src`)
**Escopo:** `backend/crates/assistant/src/**` (28 arquivos), `backend/crates/mcp/src/**` (4 arquivos), `backend/src/**` exceto `bin/agent.rs` (7 arquivos)

- **Segurança & Auth:**
  - `assistant/src/controllers/ai.rs`: `require_authenticated_actor` (401 *fail-closed* quando `user_id: None`) nos 4 endpoints HTTP (`chat_stream`, `execute_tool`, `confirm_approval`, `dismiss_approval`); `require_admin` em todos os endpoints administrativos de IA/decisor/modelos; `local_containers` condicionado a `Area::Servers`; substituição de `.unwrap_or_default()` por `?` em `AuditActor::from_headers`.
  - `assistant/src/approvals.rs`, `central.rs`, `harness/{agent.rs,confirmation.rs}` & `backend/src/registries.rs`: chaves de aprovações `pending`/`settled`/SSE escopadas por `(user_id, call_id)` + `AbortHandle`s abortados no `Drop`; auto-aceite de equipamento (`ToolKind::DeviceAccess`) escopado por `user_id` (`auto_accept::active_for_user`); `proactive::runner::ask` passando `ToolPolicy::passive()` explicitamente.
  - `assistant/src/{settings.rs,drivers/mod.rs}`: recusa em reaproveitar/desmascarar `opencode_api_key` e `decider.api_key` quando `base_url` muda (impede exfiltração de chave para endpoint arbitrário); `begin_write` em `save` e `save_decider`.
  - `mcp/src/{scopes.rs,protocol.rs,auth.rs}`: subação desconhecida em `deploy_action` recusada (*fail-closed*); limite de lote JSON-RPC (`MAX_BATCH_SIZE = 20`) com cobrança de *rate-limit* por item; correção de erro de digitação de 14 espaços em `auth.rs`.
  - `backend/src/app.rs` & `spa.rs`: checagem de `JWT_SECRET` alinhada a `ctx.environment == Environment::Production`; resposta `404` JSON para rotas desconhecidas `/api`, `/api/{*rest}`, `/v2`, `/v2/{*rest}` (em vez de servir `index.html` com 200).
- **Performance, Streaming & Fluxo:**
  - `assistant/src/drivers/openai_compatible.rs` & `{ollama,opencode,openrouter,local_models}.rs` & `context_window/*` & `web_search/*`: `OnceLock<reqwest::Client>` compartilhado em todos os clientes HTTP; `SseParser` e `pipe_ndjson_response` com buffer de bytes O(N) preservando UTF-8 multibyte fatiado entre chunks, `data:` sem espaço e *flush* de tool-calls no EOF; arquivos temporários `.partial` únicos e `OLLAMA_CREATE_LOCK` em `local_models.rs`; cache de `context_window` limitado a 2.048 entradas com recuperação de *poison*; *cache* negativo de 20s em `local_server.rs`; cópia limitada de chunks em `web_search`.
  - `assistant/src/harness/{agent.rs,grounding.rs,turn.rs,lean.rs}`: cache de `(tools, tools_tokens)` por rodada; retry de `AnswerCheck` na rodada final e tratamento de `HarnessEvent::Retract` em `collect_answer`; `InventorySnapshot::load` em única query projetada; `redact` case-insensitive seguro para Unicode (`RegexBuilder`); `size(&Value)` sem alocação em `turn.rs`; `short_instant` seguro para fronteiras UTF-8 em `lean.rs`.
  - `assistant/src/{conversations.rs,mentions.rs,decider/runtime.rs,proactive/*,data_export/*}`: `ByteCounter` zero-alloc + `begin_write` em `conversations.rs`; `search_with_areas` + projeção `into_tuple()` em `mentions.rs`; `DeciderRuntime::install` + rechecagem de *circuit breaker* pós-semáforo; `proactive` com `begin_write`, `wants_summary` fora do loop de broadcast, `Semaphore(2)` + `InFlightGuard` e checagem de `quiet_period` antes de gravar `LAST_SENT_KEY`; `data_export` com paginação *keyset* (`id > after_id`), streaming de `device_runs` direto para o ZIP e reuso único do mapa de usuários.
  - `mcp/src/{protocol.rs,settings.rs}`: `RequestCache` carregando `enabled_areas` e `all_handlers()` uma única vez por requisição HTTP; query única `is_in` em `settings::load`.
  - `backend/src/{spa.rs,initializers/*,tasks/scheduler_run.rs,registries.rs}`: `.precompressed_gzip()` no `ServeFile`; `EventBus::seed_from_outbox(&ctx.db)` no boot + pulo de `spawn_event_relay` e banner stdout em `Environment::Test`; `DeciderRuntime` instalado em `process_deps`; `scheduler_run.rs` atualizando `next_run_at` em lote (`begin_write`, lotes de 400 agrupados por intervalo), log-and-continue em `run_group`, *shutdown* gracioso (`ctrl_c`/`SIGTERM`) preservando o fallback local em `execute_one` (`AGENTS.md §6`); `drivers::connect` movido para dentro do `tokio::time::timeout` em `NetworkCentral::ask_ai`.
- **SOLID / DRY / Limpeza:**
  - Helper SSE unificado (`sse_json_response`) e delegação de `test_connection` para `drivers::from_test_input` em `controllers/ai.rs`.
- **Novos testes adicionados (`assistant`: 324 passed; `mcp`: 10 passed; `backend --lib`: 34 passed):**
  - Isolamento cross-user em `approvals.rs`, proteção de chave ao trocar `base_url`, `deploy_action` desconhecido *fail-closed*, `SseParser` com UTF-8 fatiado e EOF flush, `HarnessEvent::Retract`, `redact` Unicode, `short_instant` UTF-8, rotas `/api` e `/v2` desconhecidas retornando 404 JSON.

---

## 4. Inventário Completo de Arquivos Modificados (`git diff HEAD --numstat`)

Total verificado via `git diff --numstat`: **357 arquivos modificados**. **+17.877** inserções / **-6.561** deleções.

| Grupo / Crate | Arquivos | Inserções | Deleções |
| :--- | ---: | ---: | ---: |
| 1. Agente Remoto — Runtime, Central & Serviço (`nucleo` / `bin/agent` / `assets`) | 18 | `+1740` | `-508` |
| 1. Agente Remoto — `crates/agent-protocol` | 8 | `+875` | `-208` |
| 1. Agente Remoto — `crates/core/src/agents` | 13 | `+783` | `-187` |
| 2. Core & Infraestrutura — `crates/core` | 43 | `+1534` | `-768` |
| 3. Núcleo — `crates/nucleo` | 87 | `+4023` | `-1540` |
| 4. Servidores — Docker & Ponte de Banco (`crates/servers`) | 34 | `+1535` | `-579` |
| 5. Servidores — Container Registry (`crates/servers-registry`) | 18 | `+1288` | `-474` |
| 5. Servidores — Deploys (`crates/servers`) | 27 | `+1282` | `-473` |
| 6. Rede — Discovery, SNMP, Plugins, Topologia & Throughput (`crates/network`) | 41 | `+1682` | `-522` |
| 7. Rede (Link Tests) — `crates/network-link-tests` | 7 | `+267` | `-83` |
| 7. Rede (Wi-Fi) — `crates/network-wifi` | 8 | `+356` | `-136` |
| 7. VPN WireGuard — `crates/vpn` | 14 | `+898` | `-409` |
| 8. Assistente IA — `crates/assistant` | 28 | `+1358` | `-615` |
| 8. MCP — `crates/mcp` | 4 | `+96` | `-25` |
| 8. Wiring Raiz — `backend/src` | 7 | `+160` | `-34` |
| **Total** | **357** | **`+17877`** | **`-6561`** |

<details>
<summary><strong>1. Agente Remoto — Runtime, Central & Serviço (nucleo / bin/agent / assets)</strong> — 18 arquivos (<code>+1740</code> / <code>-508</code>)</summary>

| Arquivo | Inserções | Deleções |
| :--- | ---: | ---: |
| `backend/assets/agent/netmonitor-agent.service` | `+1` | `-0` |
| `backend/crates/nucleo/src/controllers/agents.rs` | `+163` | `-73` |
| `backend/crates/nucleo/src/services/agent_runtime/client.rs` | `+231` | `-66` |
| `backend/crates/nucleo/src/services/agent_runtime/executor.rs` | `+4` | `-1` |
| `backend/crates/nucleo/src/services/agent_runtime/identity.rs` | `+15` | `-7` |
| `backend/crates/nucleo/src/services/agent_runtime/outbox.rs` | `+169` | `-40` |
| `backend/crates/nucleo/src/services/agent_runtime/sampler.rs` | `+37` | `-8` |
| `backend/crates/nucleo/src/services/agent_runtime/session.rs` | `+386` | `-105` |
| `backend/crates/nucleo/src/services/agent_runtime/tunnel.rs` | `+52` | `-29` |
| `backend/crates/nucleo/src/services/agents/background.rs` | `+226` | `-31` |
| `backend/crates/nucleo/src/services/agents/connection.rs` | `+156` | `-73` |
| `backend/crates/nucleo/src/services/agents/install.rs` | `+7` | `-2` |
| `backend/crates/nucleo/src/services/agents/install_address.rs` | `+37` | `-19` |
| `backend/crates/nucleo/src/services/agents/local.rs` | `+43` | `-21` |
| `backend/crates/nucleo/src/services/agents/mod.rs` | `+34` | `-0` |
| `backend/crates/nucleo/src/services/agents/service.rs` | `+134` | `-30` |
| `backend/crates/nucleo/src/services/agents/snapshots/netmonitor_nucleo__services__agents__install__tests__script_de_instalacao.snap` | `+8` | `-2` |
| `backend/src/bin/agent.rs` | `+37` | `-1` |

</details>

<details>
<summary><strong>1. Agente Remoto — crates/agent-protocol</strong> — 8 arquivos (<code>+875</code> / <code>-208</code>)</summary>

| Arquivo | Inserções | Deleções |
| :--- | ---: | ---: |
| `backend/crates/agent-protocol/src/device_io.rs` | `+43` | `-6` |
| `backend/crates/agent-protocol/src/docker/images.rs` | `+46` | `-5` |
| `backend/crates/agent-protocol/src/docker/stacks.rs` | `+52` | `-5` |
| `backend/crates/agent-protocol/src/policy.rs` | `+2` | `-1` |
| `backend/crates/agent-protocol/src/protocol.rs` | `+181` | `-42` |
| `backend/crates/agent-protocol/src/telemetry.rs` | `+194` | `-77` |
| `backend/crates/agent-protocol/src/throughput.rs` | `+1` | `-1` |
| `backend/crates/agent-protocol/src/tunnel.rs` | `+356` | `-71` |

</details>

<details>
<summary><strong>1. Agente Remoto — crates/core/src/agents</strong> — 13 arquivos (<code>+783</code> / <code>-187</code>)</summary>

| Arquivo | Inserções | Deleções |
| :--- | ---: | ---: |
| `backend/crates/core/src/agents/bridge.rs` | `+331` | `-53` |
| `backend/crates/core/src/agents/database_targets.rs` | `+33` | `-5` |
| `backend/crates/core/src/agents/enrollment.rs` | `+1` | `-4` |
| `backend/crates/core/src/agents/errors.rs` | `+23` | `-4` |
| `backend/crates/core/src/agents/host.rs` | `+1` | `-4` |
| `backend/crates/core/src/agents/hub.rs` | `+20` | `-8` |
| `backend/crates/core/src/agents/identity.rs` | `+17` | `-2` |
| `backend/crates/core/src/agents/install_policy.rs` | `+9` | `-7` |
| `backend/crates/core/src/agents/registration.rs` | `+63` | `-34` |
| `backend/crates/core/src/agents/routes.rs` | `+5` | `-2` |
| `backend/crates/core/src/agents/session.rs` | `+209` | `-53` |
| `backend/crates/core/src/agents/targets.rs` | `+70` | `-10` |
| `backend/crates/core/src/agents/views.rs` | `+1` | `-1` |

</details>

<details>
<summary><strong>2. Core & Infraestrutura — crates/core</strong> — 43 arquivos (<code>+1534</code> / <code>-768</code>)</summary>

| Arquivo | Inserções | Deleções |
| :--- | ---: | ---: |
| `backend/crates/core/src/ai/lookup.rs` | `+39` | `-14` |
| `backend/crates/core/src/ai/series.rs` | `+1` | `-1` |
| `backend/crates/core/src/areas/policy.rs` | `+14` | `-11` |
| `backend/crates/core/src/audit.rs` | `+5` | `-5` |
| `backend/crates/core/src/devices/systems.rs` | `+2` | `-2` |
| `backend/crates/core/src/events/bus.rs` | `+103` | `-15` |
| `backend/crates/core/src/events/mod.rs` | `+1` | `-1` |
| `backend/crates/core/src/events/relay.rs` | `+38` | `-7` |
| `backend/crates/core/src/maintenance/resource_cleanup.rs` | `+121` | `-67` |
| `backend/crates/core/src/migrations/m20220101_000001_users.rs` | `+1` | `-1` |
| `backend/crates/core/src/migrations/m20260810_000011_monitor_results.rs` | `+6` | `-0` |
| `backend/crates/core/src/migrations/m20260810_000012_metrics.rs` | `+7` | `-1` |
| `backend/crates/core/src/migrations/m20260810_000016_alert_events.rs` | `+12` | `-0` |
| `backend/crates/core/src/models/monitors.rs` | `+9` | `-2` |
| `backend/crates/core/src/models/system_settings.rs` | `+19` | `-13` |
| `backend/crates/core/src/models/users.rs` | `+36` | `-17` |
| `backend/crates/core/src/monitoring/device_status.rs` | `+18` | `-8` |
| `backend/crates/core/src/monitoring/execution_guard.rs` | `+94` | `-125` |
| `backend/crates/core/src/monitoring/health/mod.rs` | `+8` | `-0` |
| `backend/crates/core/src/monitoring/health/parsers.rs` | `+16` | `-15` |
| `backend/crates/core/src/monitoring/health/sources.rs` | `+34` | `-3` |
| `backend/crates/core/src/monitoring/metrics_repository.rs` | `+56` | `-63` |
| `backend/crates/core/src/monitoring/mod.rs` | `+1` | `-1` |
| `backend/crates/core/src/network_tools/icmp_probe.rs` | `+61` | `-14` |
| `backend/crates/core/src/network_tools/nat.rs` | `+2` | `-1` |
| `backend/crates/core/src/network_tools/neighbor_cache.rs` | `+23` | `-9` |
| `backend/crates/core/src/network_tools/ping_client.rs` | `+10` | `-5` |
| `backend/crates/core/src/network_tools/public_ip.rs` | `+14` | `-5` |
| `backend/crates/core/src/routines/cadence.rs` | `+22` | `-5` |
| `backend/crates/core/src/shared/cidr.rs` | `+62` | `-7` |
| `backend/crates/core/src/shared/client_addr.rs` | `+33` | `-25` |
| `backend/crates/core/src/shared/crypto.rs` | `+6` | `-5` |
| `backend/crates/core/src/shared/incident.rs` | `+4` | `-4` |
| `backend/crates/core/src/shared/pagination.rs` | `+6` | `-1` |
| `backend/crates/core/src/shared/rate_limit.rs` | `+71` | `-20` |
| `backend/crates/core/src/shared/runtime.rs` | `+11` | `-0` |
| `backend/crates/core/src/shared/secret_file.rs` | `+30` | `-7` |
| `backend/crates/core/src/shared/secret_store.rs` | `+52` | `-26` |
| `backend/crates/core/src/storage/cloud.rs` | `+95` | `-39` |
| `backend/crates/core/src/storage/local.rs` | `+90` | `-47` |
| `backend/crates/core/src/storage/service.rs` | `+17` | `-2` |
| `backend/crates/core/src/storage/sftp.rs` | `+117` | `-46` |
| `backend/crates/core/src/telemetry/store.rs` | `+167` | `-128` |

</details>

<details>
<summary><strong>3. Núcleo — crates/nucleo</strong> — 87 arquivos (<code>+4023</code> / <code>-1540</code>)</summary>

| Arquivo | Inserções | Deleções |
| :--- | ---: | ---: |
| `backend/crates/nucleo/src/controllers/alerts.rs` | `+38` | `-5` |
| `backend/crates/nucleo/src/controllers/auth.rs` | `+94` | `-32` |
| `backend/crates/nucleo/src/controllers/backup.rs` | `+4` | `-18` |
| `backend/crates/nucleo/src/controllers/devices.rs` | `+52` | `-26` |
| `backend/crates/nucleo/src/controllers/diagnostics.rs` | `+27` | `-7` |
| `backend/crates/nucleo/src/controllers/dns.rs` | `+45` | `-32` |
| `backend/crates/nucleo/src/controllers/events.rs` | `+64` | `-38` |
| `backend/crates/nucleo/src/controllers/logs.rs` | `+27` | `-20` |
| `backend/crates/nucleo/src/controllers/monitors.rs` | `+18` | `-15` |
| `backend/crates/nucleo/src/controllers/probes.rs` | `+35` | `-11` |
| `backend/crates/nucleo/src/controllers/push.rs` | `+20` | `-9` |
| `backend/crates/nucleo/src/controllers/scheduled_tasks.rs` | `+2` | `-8` |
| `backend/crates/nucleo/src/controllers/settings.rs` | `+1` | `-1` |
| `backend/crates/nucleo/src/controllers/storages.rs` | `+5` | `-0` |
| `backend/crates/nucleo/src/dtos/resources.rs` | `+11` | `-19` |
| `backend/crates/nucleo/src/services/ai_tools/actions.rs` | `+29` | `-0` |
| `backend/crates/nucleo/src/services/ai_tools/alert_rules.rs` | `+159` | `-116` |
| `backend/crates/nucleo/src/services/ai_tools/analysis.rs` | `+24` | `-8` |
| `backend/crates/nucleo/src/services/ai_tools/audit.rs` | `+13` | `-9` |
| `backend/crates/nucleo/src/services/alerts/actions.rs` | `+36` | `-14` |
| `backend/crates/nucleo/src/services/alerts/adaptive_latency.rs` | `+5` | `-2` |
| `backend/crates/nucleo/src/services/alerts/baseline.rs` | `+14` | `-0` |
| `backend/crates/nucleo/src/services/alerts/correlation.rs` | `+65` | `-39` |
| `backend/crates/nucleo/src/services/alerts/episode.rs` | `+16` | `-2` |
| `backend/crates/nucleo/src/services/alerts/inhibition.rs` | `+111` | `-2` |
| `backend/crates/nucleo/src/services/alerts/instability.rs` | `+35` | `-19` |
| `backend/crates/nucleo/src/services/alerts/manager.rs` | `+63` | `-21` |
| `backend/crates/nucleo/src/services/auth/setup.rs` | `+46` | `-17` |
| `backend/crates/nucleo/src/services/backup/archive.rs` | `+55` | `-6` |
| `backend/crates/nucleo/src/services/backup/copies.rs` | `+59` | `-14` |
| `backend/crates/nucleo/src/services/backup/plan.rs` | `+17` | `-4` |
| `backend/crates/nucleo/src/services/backup/restore.rs` | `+210` | `-24` |
| `backend/crates/nucleo/src/services/backup/schedule.rs` | `+37` | `-19` |
| `backend/crates/nucleo/src/services/backup/service.rs` | `+32` | `-17` |
| `backend/crates/nucleo/src/services/devices/access.rs` | `+85` | `-33` |
| `backend/crates/nucleo/src/services/maintenance/data_pruner.rs` | `+111` | `-69` |
| `backend/crates/nucleo/src/services/maintenance_windows.rs` | `+19` | `-3` |
| `backend/crates/nucleo/src/services/monitoring/checkers/host_resources.rs` | `+25` | `-4` |
| `backend/crates/nucleo/src/services/monitoring/checkers/http.rs` | `+2` | `-1` |
| `backend/crates/nucleo/src/services/monitoring/checkers/ping.rs` | `+21` | `-14` |
| `backend/crates/nucleo/src/services/monitoring/checkers/system_health.rs` | `+1` | `-1` |
| `backend/crates/nucleo/src/services/monitoring/ip_reconciliation.rs` | `+84` | `-48` |
| `backend/crates/nucleo/src/services/monitoring/ping_diagnostics.rs` | `+9` | `-8` |
| `backend/crates/nucleo/src/services/monitoring/presenter.rs` | `+86` | `-58` |
| `backend/crates/nucleo/src/services/monitoring/result_processor.rs` | `+11` | `-5` |
| `backend/crates/nucleo/src/services/monitoring/rollup.rs` | `+7` | `-4` |
| `backend/crates/nucleo/src/services/monitoring/saas.rs` | `+62` | `-38` |
| `backend/crates/nucleo/src/services/network_tools/dns/latency.rs` | `+59` | `-35` |
| `backend/crates/nucleo/src/services/network_tools/mactelnet.rs` | `+1` | `-1` |
| `backend/crates/nucleo/src/services/network_tools/playbook.rs` | `+45` | `-31` |
| `backend/crates/nucleo/src/services/network_tools/speedtest.rs` | `+144` | `-60` |
| `backend/crates/nucleo/src/services/network_tools/traceroute.rs` | `+20` | `-5` |
| `backend/crates/nucleo/src/services/notifications/channels/telegram.rs` | `+15` | `-2` |
| `backend/crates/nucleo/src/services/notifications/outbox.rs` | `+100` | `-33` |
| `backend/crates/nucleo/src/services/notifications/service.rs` | `+8` | `-3` |
| `backend/crates/nucleo/src/services/onboarding.rs` | `+16` | `-5` |
| `backend/crates/nucleo/src/services/probes/agent.rs` | `+55` | `-32` |
| `backend/crates/nucleo/src/services/probes/buffer.rs` | `+34` | `-19` |
| `backend/crates/nucleo/src/services/probes/liveness.rs` | `+25` | `-9` |
| `backend/crates/nucleo/src/services/probes/receiver.rs` | `+39` | `-20` |
| `backend/crates/nucleo/src/services/scheduled_tasks/arguments.rs` | `+2` | `-2` |
| `backend/crates/nucleo/src/services/scheduled_tasks/runner.rs` | `+185` | `-20` |
| `backend/crates/nucleo/src/services/scheduled_tasks/service.rs` | `+55` | `-15` |
| `backend/crates/nucleo/src/services/server_addresses.rs` | `+1` | `-1` |
| `backend/crates/nucleo/src/services/settings/database.rs` | `+71` | `-18` |
| `backend/crates/nucleo/src/services/syslog/app_layer.rs` | `+12` | `-1` |
| `backend/crates/nucleo/src/services/syslog/categorizer.rs` | `+47` | `-6` |
| `backend/crates/nucleo/src/services/syslog/db.rs` | `+2` | `-0` |
| `backend/crates/nucleo/src/services/syslog/hints.rs` | `+87` | `-27` |
| `backend/crates/nucleo/src/services/syslog/listener.rs` | `+60` | `-36` |
| `backend/crates/nucleo/src/services/syslog/matcher.rs` | `+117` | `-30` |
| `backend/crates/nucleo/src/services/syslog/provision.rs` | `+12` | `-29` |
| `backend/crates/nucleo/src/services/syslog/queue.rs` | `+14` | `-4` |
| `backend/crates/nucleo/src/services/syslog/repository.rs` | `+25` | `-12` |
| `backend/crates/nucleo/src/services/syslog/resolver.rs` | `+80` | `-16` |
| `backend/crates/nucleo/src/services/syslog/retention.rs` | `+170` | `-27` |
| `backend/crates/nucleo/src/services/syslog/search.rs` | `+112` | `-16` |
| `backend/crates/nucleo/src/services/syslog/sources.rs` | `+79` | `-19` |
| `backend/crates/nucleo/src/services/syslog/templates.rs` | `+6` | `-6` |
| `backend/crates/nucleo/src/services/syslog/writer.rs` | `+23` | `-9` |
| `backend/crates/nucleo/src/services/telemetry/host_metrics.rs` | `+12` | `-5` |
| `backend/crates/nucleo/src/services/users.rs` | `+145` | `-42` |
| `backend/crates/nucleo/src/services/webpush/client.rs` | `+15` | `-5` |
| `backend/crates/nucleo/src/services/webpush/crypto.rs` | `+27` | `-1` |
| `backend/crates/nucleo/src/services/webpush/delivery.rs` | `+30` | `-12` |
| `backend/crates/nucleo/src/services/webpush/keys.rs` | `+66` | `-52` |
| `backend/crates/nucleo/src/views/logs.rs` | `+15` | `-14` |

</details>

<details>
<summary><strong>4. Servidores — Docker & Ponte de Banco (crates/servers)</strong> — 34 arquivos (<code>+1535</code> / <code>-579</code>)</summary>

| Arquivo | Inserções | Deleções |
| :--- | ---: | ---: |
| `backend/crates/servers/src/agent_host.rs` | `+35` | `-38` |
| `backend/crates/servers/src/controllers/databases.rs` | `+54` | `-24` |
| `backend/crates/servers/src/controllers/docker.rs` | `+12` | `-9` |
| `backend/crates/servers/src/databases/backups.rs` | `+119` | `-26` |
| `backend/crates/servers/src/databases/discovery.rs` | `+114` | `-17` |
| `backend/crates/servers/src/databases/dump.rs` | `+5` | `-2` |
| `backend/crates/servers/src/databases/jobs.rs` | `+89` | `-34` |
| `backend/crates/servers/src/databases/mysql.rs` | `+106` | `-16` |
| `backend/crates/servers/src/databases/postgres.rs` | `+132` | `-76` |
| `backend/crates/servers/src/databases/reach.rs` | `+31` | `-10` |
| `backend/crates/servers/src/databases/restore.rs` | `+52` | `-10` |
| `backend/crates/servers/src/databases/schedule.rs` | `+11` | `-0` |
| `backend/crates/servers/src/databases/sql/order.rs` | `+3` | `-5` |
| `backend/crates/servers/src/databases/sql/splitter.rs` | `+93` | `-31` |
| `backend/crates/servers/src/docker/builds/artifacts.rs` | `+21` | `-1` |
| `backend/crates/servers/src/docker/builds/local.rs` | `+99` | `-31` |
| `backend/crates/servers/src/docker/cli.rs` | `+56` | `-7` |
| `backend/crates/servers/src/docker/compose.rs` | `+8` | `-10` |
| `backend/crates/servers/src/docker/engine.rs` | `+1` | `-1` |
| `backend/crates/servers/src/docker/log_clear.rs` | `+50` | `-5` |
| `backend/crates/servers/src/docker/log_stream.rs` | `+15` | `-6` |
| `backend/crates/servers/src/docker/metrics.rs` | `+1` | `-1` |
| `backend/crates/servers/src/docker/mod.rs` | `+22` | `-3` |
| `backend/crates/servers/src/docker/operations.rs` | `+82` | `-6` |
| `backend/crates/servers/src/docker/protection.rs` | `+28` | `-3` |
| `backend/crates/servers/src/docker/remote_engine.rs` | `+80` | `-96` |
| `backend/crates/servers/src/docker/source.rs` | `+18` | `-7` |
| `backend/crates/servers/src/docker/stacks/deploy.rs` | `+36` | `-30` |
| `backend/crates/servers/src/docker/stacks/git.rs` | `+8` | `-1` |
| `backend/crates/servers/src/docker/stacks/readiness.rs` | `+45` | `-28` |
| `backend/crates/servers/src/docker/stacks/registry.rs` | `+32` | `-1` |
| `backend/crates/servers/src/docker/update.rs` | `+64` | `-26` |
| `backend/crates/servers/src/docker/volume_export.rs` | `+12` | `-17` |
| `backend/crates/servers/src/views/databases.rs` | `+1` | `-1` |

</details>

<details>
<summary><strong>5. Servidores — Container Registry (crates/servers-registry)</strong> — 18 arquivos (<code>+1288</code> / <code>-474</code>)</summary>

| Arquivo | Inserções | Deleções |
| :--- | ---: | ---: |
| `backend/crates/servers-registry/src/api/blobs.rs` | `+53` | `-16` |
| `backend/crates/servers-registry/src/api/manifests.rs` | `+11` | `-5` |
| `backend/crates/servers-registry/src/api/mod.rs` | `+51` | `-14` |
| `backend/crates/servers-registry/src/api/uploads.rs` | `+175` | `-47` |
| `backend/crates/servers-registry/src/archive.rs` | `+36` | `-12` |
| `backend/crates/servers-registry/src/auth.rs` | `+1` | `-0` |
| `backend/crates/servers-registry/src/browse.rs` | `+151` | `-66` |
| `backend/crates/servers-registry/src/catalog.rs` | `+119` | `-48` |
| `backend/crates/servers-registry/src/controllers.rs` | `+8` | `-0` |
| `backend/crates/servers-registry/src/ingest.rs` | `+58` | `-24` |
| `backend/crates/servers-registry/src/retention.rs` | `+59` | `-29` |
| `backend/crates/servers-registry/src/scan/advisory.rs` | `+4` | `-1` |
| `backend/crates/servers-registry/src/scan/mod.rs` | `+151` | `-71` |
| `backend/crates/servers-registry/src/scan/osv.rs` | `+287` | `-97` |
| `backend/crates/servers-registry/src/scan/version.rs` | `+65` | `-29` |
| `backend/crates/servers-registry/src/storage.rs` | `+22` | `-2` |
| `backend/crates/servers-registry/src/tarstream.rs` | `+32` | `-10` |
| `backend/crates/servers-registry/src/tokens.rs` | `+5` | `-3` |

</details>

<details>
<summary><strong>5. Servidores — Deploys (crates/servers)</strong> — 27 arquivos (<code>+1282</code> / <code>-473</code>)</summary>

| Arquivo | Inserções | Deleções |
| :--- | ---: | ---: |
| `backend/crates/servers/src/ai_tools.rs` | `+38` | `-25` |
| `backend/crates/servers/src/controllers/deploy_builds.rs` | `+2` | `-5` |
| `backend/crates/servers/src/controllers/deploys.rs` | `+21` | `-34` |
| `backend/crates/servers/src/deploys/ai_tools.rs` | `+89` | `-38` |
| `backend/crates/servers/src/deploys/applications.rs` | `+23` | `-16` |
| `backend/crates/servers/src/deploys/builds/mod.rs` | `+276` | `-48` |
| `backend/crates/servers/src/deploys/builds/push.rs` | `+7` | `-4` |
| `backend/crates/servers/src/deploys/builds/runner.rs` | `+70` | `-79` |
| `backend/crates/servers/src/deploys/builds/secrets.rs` | `+23` | `-27` |
| `backend/crates/servers/src/deploys/catalog.rs` | `+66` | `-34` |
| `backend/crates/servers/src/deploys/connections.rs` | `+23` | `-3` |
| `backend/crates/servers/src/deploys/console.rs` | `+21` | `-1` |
| `backend/crates/servers/src/deploys/env_versions.rs` | `+38` | `-1` |
| `backend/crates/servers/src/deploys/executor.rs` | `+52` | `-25` |
| `backend/crates/servers/src/deploys/github/app.rs` | `+13` | `-0` |
| `backend/crates/servers/src/deploys/github/compose.rs` | `+17` | `-3` |
| `backend/crates/servers/src/deploys/github/discovery.rs` | `+10` | `-13` |
| `backend/crates/servers/src/deploys/github/mod.rs` | `+15` | `-6` |
| `backend/crates/servers/src/deploys/github/publish.rs` | `+8` | `-3` |
| `backend/crates/servers/src/deploys/images.rs` | `+36` | `-0` |
| `backend/crates/servers/src/deploys/oidc.rs` | `+23` | `-4` |
| `backend/crates/servers/src/deploys/orchestrator.rs` | `+152` | `-66` |
| `backend/crates/servers/src/deploys/readiness.rs` | `+3` | `-2` |
| `backend/crates/servers/src/deploys/runs.rs` | `+189` | `-28` |
| `backend/crates/servers/src/deploys/usage.rs` | `+24` | `-8` |
| `backend/crates/servers/src/deploys/webhook.rs` | `+18` | `-0` |
| `backend/crates/servers/src/deploys/workflow.rs` | `+25` | `-0` |

</details>

<details>
<summary><strong>6. Rede — Discovery, SNMP, Plugins, Topologia & Throughput (crates/network)</strong> — 41 arquivos (<code>+1682</code> / <code>-522</code>)</summary>

| Arquivo | Inserções | Deleções |
| :--- | ---: | ---: |
| `backend/crates/network/src/agent_commands.rs` | `+7` | `-6` |
| `backend/crates/network/src/area_policy.rs` | `+10` | `-1` |
| `backend/crates/network/src/controllers/discovery.rs` | `+1` | `-3` |
| `backend/crates/network/src/controllers/networks.rs` | `+52` | `-21` |
| `backend/crates/network/src/controllers/package_mirror.rs` | `+21` | `-6` |
| `backend/crates/network/src/controllers/port_scan.rs` | `+10` | `-1` |
| `backend/crates/network/src/controllers/snmp.rs` | `+8` | `-1` |
| `backend/crates/network/src/discovery/conflicts.rs` | `+28` | `-24` |
| `backend/crates/network/src/discovery/pipeline.rs` | `+20` | `-1` |
| `backend/crates/network/src/discovery/queue.rs` | `+20` | `-9` |
| `backend/crates/network/src/discovery/remote.rs` | `+12` | `-0` |
| `backend/crates/network/src/discovery/scanners/http.rs` | `+13` | `-3` |
| `backend/crates/network/src/discovery/scanners/names.rs` | `+8` | `-1` |
| `backend/crates/network/src/discovery/service.rs` | `+62` | `-17` |
| `backend/crates/network/src/plugins/ai_tools.rs` | `+27` | `-11` |
| `backend/crates/network/src/plugins/auto_accept.rs` | `+75` | `-0` |
| `backend/crates/network/src/plugins/builtin/mod.rs` | `+19` | `-5` |
| `backend/crates/network/src/plugins/compat.rs` | `+1` | `-2` |
| `backend/crates/network/src/plugins/dedicated_access/mod.rs` | `+15` | `-10` |
| `backend/crates/network/src/plugins/fingerprint.rs` | `+31` | `-11` |
| `backend/crates/network/src/plugins/fleet.rs` | `+39` | `-32` |
| `backend/crates/network/src/plugins/params.rs` | `+1` | `-1` |
| `backend/crates/network/src/plugins/review/mod.rs` | `+10` | `-0` |
| `backend/crates/network/src/plugins/review/static_scan.rs` | `+168` | `-19` |
| `backend/crates/network/src/plugins/runs.rs` | `+59` | `-22` |
| `backend/crates/network/src/plugins/runtime.rs` | `+152` | `-27` |
| `backend/crates/network/src/plugins/service.rs` | `+55` | `-12` |
| `backend/crates/network/src/plugins/transport/agent.rs` | `+23` | `-4` |
| `backend/crates/network/src/plugins/transport/local.rs` | `+47` | `-4` |
| `backend/crates/network/src/plugins/transport/mod.rs` | `+22` | `-0` |
| `backend/crates/network/src/plugins/transport/shell.rs` | `+22` | `-2` |
| `backend/crates/network/src/port_scanner.rs` | `+98` | `-39` |
| `backend/crates/network/src/routines.rs` | `+16` | `-8` |
| `backend/crates/network/src/snmp/checker.rs` | `+18` | `-18` |
| `backend/crates/network/src/snmp/client.rs` | `+30` | `-8` |
| `backend/crates/network/src/snmp/collectors.rs` | `+125` | `-21` |
| `backend/crates/network/src/snmp/service.rs` | `+143` | `-101` |
| `backend/crates/network/src/throughput/btest/codec.rs` | `+17` | `-7` |
| `backend/crates/network/src/throughput/btest/duplex.rs` | `+28` | `-9` |
| `backend/crates/network/src/topology/link_resolver.rs` | `+81` | `-17` |
| `backend/crates/network/src/topology/service.rs` | `+88` | `-38` |

</details>

<details>
<summary><strong>7. Rede (Link Tests) — crates/network-link-tests</strong> — 7 arquivos (<code>+267</code> / <code>-83</code>)</summary>

| Arquivo | Inserções | Deleções |
| :--- | ---: | ---: |
| `backend/crates/network-link-tests/src/health.rs` | `+50` | `-26` |
| `backend/crates/network-link-tests/src/janitor.rs` | `+40` | `-5` |
| `backend/crates/network-link-tests/src/plan.rs` | `+44` | `-20` |
| `backend/crates/network-link-tests/src/repository.rs` | `+37` | `-10` |
| `backend/crates/network-link-tests/src/routines.rs` | `+22` | `-6` |
| `backend/crates/network-link-tests/src/runner.rs` | `+17` | `-7` |
| `backend/crates/network-link-tests/src/service.rs` | `+57` | `-9` |

</details>

<details>
<summary><strong>7. Rede (Wi-Fi) — crates/network-wifi</strong> — 8 arquivos (<code>+356</code> / <code>-136</code>)</summary>

| Arquivo | Inserções | Deleções |
| :--- | ---: | ---: |
| `backend/crates/network-wifi/src/ai_tools.rs` | `+14` | `-10` |
| `backend/crates/network-wifi/src/collector.rs` | `+44` | `-16` |
| `backend/crates/network-wifi/src/live.rs` | `+2` | `-2` |
| `backend/crates/network-wifi/src/roaming/detect.rs` | `+38` | `-11` |
| `backend/crates/network-wifi/src/roaming/mod.rs` | `+123` | `-45` |
| `backend/crates/network-wifi/src/routines.rs` | `+4` | `-7` |
| `backend/crates/network-wifi/src/watch.rs` | `+97` | `-35` |
| `backend/crates/network-wifi/src/watchlist.rs` | `+34` | `-10` |

</details>

<details>
<summary><strong>7. VPN WireGuard — crates/vpn</strong> — 14 arquivos (<code>+898</code> / <code>-409</code>)</summary>

| Arquivo | Inserções | Deleções |
| :--- | ---: | ---: |
| `backend/crates/vpn/src/ai_tools.rs` | `+1` | `-1` |
| `backend/crates/vpn/src/alerts/watcher.rs` | `+79` | `-12` |
| `backend/crates/vpn/src/cidr.rs` | `+12` | `-2` |
| `backend/crates/vpn/src/config_writer.rs` | `+54` | `-21` |
| `backend/crates/vpn/src/controllers/vpn_peers.rs` | `+87` | `-119` |
| `backend/crates/vpn/src/controllers/vpn_servers.rs` | `+3` | `-16` |
| `backend/crates/vpn/src/ip_allocator.rs` | `+62` | `-17` |
| `backend/crates/vpn/src/peer_service.rs` | `+214` | `-55` |
| `backend/crates/vpn/src/peer_status.rs` | `+93` | `-19` |
| `backend/crates/vpn/src/preflight.rs` | `+25` | `-9` |
| `backend/crates/vpn/src/profiles/wg_conf.rs` | `+1` | `-4` |
| `backend/crates/vpn/src/server_service.rs` | `+117` | `-59` |
| `backend/crates/vpn/src/shell_escape.rs` | `+2` | `-9` |
| `backend/crates/vpn/src/traffic_recorder.rs` | `+148` | `-66` |

</details>

<details>
<summary><strong>8. Assistente IA — crates/assistant</strong> — 28 arquivos (<code>+1358</code> / <code>-615</code>)</summary>

| Arquivo | Inserções | Deleções |
| :--- | ---: | ---: |
| `backend/crates/assistant/src/approvals.rs` | `+104` | `-36` |
| `backend/crates/assistant/src/central.rs` | `+3` | `-2` |
| `backend/crates/assistant/src/context_window/mod.rs` | `+33` | `-19` |
| `backend/crates/assistant/src/context_window/sources.rs` | `+20` | `-5` |
| `backend/crates/assistant/src/controllers/ai.rs` | `+77` | `-132` |
| `backend/crates/assistant/src/conversations.rs` | `+38` | `-14` |
| `backend/crates/assistant/src/data_export/mod.rs` | `+63` | `-33` |
| `backend/crates/assistant/src/data_export/repository.rs` | `+12` | `-6` |
| `backend/crates/assistant/src/decider/runtime.rs` | `+13` | `-1` |
| `backend/crates/assistant/src/drivers/mod.rs` | `+96` | `-1` |
| `backend/crates/assistant/src/drivers/openai_compatible.rs` | `+273` | `-162` |
| `backend/crates/assistant/src/harness/agent.rs` | `+94` | `-17` |
| `backend/crates/assistant/src/harness/grounding.rs` | `+83` | `-36` |
| `backend/crates/assistant/src/harness/lean.rs` | `+9` | `-4` |
| `backend/crates/assistant/src/harness/turn.rs` | `+52` | `-2` |
| `backend/crates/assistant/src/local_models.rs` | `+56` | `-19` |
| `backend/crates/assistant/src/local_server.rs` | `+45` | `-10` |
| `backend/crates/assistant/src/mentions.rs` | `+72` | `-26` |
| `backend/crates/assistant/src/ollama.rs` | `+14` | `-9` |
| `backend/crates/assistant/src/opencode.rs` | `+12` | `-16` |
| `backend/crates/assistant/src/openrouter.rs` | `+12` | `-16` |
| `backend/crates/assistant/src/proactive/digest.rs` | `+15` | `-5` |
| `backend/crates/assistant/src/proactive/incident.rs` | `+66` | `-16` |
| `backend/crates/assistant/src/proactive/triage.rs` | `+6` | `-7` |
| `backend/crates/assistant/src/settings.rs` | `+73` | `-9` |
| `backend/crates/assistant/src/web_search/cache.rs` | `+3` | `-5` |
| `backend/crates/assistant/src/web_search/mod.rs` | `+10` | `-5` |
| `backend/crates/assistant/src/web_search/providers/mod.rs` | `+4` | `-2` |

</details>

<details>
<summary><strong>8. MCP — crates/mcp</strong> — 4 arquivos (<code>+96</code> / <code>-25</code>)</summary>

| Arquivo | Inserções | Deleções |
| :--- | ---: | ---: |
| `backend/crates/mcp/src/auth.rs` | `+1` | `-1` |
| `backend/crates/mcp/src/protocol.rs` | `+71` | `-13` |
| `backend/crates/mcp/src/scopes.rs` | `+15` | `-6` |
| `backend/crates/mcp/src/settings.rs` | `+9` | `-5` |

</details>

<details>
<summary><strong>8. Wiring Raiz — backend/src</strong> — 7 arquivos (<code>+160</code> / <code>-34</code>)</summary>

| Arquivo | Inserções | Deleções |
| :--- | ---: | ---: |
| `backend/src/app.rs` | `+3` | `-4` |
| `backend/src/initializers/monitoring.rs` | `+9` | `-2` |
| `backend/src/initializers/process_deps.rs` | `+2` | `-1` |
| `backend/src/initializers/setup.rs` | `+4` | `-0` |
| `backend/src/registries.rs` | `+11` | `-8` |
| `backend/src/spa.rs` | `+63` | `-4` |
| `backend/src/tasks/scheduler_run.rs` | `+68` | `-15` |

</details>

---

## 5. Invariantes Preservadas (`AGENTS.md`) & Notas de Limpeza

### Invariantes Verificadas e Preservadas
- **§3 Independência SQLite / PostgreSQL 18 & Transações de Escrita:**
  - **0** ocorrências de `.begin().await` deferido em produção (validado por `conventions::transacoes`).
  - Todas as migrations alteradas foram editadas diretamente (sistema pré-produção), sem migrations de compatibilidade ou *shims*.
  - Consultas de schema (`table_exists`), ordenação (`NullOrdering::First`), upserts (`OnConflict` + dedup de lotes no OSV) e truncamento de datas (`UTC`) mantêm semântica idêntica em SQLite e PostgreSQL 18.
- **§6 Preservação do `vpn-probe`:**
  - `DEFAULT_VPN_PROBE_TOKEN = "default_vpn_probe_token"` preservado em `vpn_registrar.rs` e `probes/agent.rs`.
  - Comando CLI `vpn_probe_register` intacto.
  - Fallback de execução local em `backend/src/tasks/scheduler_run.rs` (`execute_one`) preservado integralmente.
- **§7 Fronteiras Arquiteturais (ADRs 003, 011, 012, 013, 014, 017, 021, 022, 023):**
  - API segue sem executar `wg` nem `docker` e sem montar `docker.sock` (`conventions::architecture` 100% verde).
  - Ping nativo via socket ICMP `SOCK_DGRAM` (`netmonitor_core::network_tools::icmp_probe`).
  - Agente remoto continua apenas discando para fora; `AGENT_ALLOW` soberano; `DockerCall` enum fechado; ponte de banco apenas encaminha bytes para `AGENT_DATABASE_TARGETS` em quadros binários com janela de crédito na fila de menor prioridade; `PROTOCOL_VERSION = 7` estrito.
  - Plugins Rhai seguem confinados ao sandbox da central sem socket direto; quarentena sem atalhos; `reveals: true` bloqueado fora de `plugins::reveal`; acesso da IA a equipamentos sob `ToolKind::DeviceAccess` com auto-aceite agora também escopado por `user_id`.
  - Registry `/v2` mantém recusa de token pessoal pela borda pública (agora estritamente *fail-closed* via `shared::client_addr`) e build exclusivamente no agente com permissão `build` e segredos via `--secret`.
- **§9 Tempo Real Exclusivamente por SSE:**
  - Nenhum polling introduzido; eventos chegam pelo barramento global (`EventBus`), agora com serialização única (`EncodedEvent`) e detecção de desconexão (`sender.closed()`).
- **§10 / §11 SOLID & DRY:**
  - Reutilização canônica de `duration_to_ms`, `probe_icmp_with_client`, `shared::client_addr`, `shared::runtime::is_production_environment`, `OnceLock<reqwest::Client>`, `OnceLock<Regex>` / cache limitado e remoção de `.unwrap()`/`.expect()` em caminhos de produção.

### Notas de Disco e Limpeza (`backend/target`)
- **Espaço livre medido ao final da Fase 4:**
  - Unidade `D:`: **222,99 GB livres** (730,87 GB usados)
  - Unidade `C:`: **448,46 GB livres** (504,40 GB usados)
- **Segunda rodada:** `cargo clean` ao fim removeu 105,8 GiB de `backend/target` (D: com 214 GB livres); o próximo build recompila do zero.
- **Tamanho de `backend/target` na primeira rodada:** **86,29 GB** (era 67,04 GB antes da rodada, devido aos artefatos `debug` + `test` + `release`).
  - Se desejar liberar espaço após concluir a vistoria e o commit, execute `cargo clean` em `backend/` (ou remova `backend/target/debug` mantendo os binários em `backend/target/release`).
- **Banco de desenvolvimento local:** Como 4 migrations (na segunda rodada, também o índice único parcial de `m20260810_000016`) em `backend/crates/core/src/migrations/` receberam novos índices diretamente (regra pré-produção `AGENTS.md §3`), recrie o banco local de dev na próxima subida para aplicar os novos índices.

---

## 6. Segunda Rodada — Correções Pós-Vistoria

A revisão do diff acima (9 frentes, uma por área) achou bugs de fluxo, condições de corrida que só aparecem no PostgreSQL e quatro duplicações. Esta seção registra o que mudou; **onde ela contradiz as seções 2–3, vale esta**.

**Lição transversal:** `begin_write` só serializa no SQLite (`BEGIN IMMEDIATE`). No PostgreSQL ele não trava nada, então ler-decidir-gravar precisa de `UPDATE` condicional com conferência de `rows_affected`, índice único parcial, `FOR UPDATE SKIP LOCKED` ou releitura dentro da transação.

### 6.1 Agente remoto (`agent-protocol`, `nucleo/src/services/{agent_runtime,agents}`, `core/src/agents`)
- 401 com `AGENT_ENROLL_CODE`: o agente troca o código primeiro e só sobrescreve o token se a troca der certo (antes apagava o token salvo).
- Com o canal caído ou o agente encerrando, o resultado de Monitor/Discovery vai para o outbox (token `session_closed`); só o `Body::Cancel` da central omite a resposta.
- Central: fila de entrada com contrapressão (`send().await` quando cheia, nada é descartado); no fechamento o worker drena até esvaziar, com prazo de 15 s. Resultados vão ao banco em grupos de até 16 ou 500 ms depois do primeiro (`RESULT_FLUSH_WINDOW`).
- Outbox do agente: `push` assíncrono e durável (retorna com o evento em disco), `flush()` no encerramento, confirmação por sequência (`ack(&[seq])`).
- Escritas no socket por `send_bounded` (55 s no agente, 60 s na central); o túnel cancela `stop` quando a entrada fecha depois do EOF remoto.
- Telemetria: recuo de relógio de até 2 min ignorado; recuo maior fecha o balde; base dos contadores descartada depois de lacuna (sem pico falso).
- `AgentSession`: futuro de pedido descartado no meio manda `Cancel` ao agente (`PendingGuard::drop`), para operação ou deploy abortado não seguir rodando.
- `From<RemoteError>`: `Engine` passou de 500 para 400 (`business_rule`); `Engine`, `Disabled` e `Conflict` levam a mensagem remota.

### 6.2 Core
- `device_status`: CAS (`UPDATE … WHERE status=<lido>`, até 3 tentativas), publica só com `rows_affected == 1`.
- `client_addr::is_internal_ip` volta a usar `private_networks` (sem 169.254/16); `sftp` trata `NoSuchFile`/`Failure` como diretório ausente; `secret_store` purga vencidos em todo `put`; `cadence` varre só acima de 1024 entradas e no máximo 1×/h; `health` devolve `unknown` em `JoinError`.
- Telemetria: rollup vazio não abre transação; lote de containers deduplicado por nome (já cortado em 255) antes do `insert_many` — o `ON CONFLICT` do PostgreSQL recusa a mesma linha duas vezes no comando.
- **DRY:** `shared::db_errors::is_unique_violation` (antes duplicado em VPN e Wi-Fi; agora também em `storage/service.rs` e `alerts/manager.rs`); `ai::lookup::device_fields_match` (antes repetido em `mentions.rs`); criação de usuário única em `users::Model::insert_new` / `NewUser` + `hash_password_off_runtime` (antes em 3 lugares).
- `Classifier::categorize_log` devolve `LogVerdict` (`Suggested`/`Unsure`/`Unavailable`): só "sem confiança" congela o padrão por 24 h; falha da chamada volta na próxima passada do backfill.

### 6.3 Núcleo
- Notificações: claim com `FOR UPDATE SKIP LOCKED` (PostgreSQL) e conferência de `rows_affected`; `refresh_claim` renova as linhas reservadas, e só claim sem progresso por 5 min volta para `pending`.
- Alertas: índice único parcial `alert_events_open_rule_scope_unique`; violação tratada como continuação do alerta aberto. O recovery relê e grava cada evento dentro do `begin_write` (não perde `acknowledgedAt`/`silencedUntil`). `acknowledge`/`silence` não reabrem alerta resolvido.
- Result processor: guarda `LastRunAt < finished_at` (estrita). Rollup: purga limitada à marca d'água de id e a `end − (lookback+1)h`. Catálogo SaaS lê só a última latência (`fetch_recent_result_rows`, limite 1). O agente de probe posta sublotes à medida que as checagens terminam; o ProbeBuffer serializa cada item uma vez.
- Syslog: orçamento em linhas (ver §2); nome todo numérico e ponto final recusados no destino; a porta do catálogo não vai para o syslog; cache de capacidade só com resposta do banco; `DENSITY_LIMIT` de volta a 10 mil.
- Backup: export de configuração na ordem de criação; espera de 256 MiB no de configuração e 64 MiB no completo; `RestoreGuard` exclui restauração, cópias e backup completo até o fim do VACUUM; o agendado pula o ciclo, sem notificar, quando a vez está ocupada.
- Tarefas agendadas: reconciliação por `update_many` condicional (`status='running'`) dentro do `begin_write`. Push: DELETE remove inscrição sem dono. `run_digest` exige Admin.

### 6.4 Servidores — Docker e bancos
- A retenção só esquece a linha quando o armazenamento foi removido ou não existe mais; falha ao abrir preserva.
- Progresso: etapas de baixo nível respeitam `PUBLISH_EVERY`; marcos publicam na hora.
- `EphemeralDockerConfig` apagado de forma síncrona no `Drop`; temporário do dump criado `0600`.
- Descoberta pela ponte: `BRIDGE_TIMEOUT = 15 s < OPEN_TIMEOUT`, teto total `BRIDGE_BUDGET = 40 s`.
- Audit Docker só depois do aceite; prazo e audiência do stream de logs conferidos também no `recv`; `Mutex` por projeto podado quando ninguém mais usa; `CACHED_CLIENT` desligado em `cfg(test)`; um `Vec` reaproveitado por `COPY`.

### 6.5 Deploys e registry
- OSV: carga completa em lotes curtos com geração em `created_at`; a geração antiga só é apagada (em blocos de 2000) depois de o zip ser lido sem erro, e erro de leitura aborta antes de apagar; cursor até 1 µs antes do primeiro aviso que falhou; 404/410 pulado e registrado.
- `storage::push_gate`: envios seguram a leitura do arquivo ao vínculo; a coleta só apaga blob com `try_write` (vale dentro de um processo).
- Uploads: guarda que tira a vez do mapa; registro apagado se o temporário já foi consumido. Blobs: 416 só com início ≥ tamanho (e `bytes=-0`); faixa inválida, várias faixas ou blob vazio dão 200 (RFC 9110); sufixo `bytes=-N` atendido.
- Orquestrador: `runs::find_many_without_log` no cancelamento de sequência; o ingest reaproveita o buffer de 256 KiB.

### 6.6 Rede
- SNMP: fallback GETNEXT em `Timeout` (não em `Auth`), host sem GETBULK memorizado por 1 h; `detect_connection` corre o perfil cadastrado junto com os candidatos (janela de 500 ms); apply/toggle esperam a coleta em curso até 10 s antes do 409; `run_now` só consolida monitor ligado e sem probe.
- Plugins: máscara conforme §2; regex de plugin com teto de 1/2 MiB, igual na instalação e na execução; pool SSH chaveado pelo hash das credenciais, 5 min de ociosidade e despejo do menos usado; aceite automático por usuário na tela e no revoke; o static scan confere POST fora de mapa inline.
- Descoberta: `try_reserve`/`bind_run` com `finish` no erro; qualquer run viva bloqueia (local e por probe); conflitos de camada 2 pela última run concluída de cada rede, com erro propagado.
- btest: status periódico também no teste só de envio.

### 6.7 VPN, Wi-Fi e testes de enlace
- `traffic_recorder`: snapshot em memória e, sem assinante SSE local (scheduler em processo próprio), gravação no outbox só quando o quadro muda; `notify_peers_changed` esquece os snapshots; contadores de peers revogados saem da memória.
- `config_writer`: limpeza de temporários órfãos com mais de 60 s.
- **DRY:** `sync_peers` e `sync_peers_loaded` num caminho só, com `begin_write`; `plan.rs` usa `transparent::is_transparent_by`.
- Link tests: `persist` grava as amostras parciais (teto de 120 por fluxo); passada destacada só no laço do servidor (`Cycle::detached`).
- Wi-Fi: com rodada em curso, a ferramenta da IA espera a primeira leitura pelo barramento (sem polling) até 25 s.

### 6.8 Assistente, MCP e raiz
- `retry_on_last` no harness; download de modelo local com nome fixo `sha256-{hex}.partial` e trava; cache do servidor local por URL, esquecido ao salvar; `merge_unmasked`/`with_stored_key` devolvem erro de validação; `run_forever` recebe o futuro de encerramento.

### 6.9 Pendências conhecidas
- ~~**SNMPv3**~~ — **resolvido**: credenciais SNMP viraram perfis cifrados (`snmp_profiles`), o dispositivo aponta para um (`snmp_profile_id`) e coleta, monitores (credencial injetada na hora de rodar), descoberta e identificação resolvem pelo mesmo `devices::snmp_credentials`; tela em Configurações › Rede.
- ~~**Relay do SSE no PostgreSQL**~~ — **resolvido** ([ADR 025](adr/025-varios-processos-lider-e-notify.md)): o cursor saiu; a linha do outbox e o `pg_notify` saem no mesmo commit, e a escuta relê a lacuna depois de uma queda.
- ~~**Snapshot entre processos**~~ — **resolvido** (ADR 025): o snapshot viaja como snapshot (tipo + chave) e entra no cache do outro processo; quem começa a escutar pede `resync`.
- ~~**Tarefas agendadas**~~ — **resolvido** (ADR 025): o laço e a reconciliação de boot só rodam no processo líder. Resta o caso estreito da execução manual num processo não líder durante a troca de líder (registrado no ADR).
- ~~**Rollup e push gate**~~ — **resolvido** (ADR 025): agregação e purga numa transação `REPEATABLE READ` (`begin_snapshot_write`); o ciclo, com o despacho de notificações, só roda no líder.
- ~~**GET de modelos da IA**~~ — **resolvido**: qualquer papel lista pelo que está salvo; outra URL ou chave em claro na query é configurar a IA e exige administrador (`settings::listing_overrides_saved`, 403 para os demais).
- **Banco único** (decisão de 2026-11-25, ADR 020 revisto): produção e desenvolvimento só em PostgreSQL (`docker-compose.dev.yml` para o dev); o boot recusa SQLite fora dos testes (`app::database_allowed`). O SQLite fica só como banco da suíte.
- **Transporte SNMP por thread** (`snmp/client.rs`): o `UdpTransport` num `static` morria com o primeiro runtime tokio que o criou; agora há um por thread do runtime.
- **Testes instáveis sob carga** corrigidos: o amostrador de vazão roda com o relógio do tokio pausado, e a corrida do `BEGIN` deferido usa uma barreira para garantir que as duas transações leiam antes de gravar.
- **Em aberto (registrado):** o script da VPN não provisiona usuário SNMPv3 no equipamento (só comunidade v1/v2c); a lista de perfis SNMP não tem evento SSE próprio (recarrega após cada mudança feita na tela); no SQLite dos testes, `created_at` com `CURRENT_TIMESTAMP` tem formato textual diferente do que o sea-orm grava, o que pode antecipar a poda do outbox no mesmo dia (só na suíte).
