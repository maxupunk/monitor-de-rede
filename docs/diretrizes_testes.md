# 🧪 Diretrizes de Teste

> [!IMPORTANT]
> Todo recurso novo, refatoração ou correção de bug **DEVE** vir acompanhado de
> teste. E "concluído" significa que os quatro comandos abaixo passaram, rodados
> de dentro de `backend/`:
>
> ```powershell
> cargo fmt --all --check
> cargo clippy --all-targets -- -D warnings
> cargo test
> cargo build --release
> ```
>
> No frontend: `npm --prefix frontend run typecheck | format | lint | build`.

---

## 1. Onde cada teste mora

| Tipo | Localização | Escopo |
| :--- | :--- | :--- |
| **Unitário** | `#[cfg(test)] mod tests` **no próprio módulo** | Funções puras: parsers, cálculos, normalizações, formatação de mensagem. Sem banco, sem rede. |
| **De requisição** | `backend/tests/requests/` | Endpoints HTTP e fluxos completos com banco. |
| **De modelo** | `backend/tests/models/` | Regras de modelo e paginação, direto contra o banco. |
| **De convenção** | `backend/tests/conventions/` | Regras estruturais do código — hoje, `camelCase` em todo DTO. |
| **De tarefa** | `backend/tests/tasks/` | Comandos de CLI. |
| **Snapshot** | `insta`, ao lado do teste | Artefatos textuais: scripts de VPN, `wg0.conf`, payloads longos. |

Função pura tem teste no próprio arquivo. Isso não é preferência de
organização: é o que faz o teste ser lido junto com o código que ele descreve.

O alvo `tests/mod.rs` agrega os submódulos — daí `cargo test --test mod <filtro>`
para rodar um teste específico.

## 2. Regras de ouro

### 2.1 Isolamento de banco

Testes de requisição usam `request_with_config::<App, _, _>`, e o
`Hooks::truncate` limpa as 23 tabelas entre eles. A lista está em
`src/models/tables.rs` (`CREATION_ORDER`) — **tabela nova entra lá**, senão um
teste passa a depender da sujeira deixada pelo anterior. Há um teste que falha
se a lista divergir das migrations.

```rust
#[tokio::test]
#[serial]
async fn monitor_run_grava_resultado() {
    request_with_config::<App, _, _>(RequestConfig::default(), |request, ctx| async move {
        let user = prepare_data::init_user_login(&request, &ctx).await;
        let response = request.post("/api/monitors/1/run").await;
        assert_eq!(response.status_code(), 200);
    })
    .await;
}
```

### 2.2 `#[serial]` em tudo que toca estado global de processo

Obrigatório em testes que mexem em **variável de ambiente**, no
`ScanSessionService`, no cofre de chaves da VPN e no rate limiter. Sem ele, dois
testes paralelos disputam o mesmo estático e a falha aparece de forma
intermitente — o pior tipo de teste quebrado.

Quem seta variável de ambiente **remove no fim**, mesmo que o teste falhe no
meio.

### 2.3 Nada de rede externa

> [!WARNING]
> Nunca aponte um teste para `google.com`, `httpbin.org` ou qualquer serviço de
> terceiro. Isso troca uma verificação determinística por latência e disponibilidade
> alheias, e o teste passa a falhar por motivos que não são o código.

Use apenas `127.0.0.1` ou `localhost`. Testes de rede nativa (`ping`, socket)
têm teto de **5 segundos**.

### 2.4 Os dois dialetos

A suíte roda em **SQLite**; produção é **PostgreSQL**. Toda consulta precisa
valer nos dois. O ponto que mais morde: o SQLite reporta todo inteiro como
`INTEGER`, então `cargo loco db entities` rodado contra ele gera `i64` onde o
Postgres tem `INT4` — e aí o `sqlx` recusa a leitura em produção. **Gere
entidades sempre contra o PostgreSQL.**

### 2.5 Status codes explícitos

Valide o código exato (`200`, `201`, `204`, `404`, `422`), não "não deu erro".

### 2.6 Bindings TypeScript

Os arquivos de `frontend/src/bindings/` são gerados por `ts-rs` **durante
`cargo test`**. Não edite os `.ts` à mão: corrija o struct Rust e rode a suíte.

## 3. Antes de considerar pronto

```powershell
cd backend
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release

cd ..\frontend
npm run typecheck
npm run lint
npm run build
```

`cargo test` recria `netmonitor_test.sqlite*` em `backend/`. São
ignorados pelo git; não precisam ser removidos, só não versionados.
