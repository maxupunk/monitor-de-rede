# Roadmap de limpeza — remoção do AdonisJS

> Objetivo: deixar o repositório rodando **só** com o `backend-rust/` (Loco.rs),
> sem resíduo do AdonisJS em disco, em configuração, em documentação ou **no
> banco de dados** — que é onde está a causa do erro que trava o `docker compose up`.
>
> O corte de código já aconteceu (commit `340eecf`, tag `adonisjs-final`). O que
> resta é o rastro: um volume Postgres com o esquema antigo, um diretório
> `backend/` ignorado pelo git mas vivo em disco, scripts de raiz apontando para
> `npm --prefix backend`, e docs/comentários escritos na gramática do Adonis.

> **Status (12/08/2026):** Fases 0, 1, 2 e 3 concluídas. Fases 4, 5 e 6 pendentes.

---

## Fase 0 — Destravar o `docker compose up` (bloqueante)

### O que está acontecendo

```
Applying migration 'm20260810_000001_users_active'
Error: DB(... code: "42701", message: "column \"active\" of relation \"users\" already exists")
service "migration" didn't complete successfully: exit 1
```

O volume `monitorderede_pgdata` **não é um banco novo**: ele foi criado pelo
AdonisJS em 09/08 e ainda carrega o esquema dele. O `psql` mostra 28 tabelas,
incluindo cinco que o backend Rust não conhece:

| Tabela | Origem | Existe no Rust? |
| :--- | :--- | :---: |
| `adonis_schema`, `adonis_schema_versions` | histórico de migrations do Lucid | ❌ |
| `auth_access_tokens` | `@adonisjs/auth` (substituído por JWT, ver `migration/src/lib.rs:42`) | ❌ |
| `device_addresses`, `device_macs` | modelo antigo de endereços | ❌ |

E o histórico das duas ferramentas está inconsistente:

- `adonis_schema` → 25 migrations aplicadas;
- `seaql_migrations` → **1** linha (`m20220101_000001_users`).

Ou seja: o SeaORM acha que o banco está quase vazio, mas as tabelas já existem.
A primeira migration que faz `ALTER TABLE` em vez de `CREATE TABLE IF NOT EXISTS`
— justamente a `m20260810_000001_users_active` — bate de frente com a coluna
`active` que o Lucid já tinha criado. **Não é bug de migration; é banco sujo.**

### Rota recomendada: recriar o banco do zero

Custo real, conferido no banco atual: `users = 0`, `devices = 1`,
`monitors = 12`, `vpn_peers = 0`, `metrics = 169.101`. Não há usuário
cadastrado nem peer de VPN — é massa de teste. Descartar sai mais barato e mais
seguro que reconciliar dois históricos de migration.

```powershell
# 1. Derruba a stack e APAGA o volume do Postgres (isto é destrutivo)
docker compose down -v

# 2. Sobe de novo — a `migration` roda as 23 migrations em banco limpo
docker compose up -d --build

# 3. Confere: 23 linhas em seaql_migrations, e nenhuma tabela `adonis_*`
docker compose exec postgres psql -U netmonitor -d netmonitor -c "select count(*) from seaql_migrations"
docker compose exec postgres psql -U netmonitor -d netmonitor -c "\dt"

# 4. Cria o usuário administrador (o banco novo não tem nenhum)
docker compose run --rm server backend_rust-cli task user:create `
  email:admin@monitor.local name:"Admin" password:"troque-esta-senha"
```

- [x] 🟢 **Concluído** — volume do banco apagado. Usada a variante que **preserva
      o `wg-config`**: `docker compose down` + `docker volume rm monitorderede_pgdata`.
      Números conferidos antes de apagar e idênticos aos previstos acima
      (`users=0`, `devices=1`, `monitors=12`, `vpn_peers=0`, `metrics=169.101`,
      `seaql_migrations=1`, `adonis_schema=25`).
- [x] 🟢 **Concluído** — `migration` aplicou as 23 migrations em banco limpo e saiu com 0
- [x] 🟢 **Concluído** — `seaql_migrations` com 23 linhas
- [x] 🟢 **Concluído** — `\dt` traz 23 tabelas, sem `adonis_schema`, `adonis_schema_versions`, `auth_access_tokens`, `device_addresses` nem `device_macs`
- [x] 🟢 **Concluído** — admin criado (`admin@monitor.local`, PID `67170dba-…`).
      O `user:create` enfileira um e-mail de boas-vindas e loga
      `no email sender configured`; é esperado — não há SMTP no compose e o
      usuário é gravado antes disso.
- [x] 🟢 **Concluído** — login conferido: `POST /api/auth/login` responde 200 com
      token e usuário. Exigiu corrigir o `JWT_SECRET` (ver abaixo). O frontend em
      `http://localhost:8081` responde 200; o login pela tela não foi clicado.

#### Achado fora do roteiro: `scheduler` em restart loop

Com o banco destravado, a stack subiu inteira pela primeira vez — e aí apareceu
um bug que o `migration` falhando vinha escondendo:

```
Error: Scheduler(InvalidConfigSchema {
  error: Error("unknown field `scheduler`, expected `jobs` or `output`") })
```

`config/scheduler.yaml` começava com um wrapper `scheduler:`. Esse wrapper só
vale quando a seção mora dentro de um `config/<env>.yaml`; passado por
`scheduler --config <arquivo>`, o Loco desserializa o arquivo **inteiro** como a
config do scheduler, e espera `output`/`jobs` na raiz. Wrapper removido.

- [x] 🟢 **Concluído** — `config/scheduler.yaml` corrigido (`output`/`jobs` na raiz)

#### Segundo achado: `JWT_SECRET` precisa ser base64 — e não era

Com a stack de pé, **todo login respondia 401**, inclusive logo depois de um
`register` bem-sucedido no mesmo processo. A senha nunca foi o problema: o hash
gravado pelo container valida sem ressalva contra a senha usada (conferido em
teste isolado com `loco_rs::hash::verify_password`).

A causa é que o handler usa a **mesma string** nos dois pontos de saída:

```rust
if !valid { return unauthorized("unauthorized!"); }          // senha errada
let token = user.generate_jwt(...).or_else(|_| unauthorized("unauthorized!"))?;  // JWT falhou
```

Quem falhava era o segundo. O Loco assina com
`EncodingKey::from_base64_secret` (HS512, `loco-rs-1.0.1/src/auth/jwt.rs:113`),
então o segredo **tem de ser base64 válido**. O padrão do compose era
`troque_este_segredo_em_producao_por_favor` — tem `_` e 41 caracteres, não
decodifica. Resultado: servidor sobe normal, health check verde, e 100% dos
logins caem em 401 com a mensagem de senha inválida.

Não é regressão desta limpeza: o valor vinha do `docker-compose.yml`. Só apareceu
agora porque esta é a primeira vez que a stack sobe inteira.

- [x] 🟢 **Concluído** — `docker-compose.yml` com padrão base64 válido e comentário explicando a exigência
- [x] 🟢 **Concluído** — `.env` local com segredo base64 aleatório de 64 bytes
- [x] 🟢 **Concluído** — `.env.example` documenta a exigência e como gerar
- [ ] Vale considerar mensagens distintas para "senha inválida" e "falha ao gerar
      token" — não por UX (a resposta ao cliente deve seguir genérica), mas no
      log, que hoje não distingue os dois casos. Fora do escopo desta limpeza.

#### Terceiro achado: `HEALTHCHECK` marcava todo container como `unhealthy`

`server`, `scheduler`, `probe` e `vpn-probe` ficavam `unhealthy` com a API
respondendo 200. O `HEALTHCHECK` do `backend-rust/Dockerfile` rodava
`backend_rust-cli doctor` sem `--production`; nesse modo o `doctor` inclui
`check_deps()`, que lê o `Cargo.lock` — arquivo que a imagem de runtime não
recebe (ela copia só o binário e o `config/`). Saída:
`Error: VersionCheck(LockfileError("I/O operation failed: entity not found"))`,
exit 1, sempre.

Com `--production` o `doctor` roda só os checks que fazem sentido em runtime e
responde `✅ DB connection: success`, exit 0.

De quebra, o comentário acima do bloco dizia "`GET /` é o mesmo health check do
backend AdonisJS" — descrevia um health check HTTP que o comando nunca executou.
Reescrito para explicar a exigência real (era também um item da Fase 5.4).

- [x] 🟢 **Concluído** — `HEALTHCHECK` usa `doctor --production`
- [x] 🟢 **Concluído** — comentário do `Dockerfile:68` reescrito

> **Só apague o volume depois de conferir os números acima no seu ambiente.**
> `docker compose down -v` remove `pgdata` **e** `wg-config`; se já houver
> configuração de WireGuard que você queira preservar, apague apenas o volume do
> banco: `docker compose down && docker volume rm monitorderede_pgdata`.

### Rota alternativa: preservar os dados existentes

Se em outro ambiente houver dados que importam, **não** apague o volume. O
caminho é o do runbook `corte_backend_rust.md` §2 — `pg_dump` do banco Adonis,
banco novo criado pelas migrations do Rust, `pg_restore --data-only`. Depois,
limpar o resíduo:

```sql
DROP TABLE IF EXISTS adonis_schema, adonis_schema_versions, auth_access_tokens,
                     device_addresses, device_macs CASCADE;
```

Atenção ao passo 2.2 do runbook: os segredos da VPN são cifrados com `APP_KEY` e
precisam ser re-exportados **antes** do corte, por um comando que só existe no
`backend/` arquivado (`git checkout adonisjs-final -- backend/`). Como
`vpn_peers = 0` aqui, isso não se aplica ao ambiente local.

- [x] 🟢 **Concluído** — local seguiu a rota de recriar. Nenhum outro ambiente
      foi tocado; se existir produção, a rota de migração acima continua valendo.

---

## Fase 1 — Remover os artefatos do Adonis do disco

O commit `340eecf` tirou `backend/` do controle de versão (`git ls-files backend`
retorna vazio; o `.gitignore` o marca como ignorado), mas **o diretório continua
em disco** com `node_modules/`, `build/`, `ace.js`, `adonisrc.js`, `app/`,
`config/`, `database/`, `commands/`, `modules/`. Ele é peso morto: não entra em
build nenhum, mas confunde busca, IDE e agentes.

```powershell
# O conteúdo permanece recuperável pela tag: git checkout adonisjs-final -- backend/
Remove-Item -Recurse -Force "backend"
```

- [x] 🟢 **Concluído** — `backend/` removido do disco. O que restava nele já era
      só resíduo de build (`.adonisjs/`, `build/`, `node_modules/`): os fontes
      (`app/`, `config/`, `database/`, `ace.js`…) tinham saído no `340eecf`.
- [x] 🟢 **Concluído** — tag `adonisjs-final` confirmada antes de apagar
- [x] 🟢 **Concluído** — `backend_rust_development.sqlite` e `backend_rust_test.sqlite{,-shm,-wal}` removidos.
      Nota: `cargo test` **recria** os `backend_rust_test.sqlite*` (é o banco da
      suíte, `config/test.yaml`). São ignorados pelo git; não é resíduo a caçar
      de novo, só não versionar.

---

## Fase 2 — Configuração da raiz

### 2.1 `package.json` (raiz)

Todos os scripts de backend apontam para `npm --prefix backend`, que agora é um
diretório inexistente. `npm test` na raiz executa a suíte do Adonis.

Substituir por um arquivo que descreva a stack real — frontend em npm, backend em
cargo — ou remover os scripts de backend e deixar só os de frontend:

```json
{
  "name": "network-monitor-workspace",
  "version": "1.0.0",
  "private": true,
  "type": "module",
  "scripts": {
    "dev:frontend": "npm --prefix frontend run dev",
    "build:frontend": "npm --prefix frontend run build",
    "typecheck:frontend": "npm --prefix frontend run typecheck",
    "lint:frontend": "npm --prefix frontend run lint",
    "format:frontend": "npm --prefix frontend run format"
  }
}
```

- [x] 🟢 **Concluído** — `dev:backend`, `build:backend`, `test:backend`, `test`, `typecheck:backend`, `lint:backend`, `format:backend` removidos
- [x] 🟢 **Concluído** — não existia README na raiz; foi criado um (`README.md`)
      com a estrutura do repositório, o `docker compose up`, os comandos `cargo`
      de `backend-rust/` e os de npm do frontend.

### 2.2 `.gitignore`

O bloco de topo é titulado "Dependencies and AdonisJS build" e ignora
`.adonisjs`, `build`, `tmp/*`. Sem `backend/`, `build` e `.adonisjs` não existem
mais; `node_modules` continua válido por causa do `frontend/`.

- [x] 🟢 **Concluído** — cabeçalho agora é "Dependências do frontend"
- [x] 🟢 **Concluído** — `.adonisjs` e `build` removidos; `public/assets` (saída
      estática do Adonis) também saiu, já que não existe `public/` no repositório
- [x] 🟢 **Concluído** — não havia regra `backend/`: o diretório era ignorado
      justamente por `build`/`.adonisjs`/`node_modules` (`git check-ignore backend`
      retornava vazio). Com a Fase 1 feita, o ponto perdeu o objeto.
- [x] 🟢 **Concluído** — bloco `# Rust / Loco.rs` mantido intacto

### 2.3 `.dockerignore`

Ignora `build`, `dist`, `tmp` — contexto do Adonis. O contexto de build hoje é
`./backend-rust` e `./frontend`.

- [x] 🟢 **Concluído** — reescrito para `.git`, `.env`, `node_modules`, `frontend/node_modules`, `frontend/dist`, `backend-rust/target`, `.DS_Store`

### 2.4 `.prettierignore`

Cita `.adonisjs`, `build`, `database/schema.ts` (schema do Lucid) e `tmp`. Só o
frontend usa Prettier hoje.

- [x] 🟢 **Concluído** — reduzido a `node_modules`, `frontend/dist`, `frontend/dev-dist`

### 2.5 `.env` e `.env.example`

Estes dois arquivos ainda descrevem um app Node. Variáveis que **o backend Rust
não lê**:

| Variável | Situação |
| :--- | :--- |
| `NODE_ENV` | substituída por `LOCO_ENV` |
| `SESSION_DRIVER` | conceito do AdonisJS |
| `APP_URL` | ⚠️ correção: **é lido**, sim — `config/production.yaml:11` o usa em `server.host`. Mantido. |
| `PORT`, `HOST`, `LOG_LEVEL` | inertes: o `x-app-env` do compose os injeta, mas nenhum YAML os consulta (porta e binding estão fixos no config, e o nível do logger também) |
| `DB_CONNECTION`, `DB_HOST`, `DB_PORT`, `DB_USER`, `DB_PASSWORD`, `DB_DATABASE` | o Loco usa **`DATABASE_URL`** (o compose já injeta a URL completa) |
| `DB_CONNECTION=sqlite` no `.env` | pior caso: sugere que o alvo é SQLite, e não é |

Faltando: `DATABASE_URL`, `JWT_SECRET`, `LOCO_ENV`. Comentários a corrigir:

- `.env.example` — "gere com: `node ace vpn:probe-register`" → `backend_rust-cli task vpn_probe_register`
- `.env.example` — "O DataPrunerService executa a cada 1h via `scheduler:run`" → scheduler nativo do Loco (`config/scheduler.yaml`)
- ambos — menções ao **Redis**, que não existe em nenhum serviço do `docker-compose.yml`

- [x] 🟢 **Concluído** — `.env.example` reescrito só com o que o Rust lê, com
      `DATABASE_URL`, `JWT_SECRET` e `LOCO_ENV` documentados. Levantamento feito
      por `env::var`/`get_env` no código, não por suposição. Duas variáveis lidas
      que **não estavam** documentadas foram acrescentadas: `FRONTEND_ORIGIN`
      (origem do CORS, nos três YAMLs) e os `DB_*_CONNECTIONS`/`DB_*_TIMEOUT` do
      pool (comentados, com os padrões dos YAMLs).
- [x] 🟢 **Concluído** — `.env` local alinhado ao novo `.env.example`
- [x] 🟢 **Concluído** — menções a Redis removidas dos dois arquivos. Não há
      serviço Redis no compose nem `redis` no `Cargo.toml`; os `workers` do Loco
      rodam em `BackgroundAsync`, in-process.
- [x] 🟢 **Concluído** — comentários corrigidos: `node ace vpn:probe-register` →
      `backend_rust-cli task vpn_probe_register`; e a nota do `DataPrunerService`
      agora descreve o que o código faz — a purga roda **dentro** do ciclo do
      `scheduler_run` (`is_due("data_pruner", …)`), não num processo à parte.

---

## Fase 3 — Ferramentas de paridade dentro do `backend-rust/`

Duas ferramentas existiram só para provar que o Rust reproduzia o Adonis. Com o
Adonis fora, elas não têm mais contra o quê rodar:

| Arquivo | Função | Ação |
| :--- | :--- | :--- |
| `backend-rust/examples/parity_check.rs` | bate endpoint a endpoint contra `ADONIS_URL` | remover |
| `backend-rust/examples/schema_parity.rs` | compara o esquema SeaORM com as migrations do Lucid (lê o código de `backend/`) | remover |

O `schema_parity` lê o diretório `backend/`, então quebra em tempo de execução
assim que a Fase 1 rodar. Vale conferir se `examples/playground.rs` e
`examples/spikes/` ainda descrevem algo vivo antes de mexer neles.

- [x] 🟢 **Concluído** — `parity_check.rs` removido
- [x] 🟢 **Concluído** — `schema_parity.rs` removido
- [x] 🟢 **Concluído** — as duas entradas `[[example]]` do `Cargo.toml` também
      saíram (sem isso o cargo falha: elas apontam `path` explícito). Os três
      `spike_*` e o `playground` ficaram — o playground é o scaffold do Loco
      (usado pelo alias `cargo playground`) e os spikes sustentam a ADR 003.
- [x] 🟢 **Concluído** — `cargo build --examples` compila sem eles
- [x] 🟢 **Concluído** — `backend-rust/AGENTS.md:76` já não manda rodar
      `schema_parity` depois de `db entities`.
- [ ] ⚠️ **Deixado de propósito** — as outras menções vivem em
      `docs/corte_backend_rust.md` e `docs/roadmap_backend_rust.md`, que a Fase 4
      classifica como registro datado a mover **intacto** para `docs/historico/`.
      Editá-las aqui contrariaria aquela regra; a decisão fica para a Fase 4.
      Sobra ainda `migration/src/m20260810_000018_vpn_peers.rs:20`, comentário do
      tipo "explica uma decisão" — escopo da Fase 5.4.

---

## Fase 4 — Documentação

Contagem de menções a Adonis/Lucid/`node ace`/Japa por arquivo:

| Arquivo | Menções | Ação sugerida |
| :--- | :---: | :--- |
| `docs/arquitetura.md` | 28 | **reescrever**: é a referência viva; deve descrever Loco.rs/SeaORM, não Lucid |
| `docs/corte_backend_rust.md` | — | runbook **concluído** → mover para `docs/historico/` |
| `docs/roadmap_backend_rust.md` | — | roadmap **concluído** → mover para `docs/historico/` |
| `docs/diretrizes_testes.md` | 7 | reescrever em cima de `cargo test` / `#[tokio::test]`, sem Japa |
| `docs/roadmap.md` | 7 | remover a nota de topo "leia AdonisJS como…" e traduzir os termos no corpo |
| `docs/base.md` | 5 | atualizar a stack declarada |
| `docs/roadmap_vpn.md` | 5 | traduzir comandos `node ace` para `backend_rust-cli task` |
| `docs/adr/001`, `005`, `006` | — | **manter como estão** — ADR é registro histórico; reescrever apaga a decisão |
| `docs/roadmap_melhorias.md`, `docs/roadmap_dispositivos_monitores_discovery.md` | 1 cada | ajuste pontual |

Regra de bolso: documento que descreve **como o sistema é hoje** (arquitetura,
diretrizes, roadmap ativo) precisa falar Rust. Documento que registra **uma
decisão ou um procedimento datado** (ADR, runbook do corte) fica intacto e vai
para `historico/`.

- [ ] `docs/historico/` criado com os dois documentos da migração
- [ ] `docs/arquitetura.md` atualizado
- [ ] `docs/diretrizes_testes.md` atualizado
- [ ] `docs/base.md`, `docs/roadmap.md`, `docs/roadmap_vpn.md` atualizados
- [ ] Links quebrados conferidos após as movimentações

---

## Fase 5 — Instruções de agentes e comentários no código

### 5.1 `.agents/AGENTS.md` — desatualizado, e isso custa caro

Ainda manda rodar `npx tsc --noEmit` e `node ace test` como validação de
backend, e tem uma seção inteira de "Práticas de Teste no Japa". O `AGENTS.md`
da raiz já foi atualizado para `cargo fmt/clippy/test`; o de `.agents/` não.
Enquanto os dois divergirem, qualquer agente pode seguir o errado.

- [ ] Bloco "Backend" trocado por `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
- [ ] Seção "Práticas de Teste no Japa" substituída pelas diretrizes de teste do Rust
- [ ] Decidido se `.agents/AGENTS.md` continua existindo ou vira um ponteiro para o `AGENTS.md` da raiz

### 5.2 `AGENTS.md` (raiz)

O cabeçalho diz "Enquanto `backend/` existir, ele é referência de comportamento".
Depois da Fase 1 ele não existe mais.

- [ ] Parágrafo reescrito: fonte da verdade é `backend-rust/`, ponto; histórico via tag `adonisjs-final`

### 5.3 Frontend

- [ ] `frontend/nginx.conf:24` — comentário "Proxy de APIs para o Container AdonisJS"
- [ ] `frontend/src/bindings/VpnPeerWithDevice.ts:9` — se for gerado por `ts-rs`, corrigir o doc comment **no struct Rust** e regerar; editar o `.ts` à mão é desfeito na próxima geração

### 5.4 Comentários no código Rust (baixa prioridade)

Cerca de 30 arquivos em `backend-rust/src/` citam o AdonisJS. Eles se dividem em
dois tipos, e só um incomoda:

- **Explicam uma decisão** — ex.: `migration/src/lib.rs:42` ("`auth_access_tokens`
  existia por causa do `@adonisjs/auth`") justifica por que uma tabela *não*
  existe. Apagar isso perde informação. **Manter.**
- **Só apontam paridade** — ex.: `services/mod.rs:1` ("o equivalente de
  `backend/modules/**`"), `Dockerfile:68` ("o mesmo health check do backend
  AdonisJS"). Apontam para um caminho que não existe mais. **Reescrever.**

- [ ] Varredura feita com `git grep -n -i adonis -- backend-rust/src backend-rust/Dockerfile`
- [ ] Comentários do segundo tipo reescritos sem referência a caminho morto
      — o `Dockerfile:68` já saiu, junto com a correção do `HEALTHCHECK` (Fase 0)

---

## Fase 6 — Verificação final

```powershell
# Nenhuma referência viva ao Adonis fora de docs/historico e docs/adr
git grep -i -l "adonis" -- . ':!docs/historico' ':!docs/adr'

# Backend
cd backend-rust
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test

# Frontend
cd ..\frontend
npm run typecheck
npm run lint
npm run build

# Stack completa, do zero
cd ..
docker compose down -v
docker compose up -d --build
docker compose ps
```

Critério de pronto:

- [ ] `docker compose up` sobe os 8 serviços sem `migration` falhando
- [ ] `GET http://localhost:3333/` responde o health check
- [ ] Login funciona no frontend em `http://localhost:8081`
- [ ] Um monitor executa e grava resultado (prova de que o `scheduler` está no ciclo)
- [ ] `cargo test` e o build do frontend verdes
- [ ] `git grep -i adonis` só retorna `docs/historico/` e `docs/adr/`
- [ ] `backend/` não existe em disco e nada no repositório aponta para ele

---

## Ordem de execução

A Fase 0 é independente e desbloqueia o ambiente **agora** — faça primeiro. As
Fases 1→3 são mecânicas e podem ir num commit só. A Fase 4 é a mais demorada
(reescrita de `arquitetura.md`) e não bloqueia nada. A Fase 5.1 vale antecipar:
enquanto `.agents/AGENTS.md` mandar rodar `node ace test`, todo agente que ler
esse arquivo vai começar errado.

| Ordem | Fase | Bloqueia o quê |
| :---: | :--- | :--- |
| 1º | Fase 0 — banco | tudo; a stack não sobe |
| 2º | Fase 5.1 — `.agents/AGENTS.md` | qualidade de qualquer trabalho assistido |
| 3º | Fases 1–3 — disco, config, examples | nada, mas é o grosso do ruído |
| 4º | Fase 4 — docs | nada |
| 5º | Fase 6 — verificação | fecha o ciclo |
