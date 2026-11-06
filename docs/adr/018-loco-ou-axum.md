# ADR 018 — Manter o Loco.rs como ponto de montagem (não trocar por `axum` + `sea-orm` direto)

- **Status:** aceito — 2026-11-05; confirmado pelo responsável pelo produto em 2026-11-06.
- **Data:** 2026-11-05
- **Relacionado:** [ADR 007](007-scheduler-processo-unico.md),
  [ADR 017](017-arquitetura-modular.md) (seção "Loco.rs").

## Contexto

O ADR 017 deixou o Loco.rs de pé durante a reconstrução em crates e adiou a
pergunta para quando o pacote raiz fosse o único ponto de montagem: "trocá-lo
por `axum` + `sea-orm` direto vira uma troca localizada — decidida então, com
dados". A Fase 10 entregou essa condição: o domínio mora em `crates/` (núcleo,
áreas e assistente), e o pacote raiz só monta.

A pergunta é se o Loco ainda paga o que custa. Os dados abaixo foram medidos no
workspace como ele está ao fim da Fase 10.

## O que o Loco faz de fato aqui

| Recurso do Loco | Uso medido | Substituto direto |
| :--- | :--- | :--- |
| `Hooks` (`boot`, `after_context`, `routes`, `after_routes`, `truncate`, `seed`, `register_tasks`, `initializers`, `init_logger`) | 1 `impl Hooks` (`src/app.rs`, ~350 linhas) | um `main` próprio com `tokio`, `clap` e o ciclo de vida escrito à mão |
| Configuração por ambiente (`config/*.yaml` com Tera, `LOCO_ENV`, `get_env`) | 3 arquivos; `ctx.config` lido em 9 pontos | `config` + `serde_yaml` + um renderizador de variáveis |
| `AppContext` (`db`, `config`, `shared_store`) | `AppContext` em ~200 assinaturas; `shared_store` em 77 pontos (o registro de dependências de processo e os registros das áreas) | um `struct AppState` próprio e um mapa de tipos |
| Rotas (`Routes::new().prefix(..).add(..)`, `AppRoutes::prefix("/api")`, `format::json`) | 43 grupos de rotas; `format::json` em 278 respostas | `axum::Router::nest` + `Json` |
| Tarefas da CLI (`Task`, `cargo loco task`) | 7 tarefas (`scheduler_run`/`loop`, `probe_run`, `probe_register`, `vpn_probe_register`, `user_create`, `auth_setup_token`) | subcomandos `clap` |
| Initializers | 8 (servidor) | chamadas no `main` |
| Migrator e `db migrate`/`db entities` | 1 crate `migration` (sobre `sea-orm-migration`, com helpers do Loco em `schema`) | `sea-orm-migration` direto (já é o motor) |
| Testes de requisição (`request`/`request_with_config::<App>`, `boot_test`, `TestServer`) | **412** testes de requisição, 35 `boot_test`, 44 `TestServer` | um harness próprio sobre `axum-test` ou `tower::ServiceExt`, com banco por teste e o `truncate` das 47 tabelas |
| JWT (`loco_rs::auth::JWT`), mailer, `ctx.storage` | JWT em 40 pontos; 1 mailer (3 modelos); storage em 1 ponto | `jsonwebtoken`, `lettre`, `opendal` (já na árvore) |
| Workers/fila, cache, scheduler nativo | **nenhum uso** (o scheduler nativo foi abandonado no ADR 007) | — |

Arquivos `.rs` que citam `loco_rs`: 287 (`nucleo` 83, testes 80, `network`
43, `core` 28, pacote raiz 18, `assistant` 16, `servers` 10, `vpn` 6,
`migration` 3, `agent-protocol` 0). Quase todos por `AppContext` (155) e pelo
prelúdio (`loco_rs::prelude`, 87) — isto é, pelo tipo do contexto e pelas rotas, não por
recurso exclusivo do framework.

## O que o Loco custa

Os contornos que existem por causa dele, todos documentados no código:

1. `run_task` não executa initializers: toda dependência de processo mora em
   `after_context` (ADR 007), e há teste que boota por `create_context` só para
   pegar essa classe de erro (`tests/requests/process_deps.rs`).
2. O logger é reimplementado em `init_logger` (camada de log da aplicação no
   banco de logs) e o `file_appender` do Loco não é reproduzido.
3. O filtro padrão do log só libera `loco_rs`, alguns módulos dele e o crate da
   aplicação (`backend`). Com o código nos crates do workspace, o log deles
   sumia em silêncio desde a Fase 6 — corrigido na Fase 10 com as diretivas de
   `WORKSPACE_CRATES` em `app.rs`.
4. O envelope de paginação do Loco não serve ao frontend (`paginate` próprio) e
   os erros saem por `AppError`, não por `loco_rs::Error`.
5. Uma dependência a mais para auditar e acompanhar (Loco 1.2, com `loco-gen`
   na árvore).

## Alternativas avaliadas

| Alternativa | Ganho | Custo |
| :--- | :--- | :--- |
| **Trocar por `axum` + `sea-orm` direto** | some a camada de contornos (1–4); controle total do ciclo de vida e do log; um framework a menos | reescrever o harness de 412 testes de requisição + 35 `boot_test` (o maior custo, e o que protege a reconstrução), as 7 tarefas, a configuração com Tera, o JWT e o mailer; trocar o tipo `AppContext` em ~200 assinaturas espalhadas por **todos** os crates; nenhum comportamento novo para o usuário |
| **Isolar o Loco só no pacote raiz** (os crates recebem um `AppState` próprio) | os crates deixariam de depender do Loco | o `shared_store`, o `db` e a `config` são exatamente o que os crates usam do `AppContext`: seria reescrever o mesmo contexto com outro nome, pelos mesmos 200 pontos |
| **Manter** | zero trabalho; a suíte continua sendo a especificação | os contornos 1–4 ficam (já escritos e testados) |

## Decisão

**Manter o Loco.rs.** Ele hoje é, na prática, o ponto de montagem (`Hooks`), a
configuração por ambiente, o contexto compartilhado, as rotas, a CLI de
tarefas e — o que mais pesa — o harness dos testes de requisição. Os recursos
caros dele que o projeto não quer (fila, workers, cache, scheduler nativo) não
são usados e não custam nada em tempo de execução. Os contornos que ele exige
já estão escritos, documentados e cobertos por teste.

Trocá-lo seria uma reescrita de infraestrutura sem efeito para o usuário, com o
custo concentrado justamente na rede de segurança (os testes de requisição). A
reconstrução em crates (ADR 017) não depende disso: a regra de dependência é
garantida pelo `Cargo.toml`, não pelo framework.

## Quando reabrir

- Uma atualização do Loco quebrar o `Hooks`, o `AppContext` ou o harness de
  testes de forma que a migração custe mais que a troca.
- O projeto passar a precisar de algo que o Loco impeça (por exemplo, dois
  servidores HTTP no mesmo processo, ou um ciclo de vida que o `Hooks` não
  exponha).
- O harness de testes ser substituído por outro motivo — aí o maior custo da
  troca deixa de existir.

## Consequências

- Os crates continuam recebendo `&AppContext` e usando `shared_store` para os
  registros das áreas (`AreaPolicies`, `AreaRoutines`, `MonitorGroups`,
  `MonitorCheckers`, `AgentCommandDispatchers`, o `DeviceAccess` e os
  `CentralServices`).
- Regra que continua valendo: dependência de processo vai em `after_context`,
  nunca num `Initializer` (ADR 007).
- Crate novo no workspace entra em `WORKSPACE_CRATES` (`src/app.rs`), ou o log
  dele não aparece com o filtro padrão.
