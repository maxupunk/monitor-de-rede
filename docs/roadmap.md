# Roadmap NetMonitor

> Roadmap mestre do NetMonitor. Ele consolida os roadmaps temáticos já entregues, os débitos técnicos pendentes e as próximas frentes de evolução do produto.  
> **Última revisão:** 2026-08-25.

## 1. Visão e critérios de priorização

O NetMonitor monitora redes residenciais e de pequenas empresas: descoberta de dispositivos, checagens de disponibilidade, métricas, alertas inteligentes, syslog e VPN — tudo num único container Docker. O roadmap segue a ordem:

1. **Segurança e confiança operacional** primeiro: segredos, CI, autenticação e permissões.
2. **Qualidade e manutenibilidade** em seguida: testes, performance, decomposição e dever de casa técnico.
3. **Evolução de produto** por fim: novas features e melhorias de UX.

Cada item carrega severidade, esforço, responsável sugerido e critério de aceite. Itens marcados com `[x]` já estão no código e validados; itens `[ ]` são próximas entregas.

---

## 2. Roadmaps temáticos já concluídos

| Roadmap | Tema | Estado | Link |
| :--- | :--- | :--- | :--- |
| Servidor NetMonitor como dispositivo | Representar o próprio servidor como dispositivo de primeira classe | 🟢 Concluído | [`roadmap_servidor_netmonitor_como_dispositivo.md`](roadmap_servidor_netmonitor_como_dispositivo.md) |
| Ajustes de dispositivo, regras e abertura de monitores | Correções de UX e consistência no detalhe do dispositivo/monitor | 🟢 Concluído | [`roadmap_ajustes_dispositivo_e_monitores.md`](roadmap_ajustes_dispositivo_e_monitores.md) |
| Alertas inteligentes e detecção de instabilidade | Histerese de resolução, flapping, classificação de problema, higiene de notificações | 🟢 Concluído | [`roadmap_monitoramento_inteligente.md`](roadmap_monitoramento_inteligente.md) |
| Syslog nativo | Recepção, consulta, live tail, alertas por padrão de log e ativação automática | 🟢 Concluído | [`roadmap_syslog_nativo.md`](roadmap_syslog_nativo.md) |
| Auditoria de segurança e qualidade | Fases 1 e 2 (sessão, autenticação, scheduler, dados) concluídas; Fases 0 e 3 pendentes | 🟡 Parcial | [`roadmap_auditoria_seguranca.md`](roadmap_auditoria_seguranca.md) |

> **Nota:** o `roadmap_auditoria_seguranca.md` documenta itens de segurança que ainda não foram totalmente resolvidos. Eles foram reavaliados e reordenados neste roadmap mestre.

---

## 3. Fase 0 — Segurança e confiança operacional (🔴 Crítico)

> **Objetivo:** eliminar vetores de ataque que hoje são triviais em instalações padrão e garantir que os gates de qualidade realmente rodem.

### 3.1 Segredos e credenciais

- [x] **SEC-01 — Remover defaults públicos de `JWT_SECRET` e `ENCRYPTION_KEY`**
  - **Severidade:** 🔴 Crítica
  - **Esforço:** Pequeno
  - **Arquivos:** `.env.example`, `docker-compose.yml`, `backend/config/{production,development,test}.yaml`, `backend/src/app.rs`
  - **Implementado:**
    - `.env.example` e `docker-compose.yml` não expõem mais defaults públicos.
    - `backend/config/production.yaml` exige `JWT_SECRET` no ambiente.
    - `backend/src/app.rs` falha o boot em produção se `JWT_SECRET` estiver ausente.
    - `development.yaml` e `test.yaml` mantêm defaults locais devidamente documentados.

- [x] **SEC-02 — Remover chave privada WireGuard do repositório e rotacionar**
  - **Severidade:** 🔴 Crítica
  - **Esforço:** Pequeno
  - **Arquivos:** `backend/tmp/wireguard/wg0.conf`, `.dockerignore`, `.gitignore`
  - **Implementado:**
    - Arquivo removido do índice do Git (`git rm --cached`).
    - `.gitignore` e `.dockerignore` passam a excluir `backend/tmp/` e `**/.env`.
    - ⚠️ **Atenção:** a chave que estava no arquivo deve ser considerada comprometida. Em instalações reais, rotacione a chave privada do servidor WireGuard.

- [x] **SEC-06 — `POST /api/probes` deve receber token cru, não hash**
  - **Severidade:** 🟠 Alta
  - **Esforço:** Pequeno
  - **Arquivos:** `backend/src/controllers/probes.rs`, `backend/src/dtos/resources.rs`, `backend/tests/requests/scheduler_probes_lifecycle.rs`
  - **Implementado:**
    - `ProbeInput.token_hash` foi renomeado para `ProbeInput.token`.
    - O controller gera um UUID como token quando nenhum é enviado, calcula `sha256` no servidor e grava `token_hash`.
    - A resposta de criação devolve o token cru ao cliente; listagens/consultas devolvem `token: null`.
    - Teste `criacao_de_probe_recebe_token_cru_e_retorna_token` valida o fluxo.

### 3.2 Probes e resultados

- [x] **SEC-04 — Vincular resultados de probe ao probe autenticado**
  - **Severidade:** 🟠 Alta
  - **Esforço:** Médio
  - **Arquivos:** `backend/src/services/probes/receiver.rs`, `backend/tests/requests/scheduler_probes_lifecycle.rs`
  - **Implementado:**
    - `receiver.rs` consulta `monitors::Entity` e `discovery_runs::Entity` para garantir que o resultado reportado pertence ao `probe_id` autenticado.
    - Resultados de monitor/run alheios são descartados com `warn!`; o restante do lote continua processado.
    - Teste `probe_nao_reporta_resultado_de_monitor_alheio` cobre a rejeição.

### 3.3 VPN e configuração

- [x] **SEC-05 — Sanitizar nomes que viram linhas do `wg0.conf`**
  - **Severidade:** 🟠 Alta
  - **Esforço:** Pequeno
  - **Arquivos:** `backend/src/services/vpn/peer_name.rs`, `backend/src/services/vpn/config_builder.rs`, `backend/src/controllers/vpn_peers.rs`, `backend/tests/requests/vpn_orchestration.rs`
  - **Implementado:**
    - Criado `services::vpn::peer_name` com `validate` (rejeita controles/\t e nome vazio) e `sanitize_for_config` (substitui controles por `_`).
    - `POST /api/vpn/peers` e `PATCH /api/vpn/peers/:id` validam o nome antes de criar/renomear (422 para controles, 400 para nome vazio).
    - `config_builder.rs` sanitiza o nome antes de interpolar no comentário do peer.
    - Testes de snapshot e teste de integração `nome_de_peer_com_controle_e_rejeitado_na_criacao_e_no_rename` cobrem o fluxo.

### 3.4 CI/CD

- [x] **INF-01 — Mover CI para `.github/workflows/` na raiz e completar os jobs**
  - **Severidade:** 🔴 Crítica
  - **Esforço:** Médio
  - **Arquivos:** `.github/workflows/ci.yaml`
  - **Implementado:**
    - Workflow movido para a raiz com `defaults.run.working-directory: backend`.
    - Jobs: `fmt`, `clippy`, testes SQLite e Postgres, `cargo audit`, frontend (`typecheck`, `lint`, `build`), `npm audit` (`--audit-level=high`), build Docker.
    - `actions-rs/cargo@v1` substituído por `run` direto.
    - Todas as actions pinadas por SHA.

---

## 4. Fase 1 — Higiene de frontend e infraestrutura (🟠 Alta / 🟡 Média)

> **Objetivo:** fechar gaps de segurança de superfície, limpar estado sensível na SPA e endurecer o container.

- [x] **SEC-07 — Decidir e implementar armazenamento seguro do JWT**
  - **Severidade:** 🟠 Alta
  - **Esforço:** Médio
  - **Arquivos:** `frontend/src/services/apiService.ts`, `backend/src/spa.rs`
  - **Decisão:** Optou-se pela **Opção B** — manter o JWT no `localStorage` e endurecer o CSP.
  - **Implementado:**
    - `backend/src/spa.rs` passa a enviar `Content-Security-Policy` com `script-src 'self'`, `frame-ancestors 'none'`, `X-Frame-Options: DENY` e demais headers de segurança.
    - `frontend/src/services/apiService.ts` documenta a decisão e o vínculo com o CSP.
    - A migração para cookie `HttpOnly` permanece como evolução futura caso o modelo de ameaça exija (ex.: ambientes com extensões de navegador não confiáveis).

- [x] **SEC-08 — Adicionar headers de segurança no servidor estático**
  - **Severidade:** 🔵 Baixa
  - **Esforço:** Pequeno
  - **Arquivos:** `backend/src/spa.rs`
  - **Implementado:**
    - `X-Content-Type-Options: nosniff`
    - `Referrer-Policy: strict-origin-when-cross-origin`
    - `X-Frame-Options: DENY`
    - `Content-Security-Policy` com `script-src 'self'`, `frame-ancestors 'none'` e demais diretivas.
    - Teste `headers_de_seguranca_estao_configurados`.

- [x] **SEC-09 — Remover allowlist de scaffold do magic link**
  - **Severidade:** 🟡 Média
  - **Esforço:** Pequeno
  - **Arquivos:** `backend/src/controllers/auth.rs`
  - **Implementado:**
    - Regex de allowlist removida; magic link aceita qualquer e-mail válido.
    - Teste `magic_link_aceita_email_valido_sem_expor_existencia` ajustado.

- [x] **SEC-10 — Redigir senha nos derives `Debug` de credenciais**
  - **Severidade:** 🟡 Média
  - **Esforço:** Pequeno
  - **Arquivos:** `backend/src/models/users.rs`
  - **Implementado:**
    - `LoginParams` e `RegisterParams` implementam `Debug` manual, substituindo a senha por `[REDACTED]`.
    - Testes unitários garantem que `format!("{:?}", params)` não contém a senha.

- [x] **INF-02 — Supervisão do watcher e hardening do container**
  - **Severidade:** 🟡 Média
  - **Esforço:** Médio
  - **Arquivos:** `docker/entrypoint.sh`, `docker/wireguard-watcher.sh`, `docker/healthcheck.sh`, `Dockerfile`, `docker-compose.yml`
  - **Implementado:**
    - `tini` como PID 1; entrypoint supervisiona e reinicia o watcher se morrer.
    - Healthcheck via `docker/healthcheck.sh` considera API e heartbeat do watcher.
    - Imagens base (`node`, `rust`, `debian`, `postgres` no CI) pinadas por digest.
    - Compose com `read_only: true`, `tmpfs`, `security_opt: [no-new-privileges:true]` e rotação de logs.

- [x] **Frontend — Limpar estado sensível do visualizador VPN**
  - **Severidade:** 🟡 Média
  - **Esforço:** Pequeno
  - **Arquivos:** `frontend/src/components/VpnScriptViewer.vue`
  - **Implementado:**
    - `vpnStore.lastArtifact` é limpo ao fechar o diálogo (`watch` de `modelValue`).
    - Adicionada dependência `dompurify` e sanitização do SVG do QR code antes do `v-html`.

- [x] **Frontend — Melhorias no `apiService.ts`**
  - **Severidade:** 🔵 Baixa
  - **Esforço:** Pequeno
  - **Arquivos:** `frontend/src/services/apiService.ts`
  - **Implementado:**
    - `AbortSignal.timeout(15000)` em todas as requisições.
    - Redirecionamento no 401 preserva `?redirect=`.
    - `ApiError` e `NetworkError` como tipos distintos de erro.

---

## 5. Fase 2 — Qualidade, testes e performance (🟡 Média)

> **Objetivo:** pagar débitos de manutenibilidade e performance antes de novas features.

- [x] **FE-03 — Infraestrutura de testes no frontend** 🟢 Concluído
  - **Severidade:** 🟠 Alta
  - **Esforço:** Médio
  - **Arquivos:** `frontend/package.json`, `frontend/vitest.config.ts`, `frontend/tests/`
  - **Descrição:** não havia script `test` nem runner configurado.
  - **Implementado:**
    - Adicionados `vitest`, `@vue/test-utils`, `jsdom` e `@types/jsdom`.
    - Criado `frontend/vitest.config.ts` com ambiente `jsdom` e alias `@/`.
    - Script `"test": "vitest run"` no `package.json`; o script `"format"` passou a incluir `tests/`.
    - Migrados os testes existentes (`formatters.test.ts`, `ndjson.test.ts`) para a sintaxe do Vitest.
    - Adicionados testes para `utils/formatters.ts`, `composables/useMonitorDetail.ts`, `composables/useInfiniteList.ts`, `composables/useInfiniteCursor.ts` e as stores puras `preferences`, `alerts` e `dashboard`.
    - CI executa `npm run test` entre `lint` e `build`.

- [x] **QUA-01 — Decompor páginas monolíticas que cresceram** 🟢 Concluído
  - **Severidade:** 🟡 Média
  - **Esforço:** Médio
  - **Arquivos:** `frontend/src/pages/DashboardPage.vue`, `AlertsPage.vue`, `SettingsPage.vue`
  - **Descrição:** páginas com 600–950 linhas.
  - **Implementado:**
    - `DashboardPage.vue`: extraídos `StatCardsWidget`, `ActiveAlertsWidget`, `EventsFeedWidget` e `NetworkMonitorsWidget` para `frontend/src/components/dashboard/`.
    - `AlertsPage.vue`: extraídas as quatro abas (`ActiveAlertsTab`, `ResolvedAlertsTab`, `AlertRulesTab`, `AlertHistoryTab`) para `frontend/src/components/alerts/`.
    - `SettingsPage.vue`: extraídos `PreferencesCard`, `ServerAddressesCard`, `DashboardSyncCard`, `NotificationsCard`, `OnboardingCard` e `BackupCard` para `frontend/src/components/settings/`.
    - Páginas mantêm apenas orquestração, diálogos e feedback; componentes encapsulam template, estilo e chamadas às stores.
    - `typecheck`, `lint`, `build` e `test` passam sem novo `as any`.

- [x] **QUA-02 — Cachear `reqwest::Client` no `HttpChecker`**
  - **Severidade:** 🟡 Média
  - **Esforço:** Pequeno
  - **Arquivos:** `backend/src/services/monitoring/checkers/http.rs`
  - **Implementado:**
    - Dois clientes `reqwest` cacheados via `OnceLock<Result<Client, reqwest::Error>>`: um com verificação TLS padrão e outro com `danger_accept_invalid_certs(true)`.
    - O timeout continua sendo aplicado por requisição (`RequestBuilder::timeout`), preservando o comportamento anterior.
    - Testes unitários adicionados para cache e para checagens HTTP 200/404 com servidor local.
  - **Aceite:**
    - Dois clientes cacheados via `OnceLock` (um com verificação TLS padrão, outro configurável se necessário).
    - Testes de monitor HTTP continuam passando.

- [x] **QUA-03 — Timeout em `lookup_host` (ping e SNMP)**
  - **Severidade:** 🟡 Média
  - **Esforço:** Pequeno
  - **Arquivos:** `backend/src/services/monitoring/checkers/ping.rs`, `backend/src/services/snmp/client.rs`, `backend/src/services/monitoring/checkers/snmp.rs`
  - **Implementado:**
    - `tokio::time::timeout` de 5 s em `lookup_host` no ping (`resolve_host`) e no cliente SNMP (`resolve_target`).
    - Timeout de DNS no ping traduzido para `CheckResult` com `status: unknown` e mensagem clara.
    - Timeout de DNS no SNMP propagado como `SnmpError::Timeout` e mapeado para `CheckResult` `unknown` no checker SNMP.
    - Testes unitários cobrem IP literal, mapeamento de timeout para `unknown` e erros DNS/ rede permanecendo `down`.

- [x] **QUA-04 — Reduzir N+1 na coleta SNMP** 🟢 Concluído
  - **Severidade:** 🟡 Média
  - **Esforço:** Médio
  - **Arquivos:** `backend/src/services/snmp/service.rs`, `backend/tests/requests/snmp_collection_integration.rs`
  - **Implementado:**
    - Interfaces já conhecidas são carregadas de uma vez (`device_interfaces::Entity::find()`) antes do loop de sincronização.
    - `sync_interface` recebe `Option<&device_interfaces::Model>` para evitar SELECT por interface.
    - Métricas anteriores de tráfego são buscadas em lote por `latest_metrics_for_interfaces`.
    - Métricas de tráfego e sistema são acumuladas em `Vec<PendingMetric>` e gravadas com `metrics::Entity::insert_many`.
    - Testes de SNMP passam.

- [x] **QUA-05 — Honrar `monitors.timeout_seconds` na execução** 🟢 Concluído
  - **Severidade:** 🟡 Média
  - **Esforço:** Pequeno
  - **Arquivos:** `backend/src/services/monitoring/execution_guard.rs`, `backend/src/services/monitoring/scheduler/monitor_executor.rs`, `backend/src/services/monitoring/presenter.rs`, `frontend/src/stores/monitors.ts`, `frontend/src/components/monitors/MonitorDetailView.vue`
  - **Implementado:**
    - Adicionado `effective_timeout_seconds(timeout_seconds, interval_seconds)` que aplica mínimo de 1 s e máximo de `interval - 1`.
    - `monitor_executor.rs` usa `effective_timeout_seconds(monitor.timeout_seconds, monitor.interval_seconds)` para o timeout da execução.
    - `MonitorPresentation` passa a expor `timeout_seconds` e o frontend mantém o campo no tipo `Monitor` e no `emptyMonitor`.
    - Decisão: honrar a coluna, não removê-la.

- [x] **QUA-06 — Melhorar buffer offline do probe** 🟢 Concluído
  - **Severidade:** 🟡 Média
  - **Esforço:** Médio
  - **Arquivos:** `backend/src/services/probes/buffer.rs`
  - **Descrição:** buffer cresce sem teto, reescrita O(n²), escrita não atômica.
  - **Implementado:**
    - Limites configuráveis por número de itens (`PROBE_BUFFER_MAX_RESULTS`, padrão 10.000) e por bytes (`PROBE_BUFFER_MAX_BYTES`, padrão 50 MB).
    - Escrita atômica com arquivo temporário + `rename`.
    - Ao atingir o teto de bytes, deduplica por `monitor_id` mantendo o resultado mais recente de cada monitor; se ainda exceder, trunca os itens mais antigos.
    - Testes cobrem limite de itens, limite de bytes, deduplicação, truncamento por idade, escrita atômica e recuperação após crash.

- [x] **QUA-07 — Diagnosticar ICMP filtrado com confirmação TCP** 🟢 Concluído
  - **Severidade:** 🟡 Média
  - **Esforço:** Médio
  - **Arquivos:** `backend/src/services/monitoring/ping_diagnostics.rs`, `backend/src/services/network_tools/tcp_probe.rs`, catálogo de alertas e ADR 003.
  - **Implementado:**
    - Ping permanece primário e a confirmação TCP só ocorre depois de todas as retentativas com perda total.
    - Portas candidatas limitadas a três, priorizadas por monitores TCP da mesma origem e discovery recente, sem varredura ampla.
    - Respostas TCP `open` ou `closed` mantêm o dispositivo em `warning` e geram o problema `icmp_filtered`; resultados silenciosos ou inalcançáveis continuam `down` e inconclusivos.
    - Configuração `_diagnostics` é transitória e acompanha tarefas remotas sem alterar a configuração persistida.
    - Regra global “ICMP filtrado ou desativado” provisionada de forma idempotente, sem duplicar alertas genéricos de perda de pacotes.

- [x] **QUA-08 — Limitar memória nos históricos e no SQLite de produção** 🟢 Concluído
  - **Severidade:** 🔴 Crítica
  - **Esforço:** Médio
  - **Arquivos:** `docker-compose.yml`, `backend/config/production.yaml`, `backend/src/services/monitoring/{metrics_repository.rs,rollup.rs,timeseries.rs,bandwidth_latency.rs,heatmap.rs,uptime.rs}`, `backend/src/services/{snmp,topology}/service.rs`, `backend/src/services/events/relay.rs`, `backend/src/services/backup/service.rs`, `backend/src/controllers/{dns.rs,events.rs}` e testes de integração.
  - **Implementado:**
    - Busca do último contador SNMP por par `(interface, série)` com lookup indexado e `LIMIT 1`, sem carregar ou ordenar todo o histórico.
    - Pool SQLite de produção alinhado ao desenvolvimento em uma conexão por padrão; PostgreSQL pode elevar `DB_MAX_CONNECTIONS` explicitamente.
    - Séries temporais, uptime, heatmap, distribuição de eventos e correlação banda/latência processados por stream e acumuladores de cardinalidade fixa.
    - Rollup feito por agregação SQL, em janelas de no máximo sete dias por ciclo e lotes de 500 inserts; origens local/probe mistas não violam mais a chave do bucket.
    - Relay do outbox limitado a 500 eventos por passagem, histórico DNS limitado a 720 pontos por série e amostra de mediana limitada a 2.048 valores.
    - Restore SQLite executa checkpoint, `VACUUM` e `PRAGMA optimize` após confirmar a transação, liberando páginas deixadas por históricos apagados.
    - Testes cobrem cardinalidade do último valor, desempate por `id`, backlog do relay, rollup com origem mista e limites do histórico DNS.

- [x] **BE-03 / BE-05 — Finalizar builders VPN e DTOs tipados**
  - **Severidade:** 🟡 Média
  - **Esforço:** Médio
  - **Arquivos:** `backend/src/services/vpn/profiles/*.rs`, `backend/src/controllers/{devices.rs,monitors.rs,logs.rs}`
  - **Aceite:**
    - Todos os valores interpolados em scripts/configurações passam por sanitização centralizada.
    - Reduzir `serde_json::json!` nas respostas; migrar para DTOs `ts-rs`.
    - Testes de snapshot e convenção cobrem os novos DTOs.

- [x] **ARQ-01 — Unificar plataformas com Adapter Pattern** 🟢 Concluído
  - **Severidade:** 🟠 Alta
  - **Esforço:** Grande
  - **Arquivos:** `backend/src/services/devices/adapters/`, `backend/src/services/devices/systems.rs`, `backend/src/services/{syslog,discovery,vpn}/`, `frontend/src/stores/{operatingSystems,vpn}.ts`, `frontend/src/components/`, `backend/tests/conventions/device_adapters.rs`, `docs/adr/009-device-adapters.md`
  - **Implementado:**
    - Registro único de `DeviceAdapter` para RouterOS, OpenWrt, Ubiquiti, Linux, Windows, mobile e fallback genérico.
    - Identificação por SNMP/SSH/cadastro, meios de acesso e classificação de discovery dependem do contrato, sem listas paralelas.
    - Configuração e dialeto Syslog encapsulados em `SyslogConfigurationAdapter`.
    - Geradores WireGuard existentes preservados como adapters especializados e ligados ao registro principal.
    - Frontend deriva perfis, rótulos, ícones, QR Code e seleção inicial da API.
    - Validação de MAC-Telnet no backend e testes unitários/de convenção impedem regressão arquitetural.

---

## 6. Fase 3 — Evolução de produto (planejado)

> **Objetivo:** novas capacidades que aumentam o valor do produto. Só entram depois que as Fases 0 e 1 estiverem estáveis.

- [x] **Janelas de manutenção** 🟢 Concluído
  - **Severidade:** 🟡 Média
  - **Esforço:** Médio
  - **Arquivos:** `backend/src/services/maintenance_windows.rs`, `backend/src/controllers/maintenance_windows.rs`, `backend/src/views/maintenance_windows.rs`, `backend/src/dtos/resources.rs`, `backend/src/services/notifications/{policy,outbox}.rs`, `backend/migration/src/m20260821_000001_maintenance_windows.rs`, `frontend/src/stores/maintenanceWindows.ts`, `frontend/src/components/MaintenanceWindowDialog.vue`, `frontend/src/pages/MaintenanceWindowsPage.vue`, `frontend/src/layouts/DefaultLayout.vue`, `frontend/src/router/index.ts`, `backend/tests/requests/maintenance_windows.rs`
  - **Implementado:**
    - Nova tabela `maintenance_windows` com `site_id` e/ou `device_id`, `starts_at`, `ends_at`, `name`, `description` e `created_by`.
    - CRUD REST sob `/api/maintenance-windows` com validações de intervalo e existência do alvo.
    - Hierarquia respeitada: janela no site cobre dispositivos daquele site; janela no device cobre só ele.
    - Integração no despachante de notificações: alertas ainda são criados, mas a linha do `notification_outbox` nasce `suppressed` com `suppress_reason = maintenance`.
    - Novo motivo `SuppressReason::Maintenance` na política pura de notificações.
    - Página de gerenciamento no frontend com diálogo de criação/edição, tabela responsiva e menu lateral.
    - Testes de integração cobrindo CRUD, validações e supressão por janela no dispositivo e no site.

- [x] **Baseline móvel e alertas por desvio** 🟢 Concluído
  - **Severidade:** 🟡 Média
  - **Esforço:** Médio
  - **Arquivos:** `backend/src/services/alerts/baseline.rs`, `backend/src/services/alerts/{fields.rs,manager.rs}`, `backend/src/services/alerts/datasets/monitor_result.rs`, `backend/src/services/alerts/catalog/templates.rs`, `backend/src/dtos/alerts.rs`, `frontend/src/utils/alertPresentation.ts`, `backend/tests/requests/baseline_alerts.rs`
  - **Implementado:**
    - Service `baseline::MonitorBaseline` calcula baseline por monitor a partir de `monitor_results_hourly` (últimas 48 horas, ignorando buckets com poucas amostras), com cache em memória por 1 hora.
    - Seis novos campos no vocabulário de alertas: `LATENCY_BASELINE_MS`, `LATENCY_DEVIATION_PERCENT`, `PACKET_LOSS_BASELINE_PERCENT`, `PACKET_LOSS_DEVIATION_PERCENT`, `UPTIME_BASELINE_PERCENT`, `UPTIME_DEVIATION_PERCENT`.
    - Dataset `monitor_result` enriquece resultados de ping e monitor genérico com baseline de latência, perda e uptime; campos nulos quando não há dados suficientes.
    - Três templates de catálogo adicionados: desvio de latência (>50% acima da baseline), perda acima da baseline e queda de uptime abaixo da baseline.
    - Rótulos em português para os novos campos no frontend (`alertPresentation.ts`).
    - Testes unitários do baseline e testes de integração cobrindo regra de desvio por latência, perda e uptime.

- [x] **Rollup/ agregação de métricas** 🟢 Concluído
  - **Severidade:** 🟡 Média
  - **Esforço:** Médio
  - **Arquivos:** `backend/migration/src/m20260821_000002_monitor_results_hourly.rs`, `backend/src/models/_entities/monitor_results_hourly.rs`, `backend/src/models/monitor_results_hourly.rs`, `backend/src/services/monitoring/{rollup.rs,uptime.rs}`, `backend/src/services/monitoring/scheduler/{cadence.rs,maintenance_runner.rs}`, `backend/src/tasks/scheduler_run.rs`, `backend/src/dtos/monitors.rs`, `backend/src/controllers/monitors.rs`, `frontend/src/stores/monitors.ts`, `frontend/src/components/devices/tabs/DeviceOverviewTab.vue`, `frontend/src/bindings/MonitorUptimeResponse.ts`, `backend/tests/requests/devices_monitors_crud.rs`
  - **Implementado:**
    - Nova tabela `monitor_results_hourly` com buckets de 1 hora (`monitor_id`, `bucket`, `total_checks`, `up_checks`, `down_checks`, `unknown_checks`, latências agregadas e timestamps de extremo).
    - Service `rollup::rollup_monitor_results` que agrupa resultados brutos por `(monitor_id, bucket)`, persiste os buckets e opcionalmente apaga o bruto antigo via `ROLLUP_DELETE_BRUTO_AFTER_HOURS`.
    - Job `rollup_monitor_results_if_due` no scheduler, executado a cada 1 hora (configurável via `ROLLUP_INTERVAL_SECONDS`); fecha apenas buckets completos, nunca a hora em curso.
    - Service `uptime::uptime_for_monitor` que soma buckets fechados e adiciona o bucket parcial da hora atual a partir de `monitor_results`.
    - Endpoint `GET /api/monitors/:id/uptime?hours=N` (padrão 24, máximo 720) com DTO `MonitorUptimeResponse` exportado para TypeScript.
    - Card “Estabilidade dos Monitores (24h)” na aba Visão Geral do dispositivo, com barra de progresso e contagem de checagens/up/down.
    - Testes de integração cobrindo o endpoint de uptime e o rollup end-to-end.

- [x] **Trilha de auditoria** 🟢 Concluído
  - **Severidade:** 🟡 Média
  - **Esforço:** Médio
  - **Arquivos:** `backend/migration/src/m20260821_000003_audit_logs.rs`, `backend/src/models/_entities/audit_logs.rs`, `backend/src/models/audit_logs.rs`, `backend/src/services/audit.rs`, `backend/src/views/audit.rs`, `backend/src/controllers/audit.rs`, `backend/src/app.rs`, `frontend/src/stores/audit.ts`, `frontend/src/pages/AuditPage.vue`, `frontend/src/router/index.ts`, `frontend/src/layouts/DefaultLayout.vue`, `backend/tests/requests/audit.rs`
  - **Implementado:**
    - Nova tabela `audit_logs` (`user_id`, `action`, `resource_type`, `resource_id`, `resource_label`, `description`, `changes`, `ip_address`, `user_agent`, `created_at`) com índices em `created_at`, `user_id`, `(resource_type, resource_id)` e `action`.
    - Service `services::audit` com `AuditAction`, `ResourceType`, `AuditActor`, `AuditEntryInput`, `AuditFilters` e gravação isolada (falhas de auditoria nunca quebram a operação principal).
    - Controller `GET /api/audit-logs` restrito a administradores, com filtros (`userId`, `resourceType`, `resourceId`, `action`, `from`, `to`) e `PaginationMeta`.
    - Auditoria integrada nos controllers de `auth`, `devices`, `monitors`, `sites`, `networks`, `users`, `probes`, `vpn_peers`, `maintenance_windows` e `alerts` (regras), registrando criação, alteração, exclusão e login/logout com diff opcional.
    - Página frontend `/audit` (apenas admin) com tabela paginada, filtros, expansão de detalhes (IP, user-agent e diff `old`/`new`) e navegação por páginas.
    - Testes unitários do service e testes de integração cobrindo acesso restrito, listagem, filtros e paginação.

- [x] **Correlação temporal de eventos e causa raiz automática (RCA)** 🟢 Concluído
  - **Severidade:** 🟡 Média
  - **Esforço:** Médio
  - **Arquivos:** `backend/src/services/alerts/correlation.rs`, `backend/src/controllers/alerts.rs`, `backend/src/services/alerts/mod.rs`, `frontend/src/stores/alerts.ts`, `frontend/src/components/alerts/AlertCorrelationDialog.vue`, `frontend/src/components/alerts/ActiveAlertsTab.vue`, `frontend/src/pages/AlertsPage.vue`, `backend/tests/requests/alert_correlation.rs`, `frontend/tests/stores/alerts.spec.ts`
  - **Implementado:**
    - Motor de Root Cause Analysis (RCA) e Grafo de Dependências Topológico (`DependencyGraph`) em `services::alerts::correlation` combinando hierarquia declarada (`parent_id`), enlaces descobertos/manuais (`device_links`) e agrupamento por sub-rede (`network_id`).
    - Algoritmo de scoring multivariável: prioridade por papel de infraestrutura (`role_weight`), raio de alcance/impacto a jusante (BFS downstream), precedência temporal e penalidade por falha a montante.
    - Categorização causal em 8 classes (`Gateway`, `Router`, `Switch`, `Firewall`, `VpnTunnel`, `IspLink`, `SiteOutage`, `IsolatedDevice`) com métrica de confiança estatística (0 a 100%).
    - Síntese diagnóstica em linguagem natural (ex: *"17 dispositivos ficaram inacessíveis após `192.168.1.1` (Gateway Principal) parar de responder — causa provável: Gateway da Rede"*).
    - Endpoint global `GET /api/alerts/root-cause-analysis` agrupando incidentes correlacionados ativos em clusters (`IncidentCluster`) para visão consolidada.
    - Endpoint detalhado `GET /api/alerts/:id/correlation` com cadeia de dependência percorrida (`dependencyChain`), lista completa de equipamentos impactados (`impactedDevices`) e contagem.
    - Banner de Diagnóstico RCA em tempo real no topo da Central de Alertas (`AlertsPage.vue`) e diálogo interativo `AlertCorrelationDialog.vue` com chips de categoria/confiança, citação diagnóstica, visualizador de cadeia topológica e lista de nós afetados.
    - Testes unitários do algoritmo em Rust e TypeScript/Vitest, e testes de requisição cobrindo cascata de roteadores/switches, eventos isolados e agrupamento de incidentes ativos.

- [x] **PWA & Notificações Web Push em Segundo Plano** 🟢 Concluído
  - **Severidade:** 🟡 Média
  - **Esforço:** Médio
  - **Arquivos:** `backend/migration/src/m20260821_000004_push_subscriptions.rs`, `backend/src/models/_entities/push_subscriptions.rs`, `backend/src/models/push_subscriptions.rs`, `backend/src/services/webpush/{crypto.rs,keys.rs,client.rs,mod.rs}`, `backend/src/controllers/push.rs`, `backend/src/services/notifications/channels/webpush.rs`, `frontend/src/sw.ts`, `frontend/vite.config.ts`, `frontend/index.html`, `frontend/src/services/pushService.ts`, `frontend/src/composables/useNotifications.ts`, `frontend/src/components/settings/NotificationsCard.vue`, `backend/tests/requests/push.rs`
  - **Implementado:**
    - Suporte a Web Push conforme RFC 8030, RFC 8188 / RFC 8291 (AES-128-GCM) e RFC 8292 (VAPID / ES256) em Rust puro.
    - Nova tabela `push_subscriptions` e gerenciamento inteligente com rotação e geração zero-config de chaves VAPID.
    - Controlador REST sob `/api/push` (`/vapid-public-key`, `/status`, `/subscriptions`, `/test`).
    - Integração de `WebPushChannel` ao despachante do outbox de notificações com expurgo automático de subscrições expiradas (404/410 Gone).
    - Service Worker customizado (`src/sw.ts`) com Workbox precache (`injectManifest`), ouvintes nativos `push` e `notificationclick`.
- [x] **Monitoramento de Latência SaaS e Mapa de Calor Horário (Item 2.2.2)** 🟢 Concluído
  - **Severidade:** 🟡 Média
  - **Esforço:** Médio
  - **Arquivos:** `backend/src/dtos/saas.rs`, `backend/src/services/monitoring/{saas.rs,heatmap.rs}`, `backend/src/controllers/monitors.rs`, `backend/tests/requests/saas_monitoring.rs`, `frontend/src/bindings/{SaasPreset.ts,HourlyHeatmapResponse.ts,...}`, `frontend/src/stores/{monitors.ts,dashboard.ts}`, `frontend/src/components/monitors/SaasPresetsDialog.vue`, `frontend/src/components/widgets/SaasLatencyHeatmapWidget.vue`, `frontend/src/pages/{MonitorsPage.vue,DashboardPage.vue}`, `frontend/src/components/monitors/MonitorDetailView.vue`
  - **Implementado:**
    - Catálogo curado de presets SaaS (Google, Cloudflare, Microsoft 365, GitHub, Netflix, AWS, Zoom, WhatsApp) com alvos estáveis em ICMP (Ping) e HTTP HEAD (ultraleve) e thresholds automáticos de aviso/crítico.
    - Endpoints REST sob `/api/monitors`: `GET /saas/presets`, `POST /saas/provision` (provisionamento idempotente com 1-clique ou lote) e `GET /hourly-heatmap` (agregação de matriz 24h x dias via `monitor_results_hourly` + hora parcial em andamento).
    - Modal de catálogo no frontend (`SaasPresetsDialog.vue`) com filtros por categoria, busca, indicação de monitores já ativos e ações de provisionamento em massa.
    - Widget de Heatmap Horário de Latência (`SaasLatencyHeatmapWidget.vue`) com grade cromática (24h x dias), identificação automática de horários de pico (`peakHour`) e melhor horário (`bestHour`), barra consolidada de 24h e integração no Dashboard customizável e na visualização detalhada do monitor.
    - Testes unitários e testes de integração cobrindo catálogo, provisionamento, reuso de monitores e cálculo de heatmap.

- [x] **Fluxo unificado de ativação e isolamento de logs por dispositivo** 🟢 Concluído
  - **Severidade:** 🟠 Alta
  - **Esforço:** Pequeno
  - **Arquivos:** `frontend/src/components/DeviceDialog.vue`, `frontend/src/pages/{LogsPage.vue,DeviceDetailPage.vue}`, `frontend/src/components/devices/tabs/{DeviceLogsTab.vue,DeviceOverviewTab.vue}`, `frontend/src/composables/useInfiniteCursor.ts`, `frontend/tests/composables/useInfiniteCursor.spec.ts`, `backend/tests/{requests/syslog_api.rs,conventions/tela_unificada.rs}`
  - **Implementado:**
    - Ativação automática de syslog oferecida no cadastro, na edição e na aba Logs do dispositivo, sempre com um snapshot imutável do alvo.
    - Aba de logs protegida por escopo visual e descarte de respostas assíncronas obsoletas, impedindo que registros de um dispositivo apareçam em outro.
    - Resumo de tráfego por interface reposicionado antes da estabilidade dos monitores na Visão Geral.

- [x] **Prioridade visual para monitores ativos no dispositivo** 🟢 Concluído
  - A aba Monitores de `/devices/{id}` lista os ativos primeiro sem alterar a store compartilhada nem a ordem da página global.

- [x] **Identificação assistida e fluxo único de ativação de syslog** 🟢 Concluído
  - O formulário sonda automaticamente um IP completo, mantendo Sistema e Forma de acesso em modo automático com a conclusão e o motivo visíveis.
  - Fabricante e modelo só são preenchidos quando a descoberta ou a ENTITY-MIB fornecem evidência; valores informados pelo operador nunca são sobrescritos.
  - O cadastro oferece “Salvar e ativar logs”, e o endereço detectado do NetMonitor não é mais descartado quando ainda não existe como entrada nomeada.
  - Os campos automáticos exibem diretamente a conclusão efetiva sem congelá-la como declaração manual, e respostas atrasadas são descartadas por IP e sessão.
  - A ativação usa um alvo imutável do dispositivo recém-salvo e um único autocomplete para sugestão, catálogo e endereço livre.
- [x] **Rede Docker host e porta configurável** 🟢 Concluído
  - `docker-compose.host.yml` remove publicação/NAT e compartilha diretamente a rede do host.
  - `APP_PORT` define a porta real da API/SPA; `APP_EXTERNAL_PORT` continua sendo apenas a publicação no modo bridge.
  - `SYSLOG_LISTEN_PORT` define os listeners e, no modo host, é anunciado automaticamente aos equipamentos.
  - O modo é selecionado no `.env` por `COMPOSE_FILE`, com o mesmo separador em Windows e Linux.
  - O README documenta ativação, portas, endereço usado pelos equipamentos, firewall, validação, retorno à bridge e o catálogo dos recursos disponíveis.

- [x] **Gerenciador e monitoramento Docker ponta a ponta** 🟢 Concluído
  - **Arquivos:** `backend/src/services/docker/`, `backend/src/controllers/docker.rs`, `backend/src/{dtos,views}/docker.rs`, `frontend/src/pages/docker/`, `frontend/src/{services,stores}/docker.ts`, `docker-compose.yml`, `docker/entrypoint.sh`, ADR 010.
  - **Implementado:**
    - Cliente nativo da Docker Engine com timeouts, erros sanitizados e nenhum uso do CLI `docker`.
    - Visão geral da Engine e métricas de CPU, memória, rede, I/O e PIDs coletadas concorrentemente com cache curto.
    - Inventário, inspeção, logs e ciclo de vida de containers; volumes com exportação `.tar.gz` em streaming; conexão e remoção de redes; inspeção, remoção e prune de imagens.
    - Valores sensíveis de ambiente são ocultados no backend antes de atravessar a API.
    - Leitura para perfis autenticados; controle da Engine e exportação de dados apenas para administradores, com trilha de auditoria.
    - Interface responsiva com visão geral, containers, volumes, redes e imagens, estados de indisponibilidade e confirmação de ações destrutivas.
    - Testes unitários Rust, contratos HTTP e testes Vitest do service/store.

- [x] **Docker em tempo real, agrupamento e resumo no dashboard** 🟢 Concluído
  - **Arquivos:** `backend/src/services/docker/{metrics,realtime}.rs`, `frontend/src/pages/docker/`, `frontend/src/components/dashboard/DockerSummaryCard.vue`, `frontend/src/stores/{docker,events}.ts`.
  - **Implementado:**
    - Um único coletor no backend distribui inventário, métricas e estado por SSE para todas as abas; não há polling periódico no navegador.
    - A coleta só roda com assinantes SSE, usa eventos efêmeros sem inflar o `event_outbox` e publica snapshot imediato após ações administrativas.
    - Containers separados por projeto Compose, com totais de CPU, memória e estado em cada grupo.
    - Histórico agregado de CPU/RAM na visão Docker e resumo compacto no dashboard, renderizado somente quando a Engine está disponível.
    - Diálogo com download e limpeza real de logs `json-file`, conexão/desconexão de redes com proteção da última rede e contraste revisado.

- [x] **Coerência das métricas de recursos** 🟢 Concluído
  - **Arquivos:** `backend/src/services/{docker,monitoring,snmp}/`, `frontend/src/components/widgets/ResourceUsageWidget.vue`, `frontend/src/utils/{formatters,resourceMetrics}.ts`.
  - **Implementado:**
    - CPU e working set de memória Docker alinhados às fórmulas do CLI, com sinalização de snapshots parciais e invalidação de cache após ações.
    - Cache recuperável descontado em cgroup v1/v2 e em agentes UCD-SNMP, preferindo `memSysAvail` quando disponível.
    - Widgets de CPU/RAM alimentados apenas por histórico e SSE reais, com load average e capacidade medidos; removidas amostras sintéticas e capacidades fixas.
    - Unidades binárias e decimais explicitadas e tamanhos de imagem separados entre uso em disco e tamanho de conteúdo.

- [x] **RAM apresentada prioritariamente como quantidade** 🟢 Concluído
  - **Arquivos:** `backend/src/services/monitoring/presenter.rs`, `frontend/src/components/`, `frontend/src/pages/docker/`, `frontend/src/utils/`.
  - **Implementado:**
    - Cards, tabelas, gráficos, sparklines e eventos exibem memória usada e capacidade em bytes com unidade legível.
    - O percentual permanece apenas como contexto secundário e como base interna para cores e limiares de alerta.
    - Monitores SNMP publicam quantidade no `gaugeMetric` e preservam `usagePercent`/`totalBytes` como metadados auxiliares.

- [x] **Remoção de referências e caminhos de migração sem uso** 🟢 Concluído
  - Removidos o importador de segredos, o fallback de variável de cifra e os documentos dedicados à transição de backend.
  - A documentação descreve apenas a arquitetura e os contratos atuais.

---

## 7. Matriz obrigatória de validação

Toda entrega deve passar por esta matriz antes de ser considerada concluída.

### Backend

```bash
cd backend
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

### Frontend

```bash
pnpm --prefix frontend run typecheck
pnpm --prefix frontend run format
pnpm --prefix frontend run lint
pnpm --prefix frontend run build
# após adicionar testes:
pnpm --prefix frontend run test
```

### Segurança / DevOps (quando aplicável)

```bash
# Backend
cargo audit
# ou
cargo deny check

# Frontend
pnpm --prefix frontend audit

# Docker
docker build -t netmonitor:check .
```

> Regra de ouro: `cargo test` regenera os bindings `ts-rs`. Sempre rode `pnpm --prefix frontend run format` **depois** de `cargo test`, nunca antes.

---

## 8. Como usar este roadmap

1. Itens da **Fase 0** não podem ser postergados; são pré-requisitos para qualquer release em produção.
2. Itens da **Fase 1** devem acompanhar ou seguir imediatamente a Fase 0.
3. Itens da **Fase 2** podem ser paralelizados, mas não devem ser iniciados antes da Fase 0 estar completa.
4. Itens da **Fase 3** são planejados; a prioridade entre eles é decidida conforme feedback de uso e capacidade do time.
5. Ao concluir um item, marque `[x]`, atualize este arquivo e, se necessário, o `docs/debitos_tecnicos.md`.

---

## 9. Referências

- [`docs/debitos_tecnicos.md`](debitos_tecnicos.md) — catálogo detalhado de débitos, com arquivo/linha e severidade.
- [`docs/arquitetura.md`](arquitetura.md) — descrição do sistema como ele é hoje.
- [`docs/diretrizes_testes.md`](diretrizes_testes.md) — padrões de teste do projeto.
- [`AGENTS.md`](../AGENTS.md) — diretrizes obrigatórias para agentes IA trabalhando no repositório.
