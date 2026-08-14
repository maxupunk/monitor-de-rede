# NetMonitor

Monitoramento de rede com backend em **Rust (Loco.rs)** e frontend em **Vue 3 +
Vuetify**. A stack inteira é **um container**: API, interface web, ciclo de
monitores, banco (SQLite) e servidor WireGuard.

## Estrutura

| Diretório | O que é | Toolchain |
| :--- | :--- | :--- |
| `backend/` | API, scheduler, probe e migrations | `cargo` |
| `frontend/` | SPA Vue 3, servida pela própria API em produção | `npm` |
| `docker/` | Entrypoint e watcher do WireGuard | — |
| `docs/` | Arquitetura, roadmaps e ADRs | — |

O `package.json` da raiz existe **só** para atalhos do frontend. Não há comando
de backend em npm: o backend é `cargo`, rodado de dentro de `backend/`.

## Subir a stack

```powershell
cp .env.example .env   # ajuste os segredos
docker compose up -d --build
```

Interface e API na mesma porta: <http://localhost:3333>. A UDP 51820 é do
WireGuard e só importa a quem usa a VPN.

### Primeiro acesso

O servidor aplica as migrations pendentes ao subir. Banco novo não tem usuário,
e é o próprio sistema que pede o cadastro: abra o endereço acima e ele leva a
`/setup`, onde se informa nome, e-mail, senha e o **token de instalação**.

O token sai no log do boot — a linha "instalação pendente":

```powershell
docker compose logs netmonitor | Select-String setup_token
```

Se a linha já rolou para fora do terminal:

```powershell
docker compose exec netmonitor backend-cli task auth_setup_token
```

Para fixá-lo de antemão (provisionamento automatizado), defina `SETUP_TOKEN` no
`.env`. Concluído o cadastro o token deixa de ser aceito, e novos usuários
passam a ser criados por quem já está autenticado.

## Backend — comandos (`cd backend`)

```powershell
cargo run --bin backend-cli -- start      # sobe a API
cargo run --bin backend-cli -- db migrate # aplica migrations
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Tarefas de CLI: `task auth_setup_token`, `task user:create`, `task probe_register`,
`task vpn_probe_register`, `task probe_run` (agente de um probe remoto),
`task scheduler_loop` (o ciclo em processo próprio, quando não se quer o
in-process) e `task scheduler_run` (um ciclo só, para depurar).

### Probe remoto

É o único caso que pede um segundo container — um agente onde o servidor não
alcança. Mesma imagem, outro comando:

```yaml
services:
  probe:
    image: netmonitor:latest
    command: ["backend-cli", "task", "probe_run"]
    environment:
      PROBE_SERVER_URL: http://IP-DO-SERVIDOR:3333
      PROBE_TOKEN: <token gerado por `task probe_register`>
    sysctls:
      net.ipv4.ping_group_range: "0 2147483647"
    restart: unless-stopped
```

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
do backend está em `backend/config/{development,test,production}.yaml`;
o `.env` apenas preenche os `get_env(...)` desses arquivos e as substituições do
compose. O banco é apontado por **`DATABASE_URL`** (uma URL só, não campos
separados): o padrão é o SQLite do volume, e apontar a variável para um
Postgres é o que basta para trocar — o código atende aos dois dialetos.

## Histórico

O backend anterior era AdonisJS. Ele foi removido do repositório e permanece
recuperável pela tag `adonisjs-final`; as decisões estão registradas em
`docs/adr/`.
