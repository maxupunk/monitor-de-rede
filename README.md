# NetMonitor

Monitoramento de rede com backend em **Rust (Loco.rs)** e frontend em **Vue 3 +
Vuetify**. A stack completa sobe por `docker-compose.yml`.

## Estrutura

| Diretório | O que é | Toolchain |
| :--- | :--- | :--- |
| `backend-rust/` | API, scheduler, probe e migrations | `cargo` |
| `frontend/` | SPA Vue 3 servida por nginx | `npm` |
| `docker/` | Scripts do container WireGuard | — |
| `docs/` | Arquitetura, roadmaps e ADRs | — |

O `package.json` da raiz existe **só** para atalhos do frontend. Não há comando
de backend em npm: o backend é `cargo`, rodado de dentro de `backend-rust/`.

## Subir a stack

```powershell
cp .env.example .env   # ajuste os segredos
docker compose up -d --build
```

- Frontend: <http://localhost:8081>
- API: <http://localhost:3333>

O primeiro `up` roda o serviço `migration` antes do `server`. Em banco novo não
existe usuário — crie o administrador:

```powershell
docker compose run --rm server backend_rust-cli task user:create `
  email:admin@monitor.local name:"Admin" password:"troque-esta-senha"
```

## Backend — comandos (`cd backend-rust`)

```powershell
cargo run --bin backend_rust-cli -- start      # sobe a API
cargo run --bin backend_rust-cli -- db migrate # aplica migrations
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Tarefas de CLI: `task user:create`, `task probe_register`,
`task vpn_probe_register`, `task probe_run`, `task scheduler_loop` (o processo do
scheduler) e `task scheduler_run` (um ciclo só, para depurar).

## Frontend — comandos

```powershell
npm --prefix frontend run dev
npm --prefix frontend run typecheck
npm --prefix frontend run lint
npm --prefix frontend run build
```

Os mesmos atalhos existem na raiz como `npm run dev:frontend`, `build:frontend`,
`typecheck:frontend`, `lint:frontend` e `format:frontend`.

## Configuração

`.env.example` lista todas as variáveis lidas — e só elas. A configuração viva
do backend está em `backend-rust/config/{development,test,production}.yaml`;
o `.env` apenas preenche os `get_env(...)` desses arquivos e as substituições do
compose. O banco é apontado por **`DATABASE_URL`** (uma URL só, não campos
separados).

## Histórico

O backend anterior era AdonisJS. Ele foi removido do repositório e permanece
recuperável pela tag `adonisjs-final`; as decisões estão registradas em
`docs/adr/`.
