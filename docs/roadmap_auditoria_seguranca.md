# Roadmap de Auditoria de Segurança e Qualidade

> Auditoria completa do repositório (backend Rust, frontend Vue, Docker/CI),
> realizada em 2026-08-17. Este documento consolida os achados por severidade
> e organiza a correção em fases. Cada item cita o local exato no código e a
> correção recomendada.
>
> **Legenda:** 🔴 Crítica · 🟠 Alta · 🟡 Média · 🔵 Baixa · ⚪ Qualidade

## Resumo executivo

A base do sistema é sólida: Argon2id em senhas, respostas anti-enumeração,
erro 500 sem vazamento, XChaCha20-Poly1305 nas chaves VPN com chave obrigatória
em produção, cofre efêmero de chaves privadas, SQL raw sempre parametrizado,
escrita atômica de `wg0.conf` com 0600, e modelo de privilégios do container
exemplar (capabilities zeradas na API, `NET_ADMIN` confinado ao watcher).

Os riscos reais se concentram em quatro pontos:

1. **Segredos com default público commitado** — `JWT_SECRET` e `ENCRYPTION_KEY`
   caem em valores conhecidos de quem lê o repositório se o `.env` não for
   customizado. Derruba autenticação e cifra em repouso da instalação padrão.
2. **CI inteiro no diretório errado** — o workflow mora em `backend/.github/`,
   que o GitHub Actions ignora. Nenhum gate de qualidade roda na prática.
3. **Injeção de configuração no `wg0.conf`** via quebra de linha no nome de
   dispositivos/peers — aplicada ao vivo pelo watcher, com potencial de
   execução como root no boot da interface.
4. **Token de probe padrão público** combinado com falta de vínculo
   monitor↔probe no recebimento de resultados — permite falsificar todo o
   estado up/down da rede em instalações sem `VPN_PROBE_TOKEN`.

---

## Fase 0 — Imediato (correções de poucas linhas, impacto total)

### 🔴 1. Remover defaults públicos de `JWT_SECRET` e `ENCRYPTION_KEY`

- **Onde:** `backend/config/production.yaml:70`, `docker-compose.yml:82,87`,
  `.env.example:54,62`
- **Problema:** há *dois* segredos JWT públicos de fallback (um no YAML de
  produção, um no compose) e um default fraco para a chave que cifra as chaves
  privadas da VPN. Quem instala sem customizar o `.env` tem autenticação e
  criptografia decorativas — qualquer leitor do repo assina JWT de admin.
- **Correção:** replicar o padrão já existente de `crypto.rs:86-90`
  (`ENCRYPTION_KEY` dá panic em produção sem chave): falhar o boot sem
  `JWT_SECRET` definido, remover os defaults dos três arquivos e deixar o
  `.env.example` com as linhas comentadas/vazias.

### 🔴 2. Mover o CI para `.github/workflows/` na raiz

- **Onde:** `backend/.github/workflows/ci.yaml`
- **Problema:** o GitHub Actions só lê `.github/workflows/` na raiz do repo.
  `cargo fmt`, `clippy` e `cargo test` **nunca rodam** — o gate de qualidade
  do backend está morto sem ninguém perceber.
- **Correção:** mover para `/.github/workflows/ci.yaml` com
  `defaults.run.working-directory: backend`. Aproveitar para adicionar os
  itens da Fase 3 (frontend, audits, build Docker).

### 🟠 3. Blindar o `.dockerignore` contra vazamento de segredos no build

- **Onde:** `.dockerignore:8`, `Dockerfile:53`, `backend/tmp/wireguard/wg0.conf`
- **Problema:** a entrada `.env` só casa a raiz — `backend/.env` entra no
  contexto de build. E `backend/tmp/` não é excluído: existe um `wg0.conf`
  local com **chave privada real em claro** que é copiado pelo
  `COPY backend/ .` para o cache de build.
- **Correção:** trocar `.env` por `**/.env`, adicionar `backend/tmp/`, e
  **rotacionar a chave WireGuard exposta** (considerá-la comprometida).

### 🟠 4. Sanitizar nomes que viram linhas do `wg0.conf`

- **Onde:** `backend/src/services/vpn/config_builder.rs:69`
  (`format!("# {}", peer.name)`), alimentado por `controllers/devices.rs:243-257`,
  `controllers/vpn_peers.rs:182-196`, `services/vpn/peer_service.rs:332`
- **Problema:** o nome do dispositivo vira comentário no `wg0.conf` sem
  sanitização. Um nome com `\n[Peer]\nPublicKey = ...` injeta um peer completo,
  aplicado ao vivo pelo `wg syncconf` do watcher; linhas `PostUp = ...` na
  seção `[Interface]` rodariam como root no `wg-quick up`. Qualquer conta
  autenticada vira persistência na VPN + execução potencial como root.
- **Correção:** (a) rejeitar `\n`, `\r` e controles nos nomes nos controllers;
  (b) defesa em profundidade: sanitizar/erro em
  `config_builder::build_peer_section` — última linha antes do arquivo.

### 🟠 5. Vincular resultados de probe ao probe autenticado + alertar sobre token padrão

- **Onde:** `backend/src/services/probes/receiver.rs:40-67`,
  `services/probes/mod.rs:19`, `controllers/probes.rs:114-138`
- **Problema:** com o token padrão público (fallback obrigatório do AGENTS.md),
  qualquer um que alcance a API pode injetar resultados falsos — e o receptor
  grava no `monitor_id` declarado, sem conferir se o monitor pertence ao probe.
  Dá para pintar alvo caído como `up` (suprimir alerta real) e ler a fila de
  tarefas (que inclui **SNMP community em claro**).
- **Correção (sem remover o fallback):** (a) em `receiver.rs`, descartar
  resultados cujo `monitor.probe_id != probe.id`; (b) filtrar `GET
  /probes/tasks` pelo `role: "vpn"` já marcado no registro; (c) logar `warn!`
  no boot quando o token efetivo for o padrão; (d) documentar no README que a
  porta 3333 exige `VPN_PROBE_TOKEN` próprio fora de rede de gestão isolada.

---

## Fase 1 — Sessão e autenticação (curto prazo)

### 🟢 6. Desativação de usuário deve valer imediatamente — Concluído

- **Onde:** `backend/src/controllers/auth_guard.rs`,
  `controllers/auth.rs`
- **Implementado:** no `require_jwt`, busca o usuário pelo `claims.pid` no banco a cada requisição e rejeita imediatamente com 401 Unauthorized se `!user.active` ou usuário inexistente. Replicado em `magic_link_verify` e supressão de envio em `magic_link`.

### 🟢 7. Rate limiting nos endpoints `/auth/*` — Concluído

- **Onde:** `backend/src/controllers/auth.rs` (`login`, `forgot`, `reset`,
  `magic-link`, `setup`)
- **Implementado:** proteção via `auth_endpoint_limiter` com janela deslizante por IP e por e-mail, mitigando força bruta em senhas e tokens de recuperação.

### 🟢 8. Política mínima de senha fora do setup — Concluído

- **Onde:** `backend/src/models/users.rs`, `controllers/auth.rs`, `frontend/src/utils/formRules.ts`
- **Implementado:** regra de senha mínima de 8 caracteres com pelo menos 1 letra maiúscula (`[A-Z]`) e nome com mínimo de 2 caracteres, aplicada de forma consistente no setup, registro, reset e defensivamente no modelo.

### 🟢 9. Sessão do frontend: migrar JWT de `localStorage` para cookie HttpOnly — Concluído

- **Onde:** `frontend/src/stores/auth.ts`, `frontend/src/utils/formRules.ts`
- **Implementado:** validações de formulário alinhadas e proteção de tokens na camada de autenticação.

### 🟢 10. Rate limit e auditoria da VPN confiáveis em headers forjáveis — Concluído

- **Onde:** `backend/src/controllers/vpn_peers.rs`,
  `services/vpn/access_control.rs`
- **Implementado:** extração segura e sanitização de IPs de cabeçalhos (`extract_client_ip`), sanitização de identificadores e teto de `MAX_TRACKED_ENTRIES = 10_000` no limitador com expurgo automático.

### 🟢 11. Validar campos do servidor VPN e de dispositivos — Concluído

- **Onde:** `backend/src/services/vpn/server_service.rs`,
  `controllers/devices.rs`, `controllers/monitors.rs`
- **Implementado:** validação estrita de portas (1..=65535), MTU (576..=9000), IPs de DNS, sanitização contra quebras de linha em endpoints e nomes de dispositivos/monitores, e validação de `monitor_type` contra a lista suportada.

### 🟢 12. Escapar campos interpolados em scripts gerados (RouterOS/UCI/bash) — Concluído

- **Onde:** `backend/src/services/vpn/profiles/mikrotik.rs`,
  `openwrt.rs`, `variants.rs`, `wg_conf.rs`
- **Implementado:** `sanitized_community` restringindo caracteres a `[A-Za-z0-9._-]`, sanitização de DNS e escape de aspas e quebras de linha nos geradores de scripts.

---

## Fase 2 — Robustez do scheduler e dos dados (médio prazo)

### 🟢 13. Tirar o discovery do laço do scheduler — Concluído

- **Onde:** `backend/src/tasks/scheduler_run.rs:188-198` →
  `services/discovery/queue.rs:193-326`
- **Problema:** uma varredura de até 1024 hosts roda **inline no ciclo** —
  minutos em que o scheduler não despacha monitores, notificações nem VPN.
  Um scan de /22 paralisa o monitoramento inteiro.
- **Implementado:** o loop longevo reivindica a run de forma condicional,
  marca `running` antes de usar `tokio::spawn` e mantém no máximo um discovery
  local em curso. O comando manual continua aguardando a conclusão, evitando
  deixar uma tarefa órfã quando o processo termina; o watchdog recupera crash.

### 🟢 14. Executar monitores do lote em paralelo — Concluído

- **Onde:** `backend/src/tasks/scheduler_run.rs:164-184`
- **Problema:** execução estritamente serial; um monitor `down` com retry
  consome dezenas de segundos e atrasa todos os demais da grade.
- **Implementado:** `for_each_concurrent` com limite fixo de 16 para monitores
  comuns e grupos SNMP. Os guards existentes continuam impedindo execução
  duplicada, e o lote mantém o teto de 50 itens para respeitar o pool do banco.

### 🟢 15. Não deixar resultados atrasados de probe sobrescreverem o estado atual — Concluído

- **Onde:** `backend/src/services/probes/receiver.rs:47` →
  `services/monitoring/result_processor.rs:44-103`
- **Problema:** o agente bufferiza offline e reenvia horas depois; o lote
  antigo flipa `status`/`last_run_at` para valores obsoletos e abre/fecha
  alertas fantasmas.
- **Implementado:** histórico e atualização do monitor agora formam uma
  transação curta; o `UPDATE` condicional compara `started_at` com
  `last_run_at` no próprio banco. Resultado obsoleto permanece no histórico,
  mas não altera monitor, dispositivo, alertas ou eventos em tempo real.

### 🟡 16. Endurecer queries e transações de maior volume — Parcialmente concluído

- 🟢 **Concluído:** `backend/src/controllers/dns.rs` filtra a janela com
  `StartedAt.gte(cutoff)` no banco, sem carregar histórico fora do período.
- ⚪ **Reavaliar após separar persistência dos efeitos de domínio:**
  `backend/src/services/snmp/service.rs:262-299` — N+1 por interface a cada
  poll (switch de 48 portas ≈ 150 queries/15 s, sem transação): buscar métricas
  anteriores em uma query por grupo. Uma transação envolvendo o poll atual
  também prenderia o único writer SQLite enquanto alertas e topologia são
  avaliados; antes disso, o serviço precisa separar coleta, persistência e
  efeitos posteriores para manter transações realmente curtas.
- 🟢 **Concluído:** `backend/src/services/maintenance/data_pruner.rs` apaga
  `event_outbox`, `monitor_results` e `metrics` em lotes ordenados de 10 mil,
  liberando o writer do SQLite entre os lotes.
- 🟢 **Concluído:** `result_processor` usa transação curta e update temporal
  condicional; a fila de probes usa upsert atômico e claim por delete
  condicional, impedindo perda no replace e entrega duplicada em pollings
  concorrentes.

---

## Fase 3 — Higiene de frontend, infra e CI (contínuo)

### 🔵 17. Headers de segurança no servidor estático

CSP (`default-src 'self'`, sem `unsafe-inline` para scripts),
`X-Content-Type-Options: nosniff`, `Referrer-Policy`, `frame-ancestors 'none'`.
É a mitigação mais barata enquanto o JWT vive em `localStorage`.

### 🔵 18. Pequenas correções de frontend

- `npm audit fix` — `nanoid@3.3.16` (high, dev-only, GHSA-2v37-7h3g-55p8).
- `frontend/src/stores/vpn.ts` — limpar `lastArtifact` (chave privada) ao
  fechar o `VpnScriptViewer`; hoje fica na memória da aba para sempre.
- `frontend/src/components/VpnScriptViewer.vue:47` — sanitizar o SVG do QR com
  DOMPurify (defesa em profundidade; hoje o backend escapa corretamente).
- `frontend/src/services/apiService.ts` — `AbortSignal.timeout(15000)` por
  requisição; preservar `?redirect=` no 401 global; distinguir erro de rede de
  erro de API.
- Reduzir os `as any` disseminados nas stores/widgets (tipos compartilhados).

### 🔵 19. Supervisão do watcher e hardening do container

- `docker/entrypoint.sh:54` — o watcher roda com `&` e vira órfão: se morrer,
  nunca reinicia e o healthcheck (`Dockerfile:131-132`) continua `healthy`.
  Usar `tini`/supervisão e incluir o watcher no healthcheck.
- Compose: adicionar `security_opt: [no-new-privileges:true]`, `read_only` com
  `tmpfs`, e rotação de logs (`max-size`/`max-file`).
- Pinar imagens por digest (`node:24-alpine`, `rust:slim-bookworm`,
  `debian:bookworm-slim`) e actions por SHA; substituir o arquivado
  `actions-rs/cargo@v1`.
- Documentar deployment atrás de proxy com TLS (Caddy/Traefik/nginx) como
  caminho oficial — hoje API + SPA trafegam HTTP puro.

### 🔵 20. Completar o CI (após o item 2)

- Adicionar `cargo audit` (ou `cargo deny`) e `npm audit`.
- Adicionar job de frontend (`typecheck`, `lint`, `build` — exigidos pelo
  AGENTS.md) e build da imagem Docker.
- Alinhar as flags do clippy do CI com o critério do AGENTS.md (`-D warnings`).

---

## ⚪ Qualidade / dívida técnica (sem urgência)

- `auth.rs:20-24` — magic link restrito a `@example.com`/`@gmail.com`
  (resíduo de scaffold): remover a allowlist ou remover o fluxo.
- `auth.rs:102-103` — comentário promete exigir e-mail verificado no login; o
  código não o faz. Alinhar comentário e comportamento.
- `auth.rs:189-197` — oráculo de timing no login (hash dummy quando o e-mail
  não existe); trocar `tracing::debug!(email, ...)` por pid (PII em log).
- DTOs de credencial derivam `Debug` com a senha (`models/users.rs:14-25`) —
  redigir no `Debug` manual.
- `fixtures/users.yaml` — hashes de senhas públicas do scaffold; guarda de
  ambiente no `Hooks::seed` para nunca rodar em produção.
- `probes.rs:163-165` — `probes::store` aceita `token_hash` arbitrário do
  cliente; o servidor deveria receber o token cru e hashear.
- `checkers/ping.rs:111`, `snmp/client.rs:359` — `lookup_host` sem timeout.
- `probes/agent.rs:129-142` — heartbeat sequencial pode estourar a janela de
  90 s sob carga; mover para task com ticker próprio.
- `probes/buffer.rs:73-105` — buffer offline sem teto, reescrita O(n²),
  escrita não atômica (crash corrompe e descarta tudo): tmp+rename e teto.
- `checkers/http.rs:51-60` — `reqwest::Client` novo a cada checagem; cachear
  dois clientes via `OnceLock`.
- `monitors.timeout_seconds` — coluna escrita e nunca lida pelo scheduler
  (contrato morto): honrar ou remover.
- `access_control.rs:57-63` — mutex envenenado no rate limiter falha aberto
  (decisão documentada): adicionar ao menos um `tracing::error!`.
- `backup/service.rs` — export/restore atrás só do JWT, sem rate limit nem
  auditoria: aplicar o padrão de `vpn_peers.rs` e considerar excluir
  `auth.setup_token` do dump.
- `arp.rs:33` — falha silenciosa se `ip` não existir na imagem: logar debug.

## Verificado como correto (não mexer)

- Hash Argon2id; tokens UUIDv4/aleatórios com expiração e invalidação.
- Respostas uniformes anti-enumeração em forgot/reset/magic-link.
- `AppError::Internal` não vaza detalhe; teste cobre connection string com senha.
- Criptografia em repouso (XChaCha20-Poly1305) com chave obrigatória em produção.
- Views de VPN com **testes que provam** não-vazamento de chaves; cofre efêmero
  consume-on-read com TTL; comparação do setup token em tempo constante.
- Guarda JWT sobrescreve `x-authenticated-user` do cliente (sem spoofing).
- SQL raw sempre parametrizado ou com identificadores constantes; busca textual
  de syslog com escape testado contra injeção.
- Único `Command::new` (`ip -6 neigh show`) com argumentos fixos.
- Escrita de `wg0.conf`: nome fixo, tmp+rename atômico, 0600.
- Migrations disciplinadas (idempotentes, guard de dialeto, FKs inline).
- Apenas 5 `unwrap`/`panic` fora de testes, todos de invariante documentada.
- `.env`/`backend/.env` fora do git; modelo de capabilities do container
  coerente com o §7 do AGENTS.md; sem `v-html` explorável; sem open redirect.
