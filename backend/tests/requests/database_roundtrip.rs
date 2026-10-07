//! Ida e volta contra servidores de verdade: dump nativo → restauração → os
//! dados conferem, tabela por tabela.
//!
//! Precisa de um servidor local, então só roda quando a variável aponta para
//! um (`host:porta:usuário:senha`, sempre `127.0.0.1`/`localhost`):
//!
//! ```text
//! NETMONITOR_TEST_POSTGRES=127.0.0.1:55432:postgres:senha
//! NETMONITOR_TEST_MYSQL=127.0.0.1:53307:root:senha
//! NETMONITOR_TEST_MARIADB=127.0.0.1:53306:root:senha
//! ```
//!
//! Sem a variável o teste passa sem fazer nada — a suíte não pode depender de
//! um banco que a máquina do desenvolvedor não tem.

use backend::services::databases::{
    dump::DumpWriter, restore::LineReader, DatabaseEngine, DatabaseTarget, NoProgress, RestoreMode,
    SslMode,
};
use sqlx::{AssertSqlSafe, Connection, MySqlConnection, PgConnection, Row};

fn target_from_env(variable: &str, engine: DatabaseEngine) -> Option<DatabaseTarget> {
    let raw = std::env::var(variable).ok()?;
    let mut parts = raw.splitn(4, ':');
    let host = parts.next()?.to_string();
    assert!(
        host == "127.0.0.1" || host == "localhost",
        "{variable}: só alvo local"
    );
    Some(DatabaseTarget {
        engine,
        host,
        port: parts.next()?.parse().ok()?,
        username: parts.next()?.to_string(),
        password: parts.next()?.to_string(),
        ssl_mode: SslMode::Disable,
        database: None,
    })
}

fn unique(prefix: &str) -> String {
    format!(
        "{prefix}_{}",
        &uuid::Uuid::new_v4().simple().to_string()[..8]
    )
}

/// Dump para um arquivo temporário e restauração dele.
async fn dump_and_restore(
    target: &DatabaseTarget,
    source: &str,
    destination: &str,
    mode: RestoreMode,
) -> (backend::services::databases::dump::DumpStats, String) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("dump.sql.gz");
    let mut writer = DumpWriter::create(&path).await.unwrap();
    let stats = target
        .engine
        .driver()
        .dump(target, source, &mut writer, &NoProgress)
        .await
        .unwrap_or_else(|error| panic!("dump falhou: {error}"));
    let file = writer.finish().await.unwrap();

    let reader = tokio::fs::File::open(&path).await.unwrap();
    let mut lines = LineReader::gzip(Box::pin(reader), Some(file.checksum.clone()));
    target
        .engine
        .driver()
        .restore(target, destination, mode, &mut lines, &NoProgress)
        .await
        .unwrap_or_else(|error| panic!("restauração falhou: {error}"));
    (stats, file.checksum)
}

// --- PostgreSQL --------------------------------------------------------------

async fn pg(target: &DatabaseTarget, database: &str) -> PgConnection {
    PgConnection::connect(&format!(
        "postgres://{}:{}@{}:{}/{database}",
        target.username, target.password, target.host, target.port
    ))
    .await
    .unwrap()
}

const PG_FIXTURE: &str = r#"
CREATE SCHEMA vendas;
CREATE EXTENSION IF NOT EXISTS pgcrypto;
CREATE TYPE vendas.status AS ENUM ('aberto', 'pago', 'cancelado');
CREATE DOMAIN vendas.email AS text CONSTRAINT email_tem_arroba CHECK (VALUE LIKE '%@%');
CREATE SEQUENCE vendas.protocolo START 1000;
CREATE TABLE vendas.clientes (
  id serial PRIMARY KEY,
  nome text NOT NULL,
  email vendas.email,
  criado_em timestamptz DEFAULT now(),
  extra jsonb,
  tags text[],
  codigo uuid DEFAULT gen_random_uuid()
);
CREATE TABLE vendas.pedidos (
  id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  cliente_id int NOT NULL REFERENCES vendas.clientes(id) ON DELETE CASCADE,
  status vendas.status NOT NULL DEFAULT 'aberto',
  total numeric(12,2) CHECK (total >= 0),
  total_com_imposto numeric(12,2) GENERATED ALWAYS AS (total * 1.1) STORED,
  foto bytea,
  protocolo int DEFAULT nextval('vendas.protocolo')
);
CREATE INDEX pedidos_status_idx ON vendas.pedidos (status) WHERE status <> 'cancelado';
CREATE UNIQUE INDEX clientes_email_idx ON vendas.clientes (lower(email));
CREATE TABLE public.medicoes (dia date NOT NULL, valor double precision) PARTITION BY RANGE (dia);
CREATE TABLE public.medicoes_2026 PARTITION OF public.medicoes FOR VALUES FROM ('2026-01-01') TO ('2027-01-01');
CREATE TABLE public.medicoes_outras PARTITION OF public.medicoes DEFAULT;
CREATE FUNCTION vendas.total_cliente(c int) RETURNS numeric LANGUAGE sql STABLE AS $$
  SELECT coalesce(sum(total), 0) FROM vendas.pedidos WHERE cliente_id = c
$$;
CREATE FUNCTION vendas.apara_nome() RETURNS trigger LANGUAGE plpgsql AS $f$
BEGIN
  NEW.nome := trim(NEW.nome); -- ponto e vírgula; dentro do corpo
  RETURN NEW;
END;
$f$;
CREATE TRIGGER clientes_apara BEFORE INSERT OR UPDATE ON vendas.clientes
  FOR EACH ROW EXECUTE FUNCTION vendas.apara_nome();
CREATE VIEW vendas.pagos AS SELECT * FROM vendas.pedidos WHERE status = 'pago';
CREATE VIEW vendas.resumo AS SELECT cliente_id, count(*) AS n FROM vendas.pagos GROUP BY cliente_id;
CREATE OR REPLACE VIEW vendas.pagos AS SELECT * FROM vendas.pedidos WHERE status = 'pago' AND total > 0;
CREATE MATERIALIZED VIEW vendas.ranking AS
  SELECT cliente_id, sum(total) AS soma FROM vendas.pedidos GROUP BY cliente_id;
CREATE INDEX ranking_soma_idx ON vendas.ranking (soma);
INSERT INTO vendas.clientes (nome, email, extra, tags) VALUES
  ('  Ana d''Ávila  ', 'ana@x.com', '{"vip": true, "obs": "linha\nquebrada"}', ARRAY['a', 'b;c']),
  (E'Barra \\ e tab\t e quebra\nde linha 🚀', 'b@x.com', NULL, NULL),
  ('Sem email', NULL, '[]', '{}');
INSERT INTO vendas.clientes (nome, email)
  SELECT 'cliente ' || g, 'c' || g || '@x.com' FROM generate_series(1, 3000) g;
INSERT INTO vendas.pedidos (cliente_id, status, total, foto)
  SELECT 1 + (g % 50), (ARRAY['aberto','pago','cancelado']::vendas.status[])[1 + g % 3],
         (g % 997)::numeric / 7, CASE WHEN g % 10 = 0 THEN '\x00ff00de'::bytea END
  FROM generate_series(1, 5000) g;
INSERT INTO public.medicoes SELECT d::date, random() FROM generate_series('2025-12-01'::date, '2026-02-01', '1 day') d;
REFRESH MATERIALIZED VIEW vendas.ranking;
SELECT nextval('vendas.protocolo');
"#;

const PG_FINGERPRINT_TABLES: [&str; 6] = [
    "vendas.clientes",
    "vendas.pedidos",
    "public.medicoes_2026",
    "public.medicoes_outras",
    "vendas.resumo",
    "vendas.ranking",
];

async fn pg_fingerprint(conn: &mut PgConnection) -> Vec<String> {
    let mut out = Vec::new();
    for table in PG_FINGERPRINT_TABLES {
        let hash: String = sqlx::query_scalar(AssertSqlSafe(format!(
            "SELECT md5(coalesce(string_agg(t::text, E'\\n' ORDER BY t::text), '')) FROM {table} t"
        )))
        .fetch_one(&mut *conn)
        .await
        .unwrap();
        out.push(format!("{table}={hash}"));
    }
    for probe in [
        "SELECT last_value::text FROM vendas.protocolo",
        "SELECT vendas.total_cliente(3)::text",
        "SELECT count(*)::text FROM pg_indexes WHERE schemaname = 'vendas'",
    ] {
        let value: String = sqlx::query_scalar(AssertSqlSafe(probe.to_string()))
            .fetch_one(&mut *conn)
            .await
            .unwrap();
        out.push(format!("{probe}={value}"));
    }
    out
}

#[tokio::test]
async fn postgres_ida_e_volta_preserva_estrutura_e_dados() {
    let Some(target) = target_from_env("NETMONITOR_TEST_POSTGRES", DatabaseEngine::Postgres) else {
        return;
    };
    let source = unique("nm_src");
    let copy = unique("nm_dst");
    let mut admin = pg(&target, "postgres").await;
    sqlx::raw_sql(AssertSqlSafe(format!("CREATE DATABASE {source}")))
        .execute(&mut admin)
        .await
        .unwrap();
    let mut conn = pg(&target, &source).await;
    sqlx::raw_sql(PG_FIXTURE).execute(&mut conn).await.unwrap();
    let expected = pg_fingerprint(&mut conn).await;

    let (stats, _) = dump_and_restore(&target, &source, &copy, RestoreMode::NewDatabase).await;
    assert_eq!(stats.tables, 5, "clientes, pedidos e as três de medições");
    assert_eq!(stats.rows, 3003 + 5000 + 63);

    let mut restored = pg(&target, &copy).await;
    assert_eq!(pg_fingerprint(&mut restored).await, expected);

    // Identidade, serial e trigger continuam vivos depois da restauração.
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO vendas.pedidos (cliente_id, total) VALUES (1, 10) RETURNING id",
    )
    .fetch_one(&mut restored)
    .await
    .unwrap();
    assert_eq!(id, 5001);
    let nome: String = sqlx::query_scalar(
        "INSERT INTO vendas.clientes (nome) VALUES ('  aparado  ') RETURNING nome",
    )
    .fetch_one(&mut restored)
    .await
    .unwrap();
    assert_eq!(nome, "aparado");
    let _ = restored.close().await;

    // Substituir: tudo ou nada, e o resultado é a cópia de novo.
    let (_, _) = dump_and_restore(&target, &source, &copy, RestoreMode::Replace).await;
    let mut replaced = pg(&target, &copy).await;
    assert_eq!(pg_fingerprint(&mut replaced).await, expected);
    let _ = replaced.close().await;
    let _ = conn.close().await;

    for database in [&source, &copy] {
        sqlx::raw_sql(AssertSqlSafe(format!(
            "DROP DATABASE {database} WITH (FORCE)"
        )))
        .execute(&mut admin)
        .await
        .unwrap();
    }
}

// --- MySQL / MariaDB ---------------------------------------------------------

async fn my(target: &DatabaseTarget, database: Option<&str>) -> MySqlConnection {
    MySqlConnection::connect(&format!(
        "mysql://{}:{}@{}:{}/{}",
        target.username,
        target.password,
        target.host,
        target.port,
        database.unwrap_or_default()
    ))
    .await
    .unwrap()
}

const MY_FIXTURE: [&str; 10] = [
    "CREATE TABLE clientes (
       id int AUTO_INCREMENT PRIMARY KEY,
       nome varchar(200) NOT NULL,
       email varchar(200),
       criado_em timestamp NULL DEFAULT CURRENT_TIMESTAMP,
       extra json,
       nivel enum('bronze','prata','ouro') DEFAULT 'bronze',
       foto blob,
       vazio varbinary(10),
       flags bit(8),
       preco decimal(10,2),
       taxa double,
       nome_maiusculo varchar(200) GENERATED ALWAYS AS (upper(nome)) VIRTUAL,
       UNIQUE KEY email_unico (email)
     ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
    "CREATE TABLE pedidos (
       id bigint AUTO_INCREMENT PRIMARY KEY,
       cliente_id int NOT NULL,
       total decimal(12,2),
       CONSTRAINT pedidos_cliente FOREIGN KEY (cliente_id) REFERENCES clientes (id) ON DELETE CASCADE
     ) ENGINE=InnoDB",
    "INSERT INTO clientes (nome, email, extra, nivel, foto, vazio, flags, preco, taxa) VALUES
       ('Ana d''Ávila', 'ana@x.com', '{\"vip\": true}', 'ouro', X'00FF00DE', X'', b'10100101', 10.50, 0.1),
       ('Barra \\\\ e ; quebra\nde linha 🚀', 'b@x.com', NULL, 'prata', NULL, NULL, NULL, NULL, 1e-300),
       ('Nulo \\0 e ctrl-z \\Z', NULL, '[]', NULL, '', X'01', b'0', -1.00, -2.5)",
    "INSERT INTO clientes (nome, email) SELECT CONCAT('cliente ', seq), CONCAT('c', seq, '@x.com')
       FROM (SELECT a.n + b.n * 10 + c.n * 100 + d.n * 1000 AS seq FROM
         (SELECT 0 n UNION SELECT 1 UNION SELECT 2 UNION SELECT 3 UNION SELECT 4 UNION SELECT 5 UNION SELECT 6 UNION SELECT 7 UNION SELECT 8 UNION SELECT 9) a,
         (SELECT 0 n UNION SELECT 1 UNION SELECT 2 UNION SELECT 3 UNION SELECT 4 UNION SELECT 5 UNION SELECT 6 UNION SELECT 7 UNION SELECT 8 UNION SELECT 9) b,
         (SELECT 0 n UNION SELECT 1 UNION SELECT 2 UNION SELECT 3 UNION SELECT 4 UNION SELECT 5 UNION SELECT 6 UNION SELECT 7 UNION SELECT 8 UNION SELECT 9) c,
         (SELECT 0 n UNION SELECT 1 UNION SELECT 2) d) s",
    "INSERT INTO pedidos (cliente_id, total) SELECT 1 + (id % 50), id / 7 FROM clientes",
    "CREATE VIEW v_pedidos_validos AS SELECT * FROM pedidos WHERE total > 0",
    // Vem antes em ordem alfabética e depende da outra: o dump precisa
    // criar `v_pedidos_validos` primeiro.
    "CREATE VIEW a_resumo AS SELECT cliente_id, COUNT(*) AS n, SUM(total) AS soma FROM v_pedidos_validos GROUP BY cliente_id",
    "CREATE FUNCTION dobro(x INT) RETURNS INT DETERMINISTIC RETURN x * 2",
    "CREATE PROCEDURE conta_pedidos(IN c INT, OUT n INT)
     BEGIN
       DECLARE tmp INT DEFAULT 0;
       SELECT COUNT(*) INTO tmp FROM pedidos WHERE cliente_id = c;
       SET n = tmp;
     END",
    "CREATE TRIGGER clientes_apara BEFORE INSERT ON clientes FOR EACH ROW
     BEGIN
       SET NEW.nome = TRIM(NEW.nome);
       IF NEW.nivel IS NULL THEN SET NEW.nivel = 'bronze'; END IF;
     END",
];

async fn my_fingerprint(conn: &mut MySqlConnection) -> Vec<String> {
    let mut out = Vec::new();
    for table in ["clientes", "pedidos"] {
        let row = sqlx::query(AssertSqlSafe(format!("CHECKSUM TABLE {table}")))
            .fetch_one(&mut *conn)
            .await
            .unwrap();
        let checksum: Option<i64> = row.try_get(1).ok();
        let count: i64 = sqlx::query_scalar(AssertSqlSafe(format!("SELECT COUNT(*) FROM {table}")))
            .fetch_one(&mut *conn)
            .await
            .unwrap();
        out.push(format!("{table}={checksum:?}/{count}"));
    }
    for probe in [
        "SELECT CAST(SUM(soma) AS CHAR) FROM a_resumo",
        "SELECT CAST(dobro(21) AS CHAR)",
        "SELECT CAST(AUTO_INCREMENT AS CHAR) FROM information_schema.TABLES WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'clientes'",
        "SELECT HEX(foto) FROM clientes WHERE id = 1",
        "SELECT CAST(nome AS CHAR) FROM clientes WHERE id = 3",
    ] {
        let row = sqlx::query(AssertSqlSafe(probe.to_string()))
            .fetch_one(&mut *conn)
            .await
            .unwrap();
        let value: Option<String> = row
            .try_get::<Option<String>, _>(0)
            .or_else(|_| {
                row.try_get::<Option<Vec<u8>>, _>(0)
                    .map(|bytes| bytes.map(|bytes| String::from_utf8_lossy(&bytes).into_owned()))
            })
            .unwrap();
        out.push(format!("{probe}={value:?}"));
    }
    out
}

async fn mysql_roundtrip(variable: &str, engine: DatabaseEngine) {
    let Some(target) = target_from_env(variable, engine) else {
        return;
    };
    let source = unique("nm_src");
    let copy = unique("nm_dst");
    let mut admin = my(&target, None).await;
    sqlx::raw_sql(AssertSqlSafe(format!(
        "CREATE DATABASE {source} CHARACTER SET utf8mb4"
    )))
    .execute(&mut admin)
    .await
    .unwrap();
    let mut conn = my(&target, Some(&source)).await;
    for statement in MY_FIXTURE {
        sqlx::raw_sql(AssertSqlSafe(statement.to_string()))
            .execute(&mut conn)
            .await
            .unwrap_or_else(|error| panic!("fixture: {error}\n{statement}"));
    }
    let expected = my_fingerprint(&mut conn).await;

    let (stats, _) = dump_and_restore(&target, &source, &copy, RestoreMode::NewDatabase).await;
    assert_eq!(stats.tables, 2);
    assert!(stats.rows > 6000, "linhas: {}", stats.rows);

    let mut restored = my(&target, Some(&copy)).await;
    assert_eq!(my_fingerprint(&mut restored).await, expected);
    // Procedure e trigger restaurados funcionam.
    sqlx::raw_sql("CALL conta_pedidos(3, @n)")
        .execute(&mut restored)
        .await
        .unwrap();
    sqlx::raw_sql("INSERT INTO clientes (nome) VALUES ('  aparado  ')")
        .execute(&mut restored)
        .await
        .unwrap();
    let nome: String = sqlx::query_scalar("SELECT nome FROM clientes ORDER BY id DESC LIMIT 1")
        .fetch_one(&mut restored)
        .await
        .unwrap();
    assert_eq!(nome, "aparado");
    let _ = restored.close().await;

    let (_, _) = dump_and_restore(&target, &source, &copy, RestoreMode::Replace).await;
    let mut replaced = my(&target, Some(&copy)).await;
    assert_eq!(my_fingerprint(&mut replaced).await, expected);
    let _ = replaced.close().await;
    let _ = conn.close().await;

    for database in [&source, &copy] {
        sqlx::raw_sql(AssertSqlSafe(format!("DROP DATABASE {database}")))
            .execute(&mut admin)
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn mysql_ida_e_volta_preserva_estrutura_e_dados() {
    mysql_roundtrip("NETMONITOR_TEST_MYSQL", DatabaseEngine::Mysql).await;
}

#[tokio::test]
async fn mariadb_ida_e_volta_preserva_estrutura_e_dados() {
    mysql_roundtrip("NETMONITOR_TEST_MARIADB", DatabaseEngine::Mariadb).await;
}
