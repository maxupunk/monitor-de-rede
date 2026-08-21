# Roadmap NetMonitor

> Roadmap mestre do NetMonitor. Ele consolida os roadmaps temáticos já entregues, os débitos técnicos pendentes e as próximas frentes de evolução do produto.  
> **Última revisão:** 2026-08-20.

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

- [ ] **SEC-05 — Sanitizar nomes que viram linhas do `wg0.conf`**
  - **Severidade:** 🟠 Alta
  - **Esforço:** Pequeno
  - **Arquivos:** `backend/src/services/vpn/config_builder.rs`, `backend/src/controllers/vpn_peers.rs`
  - **Descrição:** `controllers/devices.rs` valida nome de dispositivo, mas nomes de peer não são validados e `config_builder.rs` interpola `peer.name` em comentário.
  - **Aceite:**
    - Rejeitar `\n`, `\r`, `\t` e caracteres de controle em nomes de peer.
    - Defesa em profundidade no gerador: sanitizar/escapar antes de interpolar em `wg0.conf`.
    - Testes de snapshot com strings maliciosas.

### 3.4 CI/CD

- [ ] **INF-01 — Mover CI para `.github/workflows/` na raiz e completar os jobs**
  - **Severidade:** 🔴 Crítica
  - **Esforço:** Médio
  - **Arquivos:** `backend/.github/workflows/ci.yaml`, `.github/workflows/ci.yaml`
  - **Descrição:** o workflow mora em `backend/.github/`, que o GitHub Actions ignora.
  - **Aceite:**
    - Workflow na raiz com `defaults.run.working-directory: backend`.
    - Jobs: `fmt`, `clippy --all-targets -- -D warnings`, `test` (SQLite e Postgres), `cargo audit` ou `cargo deny`, frontend (`typecheck`, `lint`, `build`), `npm audit` (fail em `high`/`critical`), build de imagem Docker.
    - Substituir `actions-rs/cargo@v1` por action mantida ou `run` direto.
    - Pinar actions por SHA.

---

## 4. Fase 1 — Higiene de frontend e infraestrutura (🟠 Alta / 🟡 Média)

> **Objetivo:** fechar gaps de segurança de superfície, limpar estado sensível na SPA e endurecer o container.

- [ ] **SEC-07 — Decidir e implementar armazenamento seguro do JWT**
  - **Severidade:** 🟠 Alta
  - **Esforço:** Médio
  - **Arquivos:** `frontend/src/services/apiService.ts`, `backend/src/controllers/auth.rs`
  - **Descrição:** JWT ainda mora em `localStorage`. O `roadmap_auditoria_seguranca.md` marca como concluído, mas o código não reflete.
  - **Aceite:**
    - Opção A: migrar para cookie `HttpOnly` + SameSite, com proteção CSRF para mutações.
    - Opção B: manter `localStorage` com CSP estrito e documentar a decisão explicando por que o item foi marcado como concluído.
    - Qualquer que seja a escolha, estado deve estar consistente entre código e documentação.

- [ ] **SEC-08 — Adicionar headers de segurança no servidor estático**
  - **Severidade:** 🔵 Baixa
  - **Esforço:** Pequeno
  - **Arquivos:** `backend/src/spa.rs`
  - **Descrição:** hoje só há `Cache-Control`.
  - **Aceite:**
    - `X-Content-Type-Options: nosniff`
    - `Referrer-Policy: strict-origin-when-cross-origin`
    - `X-Frame-Options: DENY` ou CSP `frame-ancestors 'none'`
    - CSP inicial permissivo o suficiente para não quebrar a SPA, mas sem `unsafe-inline` para scripts quando viável.

- [ ] **SEC-09 — Remover allowlist de scaffold do magic link**
  - **Severidade:** 🟡 Média
  - **Esforço:** Pequeno
  - **Arquivos:** `backend/src/controllers/auth.rs:20-25`
  - **Descrição:** magic link só aceita `@example.com` e `@gmail.com`.
  - **Aceite:**
    - Remover allowlist ou remover o fluxo de magic link se não for usado.
    - Testes ajustados.

- [ ] **SEC-10 — Redigir senha nos derives `Debug` de credenciais**
  - **Severidade:** 🟡 Média
  - **Esforço:** Pequeno
  - **Arquivos:** `backend/src/models/users.rs`
  - **Descrição:** `LoginParams` e `RegisterParams` derivam `Debug` e vazam senha.
  - **Aceite:**
    - Implementar `Debug` manual ocultando a senha.
    - Teste de convenção ou unitário garantindo que `format!("{:?}", params)` não contém a senha.

- [ ] **INF-02 — Supervisão do watcher e hardening do container**
  - **Severidade:** 🟡 Média
  - **Esforço:** Médio
  - **Arquivos:** `docker/entrypoint.sh`, `Dockerfile`, `docker-compose.yml`
  - **Descrição:** watcher inicia com `&` e vira órfão; imagens não pinadas por digest.
  - **Aceite:**
    - Adotar `tini` como PID 1 ou supervisão mínima que reinicie o watcher se morrer.
    - Healthcheck do container considerar vida do watcher.
    - Pinar imagens base por digest (`node`, `rust`, `debian`).
    - Adicionar `security_opt: [no-new-privileges:true]`, `read_only` com `tmpfs` e rotação de logs no compose.

- [ ] **Frontend — Limpar estado sensível do visualizador VPN**
  - **Severidade:** 🟡 Média
  - **Esforço:** Pequeno
  - **Arquivos:** `frontend/src/stores/vpn.ts`, `frontend/src/components/VpnScriptViewer.vue`
  - **Descrição:** `lastArtifact` (contém chave privada e QR code) nunca é limpo; QR code renderizado via `v-html` sem DOMPurify.
  - **Aceite:**
    - `lastArtifact` é limpo ao fechar o diálogo.
    - Adicionar DOMPurify no SVG do QR code (defesa em profundidade).

- [ ] **Frontend — Melhorias no `apiService.ts`**
  - **Severidade:** 🔵 Baixa
  - **Esforço:** Pequeno
  - **Arquivos:** `frontend/src/services/apiService.ts`
  - **Descrição:** requisições sem `AbortSignal.timeout`; `?redirect=` perdido no 401 global; erro de rede não distingue de erro de API.
  - **Aceite:**
    - `AbortSignal.timeout(15000)` por requisição.
    - Preservar e redirecionar com `?redirect=` no 401.
    - Tipo de erro distinto para rede vs API.

---

## 5. Fase 2 — Qualidade, testes e performance (🟡 Média)

> **Objetivo:** pagar débitos de manutenibilidade e performance antes de novas features.

- [ ] **FE-03 — Infraestrutura de testes no frontend**
  - **Severidade:** 🟠 Alta
  - **Esforço:** Médio
  - **Arquivos:** `frontend/package.json`, `frontend/tests/`
  - **Descrição:** não há script `test` nem runner configurado.
  - **Aceite:**
    - Adicionar `vitest`, `@vue/test-utils`, `jsdom`.
    - Script `"test"` no `package.json`.
    - Cobrir `utils/formatters.ts`, `composables/` e stores puras como primeira onda.
    - CI executa `npm run test`.

- [ ] **QUA-01 — Decompor páginas monolíticas que cresceram**
  - **Severidade:** 🟡 Média
  - **Esforço:** Médio
  - **Arquivos:** `frontend/src/pages/DashboardPage.vue`, `AlertsPage.vue`, `SettingsPage.vue`
  - **Descrição:** páginas com 600–950 linhas.
  - **Aceite:**
    - Extrair widgets, tabelas, filtros e formulários para componentes próprios.
    - `typecheck`, `lint` e `build` passam sem novo `as any`.

- [ ] **QUA-02 — Cachear `reqwest::Client` no `HttpChecker`**
  - **Severidade:** 🟡 Média
  - **Esforço:** Pequeno
  - **Arquivos:** `backend/src/services/monitoring/checkers/http.rs`
  - **Aceite:**
    - Dois clientes cacheados via `OnceLock` (um com verificação TLS padrão, outro configurável se necessário).
    - Testes de monitor HTTP continuam passando.

- [ ] **QUA-03 — Timeout em `lookup_host` (ping e SNMP)**
  - **Severidade:** 🟡 Média
  - **Esforço:** Pequeno
  - **Arquivos:** `backend/src/services/monitoring/checkers/ping.rs`, `backend/src/services/snmp/client.rs`
  - **Aceite:**
    - `tokio::time::timeout` com limite adequado (ex.: 5 s).
    - Tradução de `Elapsed` para `CheckResult` `unknown` com mensagem clara.

- [ ] **QUA-04 — Reduzir N+1 na coleta SNMP**
  - **Severidade:** 🟡 Média
  - **Esforço:** Médio
  - **Arquivos:** `backend/src/services/snmp/service.rs`
  - **Aceite:**
    - Buscar métricas anteriores em uma query por grupo de interfaces.
    - Transações curtas; não prender writer do SQLite durante avaliação de alertas/topologia.
    - Testes de SNMP passam.

- [ ] **QUA-05 — Honrar ou remover `monitors.timeout_seconds`**
  - **Severidade:** 🟡 Média
  - **Esforço:** Pequeno
  - **Arquivos:** `backend/src/models/monitors.rs`, `backend/src/controllers/monitors.rs`, `backend/src/services/monitoring/runner.rs`
  - **Aceite:**
    - Se honrar: scheduler usa `timeout_seconds` com mínimo sensato.
    - Se remover: retirar coluna do formulário, API e DTOs.
    - Nenhum contrato morto no schema.

- [ ] **QUA-06 — Melhorar buffer offline do probe**
  - **Severidade:** 🟡 Média
  - **Esforço:** Médio
  - **Arquivos:** `backend/src/services/probes/buffer.rs`
  - **Descrição:** buffer cresce sem teto, reescrita O(n²), escrita não atômica.
  - **Aceite:**
    - Tamanho máximo configurável.
    - Escrita atômica (tmp+rename).
    - Testes de crash/recuperação.

- [ ] **BE-03 / BE-05 — Finalizar builders VPN e DTOs tipados**
  - **Severidade:** 🟡 Média
  - **Esforço:** Médio
  - **Arquivos:** `backend/src/services/vpn/profiles/*.rs`, `backend/src/controllers/{devices.rs,monitors.rs,logs.rs}`
  - **Aceite:**
    - Todos os valores interpolados em scripts/configurações passam por sanitização centralizada.
    - Reduzir `serde_json::json!` nas respostas; migrar para DTOs `ts-rs`.
    - Testes de snapshot e convenção cobrem os novos DTOs.

---

## 6. Fase 3 — Evolução de produto (planejado)

> **Objetivo:** novas capacidades que aumentam o valor do produto. Só entram depois que as Fases 0 e 1 estiverem estáveis.

- [ ] **Janelas de manutenção**
  - Permitir silenciar alertas/notificações por site/dispositivo em janela agendada.
  - Evita flaps falsos durante reboots e alterações programadas.

- [ ] **Baseline móvel e alertas por desvio**
  - Alertar quando latência, perda de pacotes ou outra métrica se desvia da média das últimas 24 h, em vez de usar apenas threshold fixo.

- [ ] **Rollup/ agregação de métricas**
  - Tabelas `monitor_results` e `metrics` são append-only e crescem sem limite.
  - Agregações horárias/diárias para responder “este link é estável?” em 24 h / 7 d / 30 d.

- [ ] **Trilha de auditoria**
  - Registrar quem alterou cada recurso (usuário, timestamp, mudança).
  - Pré-requisito para compliance e debug de incidentes.

- [ ] **Correlação temporal de alertas em cascata**
  - Além da inibição por `parent_id`, detectar que muitos dispositivos caíram no mesmo segundo e sugerir causa raiz comum.

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
npm --prefix frontend run typecheck
npm --prefix frontend run format
npm --prefix frontend run lint
npm --prefix frontend run build
# após adicionar testes:
npm --prefix frontend run test
```

### Segurança / DevOps (quando aplicável)

```bash
# Backend
cargo audit
# ou
cargo deny check

# Frontend
npm --prefix frontend audit

# Docker
docker build -t netmonitor:check .
```

> Regra de ouro: `cargo test` regenera os bindings `ts-rs`. Sempre rode `npm --prefix frontend run format` **depois** de `cargo test`, nunca antes.

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
