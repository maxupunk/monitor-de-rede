# Corte para o backend Rust — runbook da Fase 9

> Procedimento operacional do corte do `backend/` (AdonisJS) para o
> `backend-rust/` (Loco.rs). Complementa a [§15 Fase 9](roadmap_backend_rust.md#fase-9--corte-e-descomissionamento).
>
> **Este documento é para ser seguido com o sistema no ar.** Cada passo tem um
> critério de parada: se ele não for atendido, o corte não avança.

---

## 0. Pré-requisitos

| Item | Como conferir |
| :--- | :--- |
| `APP_KEY` idêntica nos dois backends | `grep APP_KEY docker-compose.yml` — é a chave que decifra os segredos da VPN |
| `JWT_SECRET` definido | O backend Rust não sobe sem ele em produção |
| Backup do Postgres | `pg_dump -Fc netmonitor > netmonitor-pre-corte.dump` |
| Suíte verde no Rust | `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` |

O esquema das duas versões é o mesmo (a Fase 1 provou isso com
`cargo run --example schema_parity`), **exceto** pelas divergências deliberadas
registradas ali — todas de tipo mais largo (`bigint` no lugar de `integer`), o
que aceita os dados existentes sem conversão.

---

## 1. Paridade de contrato (sem tocar em dados)

Sobe os dois backends contra **o mesmo banco**, em portas diferentes, e compara
endpoint a endpoint:

```sh
# AdonisJS na 3333 (como já está) e Rust na 3334
docker compose up -d server                    # AdonisJS, se ainda for o compose antigo
PORT=3334 DATABASE_URL=postgres://netmonitor:secret@localhost:5433/netmonitor \
  cargo run --release --bin backend_rust-cli -- start

ADONIS_URL=http://localhost:3333 \
RUST_URL=http://localhost:3334 \
PARITY_EMAIL=admin@monitor.local PARITY_PASSWORD=admin123 \
  cargo run --example parity_check
```

O comando sai com código ≠ 0 enquanto houver divergência, então serve de portão
em CI. Ele normaliza `id`, timestamps, chaves públicas e a ordem das chaves do
JSON — e **não** normaliza nome de campo, tipo, presença de chave nem formato de
data, que é justamente o que quebra tela.

**Critério de parada:** `0 divergências`, ou toda divergência restante
registrada na [§12](roadmap_backend_rust.md#12-ajustes-necessários-no-frontend)
com o patch de frontend correspondente.

---

## 2. Migração dos dados

O esquema é o mesmo, então o caminho é `pg_dump`/`pg_restore` direto. Duas
colunas **não** atravessam, e é por isso que existe o passo 2.2.

### 2.1 Cópia

```sh
pg_dump -Fc -h localhost -p 5433 -U netmonitor netmonitor > netmonitor.dump
pg_restore -h <destino> -U netmonitor -d netmonitor --clean --if-exists netmonitor.dump
```

Conferir depois do restore:

- `jsonb`: `SELECT count(*) FROM monitors WHERE configuration IS NULL;` deve ser 0;
- `bigint`: `SELECT max(id) FROM metrics;` cabe em `i64` (sempre cabe — o tipo
  novo é mais largo, não mais estreito).

### 2.2 Re-cifra dos segredos da VPN ⚠️

`vpn_servers.private_key_encrypted` e `vpn_peers.preshared_key_encrypted` estão
no formato do `encryption` do AdonisJS (AES-256-CBC + HMAC). O backend Rust usa
XChaCha20-Poly1305 (desvio **D6**). Os dois não se leem, e o `pg_restore` copia
o criptograma antigo intacto.

```sh
# 1) No backend/ AINDA VIVO — só ele sabe decifrar:
cd backend && node ace vpn:export-secrets > /tmp/vpn-secrets.json

# 2) No backend-rust/, com o banco já restaurado:
cd ../backend-rust
backend_rust-cli task vpn_secrets_import file:/tmp/vpn-secrets.json

# 3) Apague o arquivo — ele tem as chaves em texto claro:
shred -u /tmp/vpn-secrets.json
```

O `vpn_secrets_import` termina conferindo que **todo** segredo decifra com a
`APP_KEY` atual, e falha listando o que sobrou. Se a `APP_KEY` antiga se perdeu,
não há como recuperar: a saída é rotacionar
(`POST /api/vpn/peers/:id/rotate` em cada peer) e reconfigurar o servidor —
o que derruba os túneis e exige reaplicar os scripts nos equipamentos.

**Critério de parada:** `Todos os segredos da VPN decifram com a APP_KEY atual.`
e, na tela `/vpn`, cada peer gera artefato sem o aviso
`CHAVE-PRIVADA-INDISPONIVEL`.

---

## 3. Validação em sombra (um ciclo)

Os dois backends no ar contra o mesmo banco, por um ciclo de operação (sugestão:
24 h). O AdonisJS continua servindo o frontend; o Rust só observa.

Para isso, **desligue o scheduler de um dos dois** — os dois escrevendo
`monitor_results` dobrariam o histórico e disputariam o `next_run_at`:

```sh
docker compose stop scheduler          # o do AdonisJS
backend_rust-cli scheduler --config config/scheduler.yaml
```

O que comparar ao final da janela:

| Sinal | Onde olhar | Esperado |
| :--- | :--- | :--- |
| Alertas gerados | `SELECT alert_rule_id, scope_key, count(*) FROM alert_events WHERE created_at > now() - interval '24 hours' GROUP BY 1,2;` | Mesmo conjunto de (regra, alvo) que o ciclo anterior sob o AdonisJS |
| Resultados de monitor | `SELECT status, count(*) FROM monitor_results WHERE created_at > now() - interval '24 hours' GROUP BY 1;` | Proporção `up`/`down` equivalente |
| Túneis | Tela `/vpn` | `connectionStatus` acompanha o `wg show dump` |
| SSE | Console do navegador | Eventos chegando, sem `stream:resync` recorrente |

**Critério de parada:** nenhum alerta novo que o AdonisJS não teria gerado, e
nenhum alerta que ele geraria e o Rust não gerou.

---

## 4. Corte

1. `docker compose down`
2. O `docker-compose.yml` já aponta para `./backend-rust` nos cinco serviços
   (`migration`, `server`, `scheduler`, `probe`, `vpn-probe`) — feito na Fase 9.
3. `docker compose up -d --build`
4. Conferir que os oito serviços sobem saudáveis:
   `docker compose ps` — `migration` em `exited (0)`, o resto em `running`.
5. Ciclo ponta a ponta na tela: **descobrir** uma faixa → **cadastrar** um
   dispositivo → **monitorar** → **alertar** na queda → **notificar** →
   **resolver** na volta, tudo aparecendo em tempo real.

---

## 5. Descomissionamento do `backend/`

⚠️ **Passo irreversível — só depois de todos os critérios acima.**

```sh
git tag -a adonisjs-final -m "Último estado do backend AdonisJS antes do corte"
git push origin adonisjs-final
git rm -r backend/
git commit -m "chore: arquiva o backend AdonisJS (substituído por backend-rust)"
```

A tag é o que torna a remoção reversível: `git checkout adonisjs-final -- backend/`
traz tudo de volta.

**Antes de apagar, confira que nada mais aponta para lá:**

```sh
grep -rn "backend/" --include=*.yml --include=*.json --include=*.md . | grep -v backend-rust
```

---

## Índice de comandos (AdonisJS → Rust)

| Antes | Agora |
| :--- | :--- |
| `node ace migration:run` | `backend_rust-cli db migrate` |
| `node ace scheduler:run` | `backend_rust-cli scheduler --config config/scheduler.yaml` |
| `node ace probe:run` | `backend_rust-cli task probe_run` |
| `node ace probe:register --name=X` | `backend_rust-cli task probe_register name:X` |
| `node ace vpn:probe-register` | `backend_rust-cli task vpn_probe_register` |
| `node ace vpn:export-secrets` | *(só no AdonisJS — passo 2.2)* |
| — | `backend_rust-cli task vpn_secrets_import file:...` |
| `npm run lint && npm run typecheck` | `cargo fmt --check && cargo clippy --all-targets -- -D warnings` |
| `node ace test` | `cargo test` |
