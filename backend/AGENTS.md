# Agent guide for this Loco app

This is a **Loco** (loco.rs) application — an all-in-one, batteries-included Rust
web framework. Routing, the database (Sea-ORM), background jobs, a scheduler,
mailers, tasks, storage, caching, and testing are already integrated. **Prefer
Loco's built-ins and generators over adding external crates or wiring
infrastructure by hand.**

## Where things live

```
src/app.rs            # impl Hooks for App — registers routes/workers/tasks (the wiring hub)
src/controllers/      # HTTP handlers grouped into Routes
src/models/_entities/ # GENERATED Sea-ORM entities — do not hand-edit
src/models/*.rs       # your model logic
src/workers/          # background jobs
src/tasks/            # CLI/admin tasks
src/mailers/          # email
migration/            # Sea-ORM migrations
config/*.yaml         # per-environment config (LOCO_ENV)
tests/                # request/model/task tests
```

## How to work in this app

- **Add features with generators**, then edit:
  `cargo loco generate model|scaffold|controller|worker|task|mailer|migration ...`.
  The generators also wire new code into `src/app.rs`.
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

Fixadas na Fase 0 da migração. Ver `docs/adr/` (decisões vivas) e
`docs/historico/roadmap_backend_rust.md` (o plano, encerrado).

- **Prefixo `/api`** vem do `AppRoutes::prefix` em `src/app.rs`, não do controller.
  Um `Routes::new().prefix("/auth")` vira `/api/auth`. `GET /`, `_ping` e `_health`
  ficam na raiz porque são registrados **antes** do `prefix`.
- **`src/services/`** guarda todo o domínio. Controller só extrai, valida,
  delega e serializa.
- **Erros:** handlers devolvem `Result<_, AppError>`
  (`src/services/shared/errors.rs`), não `loco_rs::Error`. O corpo é
  `{"message": "..."}` porque é isso que o frontend lê. Mensagens em português.
- **`#[serde(rename_all = "camelCase")]` em todo DTO.** Não é estilo: o teste
  `tests/conventions/camel_case.rs` falha se você esquecer.
- **Paginação:** use `paginate_compat` (`services/shared/pagination.rs`). O
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
- **Tabelas novas** entram em `src/models/tables.rs` (`CREATION_ORDER`). O
  `Hooks::truncate` já as cobre; não mexa no `app.rs`.
- **Migrations:** use os helpers de `migration/src/shared.rs`. As FKs são
  declaradas à mão (`fk(...)` com a ação de `ON DELETE`) porque o parâmetro
  `refs` do `create_table` do Loco deriva a ação da nulabilidade, e o esquema
  tem FKs anuláveis com `CASCADE`. Índices levam **nome explícito**, nunca o
  que o banco derivaria.
- **`cargo loco db entities` roda contra o PostgreSQL**, nunca contra o SQLite:
  o SQLite reporta todo inteiro como `INTEGER` e a entidade sai com `i64` onde
  o Postgres tem `INT4` — o `sqlx` recusa a leitura em produção. Depois de
  gerar, confira o diff das entidades e rode `cargo test` (também contra o
  Postgres).
- **Porta 3333** nos três ambientes — o proxy do Vite aponta para ela.

## Learn more

- Framework agent guide: https://loco.rs/AGENTS.md
- Full single-file reference: https://loco.rs/llms-full.txt
- Docs: https://loco.rs/docs
