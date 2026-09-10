# Correção 04 — Pool de conexões e o 401 que era falha de banco

- **Data:** 2026-09-09
- **Severidade:** alta (500 intermitentes e sessões derrubadas sem motivo)
- **Arquivos:** `backend/config/production.yaml`, `backend/config/development.yaml`,
  `.env`, `.env.example`, `docker-compose.yml`, `backend/src/controllers/auth_guard.rs`

## O defeito

### Um pool de uma conexão

`DB_MAX_CONNECTIONS` valia **1** em todos os lugares: default do `production.yaml` e do
`development.yaml`, valor fixo no `.env` e no `.env.example`, e default do
`docker-compose.yml`. O container de produção confirma (`DB_MAX_CONNECTIONS=1` no
ambiente).

O raciocínio original — "o SQLite tem escritor único, então uma conexão basta" — ignora
que o boot **liga o WAL** (`app::enable_sqlite_wal`). Com WAL, o SQLite atende vários
leitores em paralelo com o escritor, e dois escritores se serializam sozinhos pelo
`busy_timeout` de 5 s do sqlx. Um pool de uma conexão joga fora exatamente o que o WAL
oferece: **toda** operação passa a serializar, inclusive leitura.

Com `SCHEDULER_INTERVAL_SECONDS=5`, poll SNMP a cada 15 s por dispositivo, ingestão de
syslog e retransmissão de eventos, a disputa é contínua. Quem esperasse mais que o
`connect_timeout` (5 s) recebia `DbErr` — que vira `AppError::Internal` e sai como
**500**.

As evidências batem:

- `GET /api/auth/setup` respondendo **500** na tela de login (visível no DevTools);
- no log do container: `Failed to acquire connection from pool: Connection pool timed out`,
  em `backend::initializers::monitoring` e `backend::tasks::scheduler_run`;
- `slow statement: execution time exceeded alert threshold` num `INSERT` de `device_logs`
  levando 1,27 s.

### 401 para falha de banco

`auth_guard::require_jwt` colapsava dois casos muito diferentes num único 401:

```rust
let Ok(user) = users::Model::find_by_pid(&ctx.db, &claims.claims.pid).await else {
    // "usuário do JWT não existe no banco"  → 401
};
```

`find_by_pid` devolve `ModelError::EntityNotFound` quando o usuário sumiu, mas
`ModelError::DbErr` quando o **banco** falhou — inclusive no timeout do pool acima. Os
dois viravam 401.

E 401 tem consequência: `apiService.handleResponse` limpa o token e redireciona para o
login. Ou seja, **uma indisponibilidade de segundos deslogava quem estava trabalhando** —
o sintoma que abriu esta investigação.

## O que foi feito

### Teto do pool para 5

Alterado nos cinco pontos (`production.yaml`, `development.yaml`, `.env`, `.env.example`,
`docker-compose.yml`). Cinco é teto, não alvo: o pool só cresce sob disputa e
`min_connections` continua em 1.

O `test.yaml` **fica em 1** de propósito: a suíte roda serializada, com truncamento entre
casos, e mais conexões só acrescentariam variabilidade sem exercitar nada.

Os comentários dos arquivos foram reescritos — o antigo afirmava o contrário do que o
código faz desde que o WAL foi ligado, e comentário errado é pior que comentário nenhum.

### 503 para banco indisponível

`require_jwt` passou a distinguir os dois casos:

| Situação | Antes | Agora |
|---|---|---|
| `ModelError::EntityNotFound` | 401 | 401 (inalterado) |
| Qualquer outro erro (pool, I/O, lock) | 401 | **503** + `tracing::error!` |

O 503 preserva a sessão e diz ao cliente para tentar de novo. O log subiu de `warn` para
`error`: falha de banco no caminho de autenticação não é ruído.

## Validação

```
cargo clippy --all-targets -- -D warnings   # limpo
cargo fmt --all
cargo test    # 942 unitários + 307 de requisição, 0 falhas
```

## Como confirmar em produção

Depois do deploy, o que deve desaparecer do log:

```
Failed to acquire connection from pool: Connection pool timed out
```

E `GET /api/auth/setup` deve parar de devolver 500 na tela de login.

Se ainda aparecer sob carga, o próximo passo é `DB_CONNECT_TIMEOUT` maior (o gargalo
passa a ser escrita, não aquisição) — mas a hipótese é que o teto de 5 já resolva, porque
a disputa observada era por conexão, não por lock de escrita.

## Nota de escopo

A distinção 401/503 não estava entre os cinco itens combinados. Entrou aqui porque é o
mesmo defeito visto da outra ponta: sem ela, subir o pool reduziria a frequência dos
logouts espúrios sem eliminar a causa — qualquer falha de banco continuaria derrubando a
sessão.
