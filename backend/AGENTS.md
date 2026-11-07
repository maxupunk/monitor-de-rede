# Agent guide for this Loco app

This is a **Loco** (loco.rs) application — an all-in-one, batteries-included Rust
web framework. Routing, the database (Sea-ORM), background jobs, a scheduler,
mailers, tasks, storage, caching, and testing are already integrated. **Prefer
Loco's built-ins and generators over adding external crates or wiring
infrastructure by hand.**

## Where things live

Workspace Cargo (ADR 017). O pacote raiz (`backend`) é **só o ponto de
montagem**; o domínio mora nos crates.

```
src/app.rs              # impl Hooks for App — rotas, initializers, tarefas (a montagem)
src/registries.rs       # o que cada área contribui (políticas, checkers, rotinas, grupos,
                        # comandos do agente, acesso ao equipamento, catálogo da IA,
                        # CentralServices de cada área) — único lugar que conhece todas
src/initializers/       # process_deps (todo processo) + os do servidor
src/tasks/              # CLI/admin tasks (scheduler, probe, registros)
src/bin/                # backend-cli e netmonitor-agent
src/spa.rs              # o dist do frontend
crates/core/            # contratos e kernel: models (+ _entities GERADAS), shared, events,
                        # areas, agents (canal), routines, monitoring, devices::access…
crates/nucleo/          # o Núcleo: services/, controllers/, dtos/, views/, mailers/
crates/agent-protocol/  # contrato de fio central ↔ agente
crates/{network,servers,vpn}/  # as áreas, cada uma com services, controllers e dtos
crates/assistant/       # a IA (chat, harness, Decisor)
src/migrator.rs         # Migrator: o esquema do core + o de cada área (crates/*/src/migrations)
config/*.yaml           # per-environment config (LOCO_ENV)
tests/                  # request/model/task tests + conventions/
```

Regra de dependência: `core` ← `nucleo` e áreas; área nunca depende de outra
nem do `nucleo`; só `app.rs`, `registries` e `initializers/` nomeiam um crate
de área (`tests/conventions/architecture.rs`).

## How to work in this app

- **Os generators do Loco escrevem no pacote raiz** (`src/`): o código gerado
  vai para o crate dono (o `nucleo` ou o da área) e só a montagem fica em
  `src/app.rs`/`src/registries.rs`.
- **Everything uses `AppContext` (`ctx`)**: `ctx.db`, `ctx.config`,
  `ctx.mailer`, `ctx.storage`, `ctx.cache`, `ctx.queue_provider`. Don't create
  your own DB pool, server, or job queue.
- Start every controller/model/worker/task with `use loco_rs::prelude::*;`.
- App code returns `loco_rs::Result<T>` and uses `?`.
- Config is YAML in `config/`; secrets come from the environment via the
  `get_env` Tera helper inside the YAML.
- Primary/foreign keys are `i64` (this is Loco 0.17+).
- Tests: `request::<App, _, _>(|request, ctx| async move { ... }).await;`.

## Useful commands

```
cargo loco start            # run the app
cargo loco db migrate       # apply migrations
cargo loco routes           # list routes
cargo loco task <name>      # run a task
cargo loco doctor           # check the environment
```

## Convenções deste projeto (além do padrão Loco)

Ver `docs/adr/` para as decisões arquiteturais vigentes.

- **Prefixo `/api`** vem do `AppRoutes::prefix` em `src/app.rs`, não do controller.
  Um `Routes::new().prefix("/auth")` vira `/api/auth`. `GET /`, `_ping` e `_health`
  ficam na raiz porque são registrados **antes** do `prefix`.
- **`services/`** de cada crate guarda o domínio (`crates/nucleo/src/services`
  no Núcleo). Controller só extrai, valida, delega e serializa.
- **Erros:** handlers devolvem `Result<_, AppError>`
  (`crates/core/src/shared/errors.rs`), não `loco_rs::Error`. O corpo é
  `{"message": "..."}` porque é isso que o frontend lê. Mensagens em português.
- **`#[serde(rename_all = "camelCase")]` em todo DTO.** Não é estilo: o teste
  `tests/conventions/camel_case.rs` falha se você esquecer.
- **Paginação:** use `paginate` (`crates/core/src/shared/pagination.rs`). O
  envelope é `{data, meta}`, não o `PaginationResponse` do Loco — é o formato
  que o `useInfiniteList` do frontend lê.
- **Bindings TypeScript:** `#[ts(export, export_to = "../../frontend/src/bindings/")]`.
  Eles são gerados durante `cargo test`.
- **Dependência de processo vai em `Hooks::after_context`, nunca num
  `Initializer`.** O `run_task` do Loco **não** executa initializers — só o
  `run_app` executa. Um `Initializer` só existe para o processo servidor. Foi
  essa confusão que deixou `scheduler` e `probe` sem cliente ICMP, com todo
  monitor de ping gravando `unknown` ([ADR 007](../docs/adr/007-scheduler-processo-unico.md)).
  Regra prática: precisa no `task`? `after_context`. Só no servidor (laço de
  SSE, seed de boot)? `Initializer`.
- **Teste de coisa que roda em `task` não pode bootar por `request_with_config`**:
  esse caminho passa pelo servidor e esconde exatamente essa classe de bug. Use
  `loco_rs::boot::create_context::<App>` — ver `tests/requests/process_deps.rs`.
- **Crate novo no workspace** entra em `WORKSPACE_CRATES` (`src/app.rs`): o
  filtro padrão de log do Loco só libera o crate `backend`, e o log do crate
  novo sumiria ([ADR 018](../docs/adr/018-loco-ou-axum.md)).
- **Tabelas novas** entram em `crates/core/src/models/tables.rs` (`CREATION_ORDER`). O
  `Hooks::truncate` já as cobre; não mexa no `app.rs`.
- **Sem produção, sem migration nova para alterar esquema.** O sistema nunca foi
  instalado de verdade: mudar uma tabela é editar a migration que a cria e
  recriar o banco de desenvolvimento. Não gere migration de `alter`, de
  compatibilidade nem rotina de upgrade (ver `AGENTS.md` §3 e o DB-02 em
  `docs/debitos_tecnicos.md`).
- **Migrations:** cada crate leva as próprias tabelas em `src/migrations/`
  (o núcleo em `crates/core`, cada área no seu crate; tabela de área só tem
  FK para o núcleo — `tests/conventions/architecture.rs` confere) e usa os
  helpers de `netmonitor_core::schema`. As FKs são
  declaradas à mão (`fk(...)` com a ação de `ON DELETE`) porque o parâmetro
  `refs` do `create_table` do Loco deriva a ação da nulabilidade, e o esquema
  tem FKs anuláveis com `CASCADE`. Índices levam **nome explícito**, nunca o
  que o banco derivaria.
- **`cargo loco db entities` roda contra o PostgreSQL**, nunca contra o SQLite:
  o SQLite reporta todo inteiro como `INTEGER` e a entidade sai com `i64` onde
  o Postgres tem `INT4` — o `sqlx` recusa a leitura em produção. Depois de
  gerar, confira o diff das entidades e rode `cargo test` (também contra o
  Postgres).
- **Porta de produção configurável por `APP_PORT`**; em desenvolvimento e teste
  permanece 3333, que é o destino do proxy do Vite.

## Learn more

- Framework agent guide: https://loco.rs/AGENTS.md
- Full single-file reference: https://loco.rs/llms-full.txt
- Docs: https://loco.rs/docs
