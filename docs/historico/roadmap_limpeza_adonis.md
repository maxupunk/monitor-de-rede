# Roadmap de limpeza — remoção do AdonisJS

> Objetivo: deixar o repositório rodando **só** com o `backend/` (Loco.rs),
> sem resíduo do AdonisJS em disco, em configuração, em documentação ou **no
> banco de dados** — que é onde está a causa do erro que trava o `docker compose up`.
>
> O corte de código já aconteceu (commit `340eecf`, tag `adonisjs-final`). O que
> resta é o rastro: um volume Postgres com o esquema antigo, um diretório
> `backend/` ignorado pelo git mas vivo em disco, scripts de raiz apontando para
> `npm --prefix backend`, e docs/comentários escritos na gramática do Adonis.

> **Status (12/08/2026):** todas as fases concluídas. Este documento virou
> registro do que foi feito e, pela regra que a própria Fase 4 estabeleceu,
> foi arquivado aqui em `docs/historico/`. Não vale como instrução de
> trabalho. Para o estado atual do sistema, leia
> [`../arquitetura.md`](../arquitetura.md).

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
docker compose run --rm server backend-cli task user:create `
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
respondendo 200. O `HEALTHCHECK` do `backend/Dockerfile` rodava
`backend-cli doctor` sem `--production`; nesse modo o `doctor` inclui
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
caminho é o do runbook `docs/historico/corte_backend_rust.md` §2 — `pg_dump` do banco Adonis,
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
- [x] 🟢 **Concluído** — `netmonitor_development.sqlite` e `netmonitor_test.sqlite{,-shm,-wal}` removidos.
      Nota: `cargo test` **recria** os `netmonitor_test.sqlite*` (é o banco da
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
      de `backend/` e os de npm do frontend.

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
`./backend` e `./frontend`.

- [x] 🟢 **Concluído** — reescrito para `.git`, `.env`, `node_modules`, `frontend/node_modules`, `frontend/dist`, `backend/target`, `.DS_Store`

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

- `.env.example` — "gere com: `node ace vpn:probe-register`" → `backend-cli task vpn_probe_register`
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
      `backend-cli task vpn_probe_register`; e a nota do `DataPrunerService`
      agora descreve o que o código faz — a purga roda **dentro** do ciclo do
      `scheduler_run` (`is_due("data_pruner", …)`), não num processo à parte.

---

## Fase 3 — Ferramentas de paridade dentro do `backend/`

Duas ferramentas existiram só para provar que o Rust reproduzia o Adonis. Com o
Adonis fora, elas não têm mais contra o quê rodar:

| Arquivo | Função | Ação |
| :--- | :--- | :--- |
| `backend/examples/parity_check.rs` | bate endpoint a endpoint contra `ADONIS_URL` | remover |
| `backend/examples/schema_parity.rs` | compara o esquema SeaORM com as migrations do Lucid (lê o código do backend AdonisJS) | remover |

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
- [x] 🟢 **Concluído** — `backend/AGENTS.md:76` já não manda rodar
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
| `docs/roadmap_vpn.md` | 5 | traduzir comandos `node ace` para `backend-cli task` |
| `docs/adr/001`, `005`, `006` | — | **manter como estão** — ADR é registro histórico; reescrever apaga a decisão |
| `docs/roadmap_melhorias.md`, `docs/roadmap_dispositivos_monitores_discovery.md` | 1 cada | ajuste pontual |

Regra de bolso: documento que descreve **como o sistema é hoje** (arquitetura,
diretrizes, roadmap ativo) precisa falar Rust. Documento que registra **uma
decisão ou um procedimento datado** (ADR, runbook do corte) fica intacto e vai
para `historico/`.

- [x] 🟢 **Concluído** — `docs/historico/` criado com os dois documentos da
      migração (movidos com `git mv`, conteúdo intacto) e um `README.md` que
      explica a regra de corte e como recuperar o código pela tag.
- [x] 🟢 **Concluído** — `docs/arquitetura.md` **reescrito**. Deixou de ser um
      projeto em tempo futuro ("será", "deverá") de um sistema AdonisJS e passou
      a descrever o que existe, em tempo presente: os 8 serviços, o ciclo do
      scheduler, a fila em `probe_tasks`, as 23 tabelas, a API real levantada dos
      controllers, o SSE via `event_outbox` e as fronteiras do módulo VPN.
      Ganhou uma seção **"O que não existe"** — worker, fila externa, agregação
      de métricas e auditoria —, porque metade da confusão do documento antigo
      vinha de descrever como pronto o que nunca foi construído.
- [x] 🟢 **Concluído** — `docs/diretrizes_testes.md` reescrito sobre `cargo test`,
      `#[serial]`, `request_with_config`, `insta` e os dois dialetos SQLite/Postgres.
- [x] 🟢 **Concluído** — `docs/base.md` (stack declarada nas §7.1, §7.2 e §16),
      `docs/roadmap.md` e `docs/roadmap_vpn.md` atualizados.
- [x] 🟢 **Concluído** — links conferidos por varredura de **todos** os links
      relativos de **todo** `.md` do repositório. Corrigidos:
      - 14 links para `adr/…` dentro do `roadmap_backend_rust.md`, que quebraram
        **por causa da movimentação** para `historico/` (agora `../adr/…`);
      - 20 links em `roadmap_vpn.md` e `roadmap_melhorias.md` que apontavam para
        arquivos `.ts` do backend removido — reapontados para os equivalentes em
        `backend/src/`. **Já estavam quebrados antes desta limpeza**;
      - 3 links `file:///d:/Projetos/...` absolutos no `roadmap.md`, que só
        funcionavam na máquina de quem os escreveu;
      - o link para `docs/diretrizes_qualidade_e_checklist.md` no `AGENTS.md` da
        raiz e no `.agents/AGENTS.md` — **esse arquivo nunca existiu**.

      Varredura final: **nenhum link relativo quebrado no repositório.**
- [x] 🟢 **Concluído** — `docs/roadmap_melhorias.md`: além do ajuste pontual, as
      tarefas **pendentes** citavam arquivos `.ts` a criar (`vpn_peer_dataset.ts`,
      `vpn_peer_state_watcher.ts`, `monitor_presenter.ts`). Num roadmap ativo isso
      mandaria alguém criar arquivo TypeScript no backend Rust; reapontados.

---

## Fase 5 — Instruções de agentes e comentários no código

### 5.1 `.agents/AGENTS.md` — desatualizado, e isso custa caro

Ainda manda rodar `npx tsc --noEmit` e `node ace test` como validação de
backend, e tem uma seção inteira de "Práticas de Teste no Japa". O `AGENTS.md`
da raiz já foi atualizado para `cargo fmt/clippy/test`; o de `.agents/` não.
Enquanto os dois divergirem, qualquer agente pode seguir o errado.

- [x] 🟢 **Decisão: virou ponteiro.** Em vez de reescrever os dois blocos, o
      arquivo passou a apontar para o `AGENTS.md` da raiz. Reescrever produziria
      uma segunda cópia correta *hoje* — e a divergência que causou este item
      nasceu exatamente assim. Com um ponteiro, não há o que divergir.
      Os dois itens abaixo ficam resolvidos por consequência:
- [x] 🟢 **Concluído** — bloco "Backend" com `npx tsc`/`node ace test` não existe mais
- [x] 🟢 **Concluído** — seção "Práticas de Teste no Japa" não existe mais; as
      diretrizes de teste do Rust estão em `docs/diretrizes_testes.md`

### 5.2 `AGENTS.md` (raiz)

O cabeçalho diz "Enquanto `backend/` existir, ele é referência de comportamento".
Depois da Fase 1 ele não existe mais.

- [x] 🟢 **Concluído** — parágrafo reescrito: fonte da verdade é `backend/`,
      ponto; histórico via tag `adonisjs-final` e `docs/historico/`. De quebra, o
      link para `docs/diretrizes_qualidade_e_checklist.md` (arquivo inexistente)
      foi trocado por `arquitetura.md`, e a referência à §18 do roadmap movido
      virou texto.

### 5.3 Frontend

- [x] 🟢 **Concluído** — `frontend/nginx.conf:24` reescrito
- [x] 🟢 **Concluído** — é gerado por `ts-rs`, sim. O doc comment foi corrigido em
      `src/views/vpn.rs` e o arquivo regerado por `cargo test`.
- [x] 🟢 **Concluído** — **não previsto no plano:** existem também
      `LucidMeta.ts` e `LucidPage.ts`, do mesmo gerador. Os comentários foram
      corrigidos em `services/shared/pagination.rs` e os arquivos regerados. Os
      **nomes dos tipos** ficaram: renomeá-los é refatoração que atravessa
      backend e frontend, não limpeza de resíduo. O doc do módulo agora explica
      que o nome é histórico e que o que obriga a manter o formato é o
      `useInfiniteList`, não o ORM que lhe deu nome.
- [x] 🟢 **Concluído** — varredura confirma: nenhuma menção a Adonis no `frontend/`.

### 5.4 Comentários no código Rust (baixa prioridade)

Cerca de 30 arquivos em `backend/src/` citam o AdonisJS. Eles se dividem em
dois tipos, e só um incomoda:

- **Explicam uma decisão** — ex.: `migration/src/lib.rs:42` ("`auth_access_tokens`
  existia por causa do `@adonisjs/auth`") justifica por que uma tabela *não*
  existe. Apagar isso perde informação. **Manter.**
- **Só apontam paridade** — ex.: `services/mod.rs:1` ("o equivalente de
  `backend/modules/**`"), `Dockerfile:68` ("o mesmo health check do backend
  AdonisJS"). Apontam para um caminho que não existe mais. **Reescrever.**

- [x] 🟢 **Concluído** — varredura feita em `backend` inteiro (não só `src/`
      e `Dockerfile`): também pegou `migration/`, `tests/` e o `AGENTS.md` do
      backend.
- [x] 🟢 **Concluído** — reescritos ~35 comentários do segundo tipo, em 25
      arquivos. O `Dockerfile:68` saiu junto com a correção do `HEALTHCHECK`
      (Fase 0). Dois nomes de **teste** também citavam o Adonis
      (`chaves_de_escopo_seguem_o_formato_do_adonis`,
      `limite_segue_a_regra_do_adonis`) — renomeados para descrever a regra em
      vez da origem dela.
- [x] 🟢 **Mantidos de propósito** (comentários do primeiro tipo, que explicam
      uma decisão):
      - `migration/src/lib.rs:42` — por que `auth_access_tokens` **não** existe;
      - `services/shared/crypto.rs:13` — aviso de que banco anterior ao corte tem
        segredo em formato ilegível aqui;
      - `tasks/vpn_secrets_import.rs` — o comando **existe** por causa daquela
        migração. O procedimento foi corrigido: citava `backend/` como "ainda
        vivo", e agora manda restaurá-lo pela tag primeiro.

      ⚠️ Isso contraria o critério literal da Fase 6 (`git grep -i adonis` só
      retornar `historico/` e `adr/`). A regra desta fase é mais específica e
      prevalece: apagar essas três explicações perderia informação que o código
      não consegue recuperar sozinho.

---

## Fase 6 — Verificação final

```powershell
# Nenhuma referência viva ao Adonis fora de docs/historico e docs/adr.
# Leia "viva" como "que aponta para algo que não existe mais": as menções que
# explicam uma decisão ficam, e estão listadas na Fase 5.4.
git grep -i -l "adonis" -- . ':!docs/historico' ':!docs/adr'

# Backend
cd backend
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

- [x] 🟢 `docker compose up` sobe os 8 serviços sem `migration` falhando
- [x] 🟢 `GET http://localhost:3333/` responde o health check
- [x] 🟢 Login funciona (`POST /api/auth/login` → 200; `/api/auth/me` com o token → 200)
- [x] 🟡 Um monitor executa e grava resultado — **o scheduler está no ciclo, sim**
      (dois resultados gravados nos horários exatos do intervalo de 10 s), mas a
      checagem em si falhou. Ver o achado abaixo.
- [x] 🟢 `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test` (396 testes)
      e `typecheck`/`lint`/`build` do frontend, todos verdes
- [x] 🟢 `git grep -i adonis` retorna `docs/historico/`, `docs/adr/`, este
      documento, e as três explicações mantidas de propósito (Fase 5.4) mais os
      dois ponteiros para a tag (`README.md`, `AGENTS.md`)
- [x] 🟢 `backend/` não existe em disco e nada no repositório aponta para ele
- [x] 🟢 **Acrescentado:** nenhum link relativo quebrado em nenhum `.md` do
      repositório (varredura automatizada)

### Achados que a verificação produziu

A Fase 6 não foi carimbo. Três defeitos apareceram e foram corrigidos —
`config/scheduler.yaml` com wrapper indevido, `JWT_SECRET` fora de base64
derrubando 100% dos logins, e `HEALTHCHECK` marcando todo container como
`unhealthy` (todos documentados na Fase 0). E um quarto apareceu, **não
corrigido**, descrito abaixo.

Nenhum é regressão desta limpeza. Todos estavam escondidos atrás do `migration`
que falhava: enquanto a stack não subia inteira, nada disso tinha como aparecer.

---

## 🟢 Resolvido — `shared_store` não era inicializado nos processos de tarefa

**Corrigido em 12/08/2026**, junto com um segundo defeito que a investigação
revelou (o relay de SSE no processo errado). A decisão está registrada na
[ADR 007](../adr/007-scheduler-processo-unico.md), que supersede a ADR 005.

Resumo do que mudou:

| Antes | Depois |
| :--- | :--- |
| Deps de processo num `Initializer` — que o `run_task` não executa | `Hooks::after_context`, o único gancho chamado em todos os modos |
| `scheduler` = subprocesso do Loco por tique | `task scheduler_loop`, laço num processo só |
| `relay_pending` dentro do `run_cycle` (nunca entregava) | Laço no `server`, subido pelo `MonitoringInitializer` |
| Cadências internas (`is_due`) mortas: tudo rodava a cada 5 s | Valem de verdade: VPN 10 s, tráfego 30 s, purga 1 h |
| Nenhum teste bootava fora do caminho do servidor | `tests/requests/process_deps.rs` cobre os dois defeitos |

O diagnóstico original fica abaixo, como registro.

---

### Diagnóstico original

### O sintoma

Um monitor de ping criado numa instalação limpa grava:

```
status  = unknown
message = "A checagem não pôde ser executada localmente: Cliente ICMP não inicializado"
```

E o `scheduler` registra, a cada 5 segundos, sem parar:

```
WARN falha ao retransmitir eventos  error="Barramento de eventos não inicializado"
```

### A causa

`MonitoringInitializer::before_run` (`src/initializers/monitoring.rs`) é quem
coloca `PingClient`, `ScanSessionService` e `EventBus` no `ctx.shared_store`. Ele
roda no boot do **servidor**.

Mas `scheduler` e `probe` não são o servidor: são
`backend-cli task scheduler_run` e `... task probe_run`, processos de
tarefa, onde os initializers do Loco não passam. Os três serviços compartilham o
mesmo `AppContext`, e nos dois de tarefa o `shared_store` está vazio.

Como `run_monitor` resolve o checker de ping por
`PingChecker::from_context(ctx)` (`services/monitoring/runner.rs:53`), a
consequência é direta.

### O que isso quebra

| Efeito | Alcance |
| :--- | :--- |
| **Nenhum monitor de ping funciona pelo scheduler** | O fallback local — aquele que o `AGENTS.md` marca como "**NÃO remover**" — está morto para ping. Instalação sem probe nunca vê um ping verde. |
| **Nenhum monitor de ping funciona pelo probe** | `probes/agent.rs:178` chama o mesmo `run_monitor`, no mesmo tipo de processo. |
| **O relay de SSE do scheduler nunca entrega** | Evento gerado no ciclo fica parado em `event_outbox`. A tela não recebe o que o scheduler produz — que é justamente mudança de estado de dispositivo e abertura de alerta. |

Os outros checkers (`tcp`, `http`, `dns`) não passam pelo `shared_store` e
seguem funcionando.

### Como foi corrigido

- [x] 🟢 `shared_store` populado em `Hooks::after_context` — ver
      `src/initializers/process_deps.rs`. A instalação do cliente ICMP é
      best-effort: o container `migration` não tem o sysctl e não precisa dele,
      e derrubar o `db migrate` por isso travaria a stack inteira.
- [x] 🟢 Scheduler virou laço em processo único, o que também fez as cadências
      internas do ciclo voltarem a valer (ADR 007).
- [x] 🟢 Relay movido para o servidor.
- [x] 🟢 `tests/requests/process_deps.rs` — dois testes que bootam pelo caminho
      do `task` (`create_context`), que é onde a suíte era cega.

---

## Ordem de execução

A Fase 0 é independente e desbloqueia o ambiente **agora** — faça primeiro. As
Fases 1→3 são mecânicas e podem ir num commit só. A Fase 4 é a mais demorada
(reescrita de `arquitetura.md`) e não bloqueia nada. A Fase 5.1 vale antecipar:
enquanto `.agents/AGENTS.md` mandar rodar `node ace test`, todo agente que ler
esse arquivo vai começar errado.

| Ordem | Fase | Bloqueia o quê | Estado |
| :---: | :--- | :--- | :---: |
| 1º | Fase 0 — banco | tudo; a stack não sobe | 🟢 |
| 2º | Fase 5.1 — `.agents/AGENTS.md` | qualidade de qualquer trabalho assistido | 🟢 |
| 3º | Fases 1–3 — disco, config, examples | nada, mas é o grosso do ruído | 🟢 |
| 4º | Fase 4 — docs | nada | 🟢 |
| 5º | Fase 5.2–5.4 — agentes, frontend, comentários | nada | 🟢 |
| 6º | Fase 6 — verificação | fecha o ciclo | 🟢 |

Executada nessa ordem, com uma inversão: a Fase 5.4 (comentários no código)
entrou junto com a 5.3, antes da Fase 4, porque a varredura que as duas exigem é
a mesma e fazê-la duas vezes seria desperdício.

---

## Apêndice — Revisão de resíduos (12/08/2026)

Revisão posterior ao arquivamento encontrou e corrigiu referências que
escaparam da varredura original:

- **Comentários no código Rust ainda apontando para arquivos `.ts` do Adonis:**
  - `backend/src/models/vpn_peers.rs` — reescrito para referenciar o
    comportamento do modelo original sem citar caminho inexistente.
  - `backend/src/services/discovery/cidr_range.rs` — removida a citação a
    `backend/modules/discovery/cidr_range.ts`.
  - `backend/src/services/shared/errors.rs` — reescrito para descrever a
    função sem apontar para `backend/modules/shared/errors.ts`.
  - `backend/src/services/vpn/profiles/mod.rs` — reescrito para não citar
    `backend/modules/vpn/profiles/*.ts`.

- **`docs/roadmap_vpn.md` — menções desatualizadas corrigidas:**
  - [x] 🟢 Removida a citação "backend AdonisJS, 84 testes" da rotina de
        validação.
  - [x] 🟢 Corrigida a descrição do `PingChecker`: a imagem de runtime do
        backend Rust é `debian:bookworm-slim`, não `node:24-alpine`, e o ping
        usa socket ICMP DGRAM (`surge-ping`).
  - [x] 🟢 QR code: a dependência `qrcode` está em
        `backend/Cargo.toml`, não no `package.json` da raiz.
  - [x] 🟢 Textos de links corrigidos de `*.ts` para `*.rs`
        (`key_generator.rs`, `peer_status.rs`, `secret_store.rs`,
        `access_control.rs`).

- **`docs/roadmap.md`:**
  - [x] 🟢 Item riscado sobre `PingChecker` na "imagem Alpine" ajustado para
        falar nos dois formatos de saída de latência, sem remeter a uma imagem
        específica do backend.

- **Verificações rodadas e aprovadas:**
  - [x] 🟢 `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`,
        `cargo test` (398 testes) e `cargo build --release`.
  - [x] 🟢 `npm --prefix frontend run typecheck`, `format`, `lint` e `build`.
  - [x] 🟢 Varredura automatizada: nenhum link relativo quebrado em `.md` do
        repositório (fora de `node_modules`/`target`).
  - [x] 🟢 `git grep` não encontrou mais menções a caminhos `.ts` do Adonis
        fora dos ponteiros históricos já documentados (`AGENTS.md`,
        `README.md`, ADRs e comentários que explicam decisões de migração).
