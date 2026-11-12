# ADR 020 — PostgreSQL 18 como banco padrão do container (SQLite continua alternativa)

- **Status:** aceito — 2026-11-08, decidido com o responsável pelo produto.
- **Data:** 2026-11-08
- **Relacionado:** [ADR 008](008-syslog-parser.md) (volume do syslog),
  [ADR 017](017-arquitetura-modular.md).

## Contexto

Desde o início o container gravava em SQLite (`/data/netmonitor.sqlite`, em
WAL), e o PostgreSQL ficava para "instalações grandes" via `DATABASE_URL`. O
código já atendia aos dois: toda consulta vale nos dois dialetos, o CI tem um
job `test-postgres` e as entidades do `sea-orm` são geradas contra o Postgres.
Faltava só o container usar o PostgreSQL.

O perfil de carga do sistema é o que pesa na escolha:

- **escrita contínua e concorrente**: o scheduler grava resultados a cada ciclo
  (5 s), os polls SNMP gravam métricas, o syslog ingere linhas em rajada e as
  requisições da tela escrevem ao mesmo tempo;
- **séries temporais**: `metrics_1m`, histórico de monitores e logs, com
  retenção que apaga em blocos;
- **busca em texto** nos logs (FTS5 no SQLite, GIN no Postgres);
- instalação em **um host só**, por `docker compose`, operada por quem não é
  DBA.

O SQLite tem **um escritor por vez**. O `BEGIN IMMEDIATE`
(`shared::transaction::begin_write`) e o `busy_timeout` de 5 s seguram a fila,
mas, com o scheduler, o SNMP e o syslog escrevendo juntos, quem espera mais que
isso recebe `database is locked`, e o pool estoura em `Connection pool timed
out`, que chega à tela como 500. É o teto que motivou esta decisão.

## Opções avaliadas

| Banco | A favor | Contra | Veredito |
| :--- | :--- | :--- | :--- |
| **PostgreSQL 18** | MVCC: leitores e escritores não se bloqueiam, e várias escritas correm em paralelo. Já suportado e testado no CI; `GIN`/`tsvector` para os logs; `pg_dump` consistente com a base no ar; `JSONB`; I/O assíncrono no 18; caminho para TimescaleDB ou particionamento se o volume pedir. Licença PostgreSQL (permissiva). | Mais um container (~40 MB de RAM ocioso, ~430 MB de imagem alpine). Exige senha e um volume a mais. Atualizar de versão maior pede `pg_upgrade`. | **Escolhido** |
| MySQL 8 / MariaDB | Popular, bom em leitura simples. | **Nunca foi suportado aqui**: exigiria portar migrations, SQL bruto, FTS (o `FULLTEXT` do InnoDB não tem a mesma semântica), o realinhamento de sequências do backup e um terceiro dialeto em todo `match` de backend. DDL não é transacional (migration que falha no meio deixa o esquema pela metade). MySQL é GPL e da Oracle; o MariaDB divergiu dele. Nenhum ganho para esta carga. | Rejeitado |
| SQLite (como era) | Zero operação, um arquivo, backup por cópia, ótimo em leitura. | Um escritor só: o teto descrito acima. `VACUUM` para devolver disco; `auto_vacuum` só no arquivo vazio. | Mantido como **alternativa** (container único, desenvolvimento, testes) |
| TimescaleDB | Postgres com hipertabelas, compressão e retenção nativa: bom para métricas e logs. | Licença TSL (não OSI) nos recursos que interessam; dependência nova; muda as migrations. O volume atual (logs com teto de 4 GB, métricas por minuto) não pede isso. | Adiado: como é extensão do Postgres, fica aberto sem trocar de banco |
| ClickHouse / InfluxDB | Excelentes para séries temporais e logs em grande volume. | Não são bancos transacionais para o cadastro (usuários, dispositivos, peers da VPN): seriam um **segundo** banco a operar, sem FK nem transação entre os dois. | Rejeitados |

## Decisão

1. O `docker-compose.yml` ganha o serviço `postgres` (`postgres:18-alpine`),
   com volume próprio (`netmonitor-postgres`, montado em `/var/lib/postgresql`,
   o layout do 18), healthcheck por TCP e **sem porta publicada**. A API só
   sobe com ele saudável (`depends_on: service_healthy`).
2. A `DATABASE_URL` do container passa a ser
   `postgres://netmonitor:…@postgres:5432/netmonitor`, montada no próprio
   compose. Ela não é lida do `.env`, porque a do `.env` descreve a máquina.
3. **Os logs de syslog vão para um banco separado no mesmo servidor**
   (`netmonitor_logs`, criado por `docker/postgres/initdb/10-netmonitor-logs.sql`
   no primeiro boot), pela `SYSLOG_DB_URL`. Assim a ingestão e a retenção dos
   logs não incham o banco principal nem o backup dele, como já acontecia com o
   `logs.sqlite`.
4. **A senha nunca é padrão nem fica em branco**: o serviço `netmonitor-init`
   a sorteia no primeiro boot no volume `netmonitor-secrets` (ou grava a
   `POSTGRES_PASSWORD` do `.env`), com o `JWT_SECRET` e a `ENCRYPTION_KEY`. O
   `postgres` a lê por `POSTGRES_PASSWORD_FILE`; a API, pelo `PGPASSFILE` — as
   URLs vão sem senha. Autenticação `scram-sha-256`.
5. No modo host (`docker-compose.host.yml`) o Postgres também vai para a rede do
   host, mas **só no loopback** e na porta `POSTGRES_HOST_PORT` (padrão 15432,
   e não a 5432, que um Postgres do host costuma ocupar).
6. `DB_MAX_CONNECTIONS` do container sobe de 5 para 10.
7. **O SQLite continua suportado e testado**: o `production.yaml` mantém o
   SQLite como padrão para quem roda a imagem sem compose, o desenvolvimento
   local continua nele e a regra do AGENTS.md ("toda consulta vale nos dois
   dialetos") não muda.

## Consequências

- **Positivas:** escritas concorrentes sem `database is locked`; leituras longas
  (relatórios, exportação) não seguram o scheduler; `pg_dump` com a base no
  ar; busca nos logs por índice GIN; espaço do volume de logs devolvido pelo
  `autovacuum`, sem `VACUUM` manual.
- **Negativas:** um container e um volume a mais; a senha é mais um segredo a
  guardar; trocar a senha no `.env` depois do primeiro boot não troca a do
  banco (o `initdb` só roda com o volume vazio). Atualizar para o Postgres 19+
  exige `pg_upgrade` ou dump e restauração.
- **Migração de uma instalação SQLite existente:** o sistema ainda não foi para
  produção, então não há conversor automático (AGENTS.md §3). O backup do
  sistema (tela de Backup) é lógico e independente de dialeto: exporta no
  SQLite e restaura no Postgres, realinhando as sequências
  (`backup/service.rs`). Ele leva **a configuração** (sites, redes,
  dispositivos, monitores, regras, VPN, `system_settings`), mas **não** os
  usuários nem o histórico: depois da troca, o primeiro usuário é cadastrado de
  novo, e as métricas e os logs recomeçam do zero. A VPN só volta funcional com
  a mesma `ENCRYPTION_KEY`.
- **CI:** o job `test-postgres` passa a rodar no `postgres:18-alpine`, a mesma
  versão do container.
