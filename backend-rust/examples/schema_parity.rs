//! Verificador de paridade de esquema (§15 Fase 1).
//!
//! Compara o esquema **realmente criado** pelas migrations SeaORM com o que as
//! migrations do AdonisJS declaram. A comparação é contra o código-fonte do
//! backend atual, não contra uma transcrição minha — uma transcrição só provaria
//! que eu copiei minhas próprias suposições duas vezes.
//!
//! ```sh
//! # Postgres (recomendado — só ele reporta o tipo real de cada coluna)
//! DATABASE_URL=postgres://loco:loco@localhost:5433/netmonitor cargo run --example schema_parity
//!
//! # SQLite também funciona, com a ressalva abaixo
//! DATABASE_URL="sqlite://backend_rust_development.sqlite?mode=rwc" cargo run --example schema_parity
//! ```
//!
//! **Ressalva do SQLite:** a tipagem dele é dinâmica e o `AUTOINCREMENT` exige
//! `INTEGER PRIMARY KEY`, então toda chave sai declarada como `integer` mesmo
//! tendo sido criada como `bigint`. A verificação de *tipo* só é confiável no
//! Postgres; nomes de coluna, nulabilidade, índices e FKs são conferidos nos
//! dois.
//!
//! Sai com código 1 se houver divergência não declarada em [`DIVERGENCIAS`].

use std::collections::{BTreeMap, BTreeSet};

use sea_orm::{ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement};

// ---------------------------------------------------------------------------
// Divergências aceitas — cada uma com o motivo. Qualquer outra falha o script.
// ---------------------------------------------------------------------------

/// Tabelas que não são comparadas, com o porquê.
const TABELAS_IGNORADAS: &[(&str, &str)] = &[
    (
        "users",
        "§6 #01: fica a `users` do scaffold Loco (pid, api_key, tokens de \
         verificação/reset/magic-link), estendida com `active`. A do Adonis é \
         mais pobre e seria um retrocesso.",
    ),
    (
        "auth_access_tokens",
        "§10.2: a autenticação é `loco_rs::auth::JWT`, que não guarda token no \
         banco. A tabela existia por causa do `@adonisjs/auth`.",
    ),
];

/// Divergências de coluna aceitas: `(tabela, coluna, aspecto, motivo)`.
const DIVERGENCIAS: &[(&str, &str, &str, &str)] = &[
    (
        "monitor_results",
        "latency_ms",
        "tipo",
        "§5.3 define `latencyMs` como f64. O Adonis usa `float` (real/f32); ler \
         um f32 como f64 injeta ruído de precisão num número que a tela mostra \
         com casas decimais.",
    ),
    (
        "devices",
        "is_monitored",
        "nulabilidade",
        "O Adonis omitiu `.notNullable()` e o knex deixou a coluna anulável — \
         `NULL` aqui não tem significado distinto de `false`. NOT NULL DEFAULT \
         false elimina o terceiro estado e deixa a entidade `bool` em vez de \
         `Option<bool>`.",
    ),
    (
        "devices",
        "snmp_enabled",
        "nulabilidade",
        "Mesmo caso de `is_monitored`.",
    ),
];

/// Divergências que valem para **toda** tabela: `(coluna, aspecto, motivo)`.
const DIVERGENCIAS_GLOBAIS: &[(&str, &str, &str)] = &[(
    "updated_at",
    "nulabilidade",
    "O `timestamps_tz` do Loco cria `NOT NULL DEFAULT now()`; o Adonis deixou \
     anulável. Uma linha sempre tem um instante de última escrita — `null` aqui \
     só produz ramo morto em quem lê. É o padrão do framework e vale mais que a \
     cópia literal (ADR 006).",
)];

// ---------------------------------------------------------------------------
// Modelo do esquema
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
struct Coluna {
    familia: String,
    nullable: bool,
}

#[derive(Debug, Default)]
struct Tabela {
    colunas: BTreeMap<String, Coluna>,
    /// nome do índice → (colunas, é único)
    indices: BTreeMap<String, (Vec<String>, bool)>,
    /// coluna → (tabela referenciada, ação de ON DELETE em maiúsculas)
    fks: BTreeMap<String, (String, String)>,
}

type Esquema = BTreeMap<String, Tabela>;

/// Reduz os tipos de cada dialeto a famílias comparáveis.
fn familia(tipo: &str) -> String {
    let t = tipo.trim().to_lowercase();
    let t = t.split('(').next().unwrap_or(&t).trim().to_string();
    match t.as_str() {
        // knex
        "increments" | "bigincrements" | "integer" | "biginteger" => "int",
        "string" => "string",
        "text" => "text",
        "float" | "double" => "float",
        "boolean" => "bool",
        "jsonb" | "json" => "json",
        "timestamp" => "timestamp",
        // postgres
        "bigint" | "smallint" | "int4" | "int8" => "int",
        "character varying" | "varchar" => "string",
        "double precision" | "real" | "float8" | "float4" => "float",
        "timestamp with time zone" | "timestamp without time zone" | "timestamptz" => "timestamp",
        // sqlite (tipos declarados pelo sea-query)
        "jsonb_text" | "json_text" => "json",
        "timestamp_with_timezone_text" | "timestamp_text" | "datetime_text" => "timestamp",
        outro => outro,
    }
    .to_string()
}

// ---------------------------------------------------------------------------
// Lado AdonisJS: parsing das migrations .ts
// ---------------------------------------------------------------------------

/// Expressões do parser, compiladas uma vez só.
///
/// Compilar regex é caro e elas não mudam entre arquivos; deixá-las no laço
/// custaria uma compilação por declaração de coluna.
struct Padroes {
    comentario_bloco: regex::Regex,
    comentario_linha: regex::Regex,
    espacos: regex::Regex,
    nome_tabela: regex::Regex,
    divisor: regex::Regex,
    indice: regex::Regex,
    indice_nomeado: regex::Regex,
    unico: regex::Regex,
    unico_nomeado: regex::Regex,
    coluna: regex::Regex,
    in_table: regex::Regex,
    on_delete: regex::Regex,
    literal: regex::Regex,
}

impl Padroes {
    fn novo() -> Self {
        Self {
            comentario_bloco: regex::Regex::new(r"(?s)/\*.*?\*/").unwrap(),
            comentario_linha: regex::Regex::new(r"//[^\n]*").unwrap(),
            espacos: regex::Regex::new(r"\s+").unwrap(),
            nome_tabela: regex::Regex::new(r"tableName\s*=\s*'([^']+)'").unwrap(),
            divisor: regex::Regex::new(r"\btable\s*\.").unwrap(),
            indice: regex::Regex::new(r"^index\(\s*\[([^\]]*)\][^)]*\)").unwrap(),
            indice_nomeado: regex::Regex::new(r"^index\(\s*\[[^\]]*\]\s*,\s*'([^']+)'").unwrap(),
            unico: regex::Regex::new(r"^unique\(\s*\[([^\]]*)\]").unwrap(),
            unico_nomeado: regex::Regex::new(r"indexName:\s*'([^']+)'").unwrap(),
            coluna: regex::Regex::new(r"^(\w+)\(\s*'([^']+)'").unwrap(),
            in_table: regex::Regex::new(r"\.inTable\('([^']+)'\)").unwrap(),
            on_delete: regex::Regex::new(r"\.onDelete\('([^']+)'\)").unwrap(),
            literal: regex::Regex::new(r"'([^']+)'").unwrap(),
        }
    }

    /// Remove comentários para eles não confundirem o divisor de chamadas.
    fn sem_comentarios(&self, fonte: &str) -> String {
        let sem_bloco = self.comentario_bloco.replace_all(fonte, "");
        self.comentario_linha
            .replace_all(&sem_bloco, "")
            .to_string()
    }

    fn lista_de_strings(&self, bruto: &str) -> Vec<String> {
        self.literal
            .captures_iter(bruto)
            .map(|c| c[1].to_string())
            .collect()
    }
}

fn esquema_adonis(dir: &std::path::Path) -> Esquema {
    let re = Padroes::novo();
    let mut esquema = Esquema::new();

    let mut arquivos: Vec<_> = std::fs::read_dir(dir)
        .expect("ler as migrations do AdonisJS")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "ts"))
        .collect();
    arquivos.sort();

    for arquivo in arquivos {
        let fonte = re.sem_comentarios(&std::fs::read_to_string(&arquivo).expect("ler migration"));

        let Some(nome_tabela) = re.nome_tabela.captures(&fonte).map(|c| c[1].to_string()) else {
            continue;
        };

        let mut tabela = Tabela::default();

        // Cada `table.xxx(...)` abre uma declaração que segue encadeada até a
        // próxima. O espaço em branco é colapsado **antes** de dividir: o
        // formato do prettier quebra a linha entre `table` e `.integer(...)`,
        // e dividir no texto cru grudaria a declaração na anterior — foi assim
        // que a primeira versão deste script atribuiu metade das FKs à coluna
        // `id`.
        let uma_linha = re.espacos.replace_all(&fonte, " ").to_string();

        // A divisão é por regex, não pela literal `table.`: depois do colapso a
        // forma multilinha vira `table .integer(...)`, com espaço antes do
        // ponto. Dividir na literal deixaria essas declarações grudadas na
        // anterior — e era daí que vinham as "FKs da coluna id".
        for chamada in re.divisor.split(&uma_linha).skip(1) {
            if let Some(cap) = re.indice.captures(chamada) {
                let colunas = re.lista_de_strings(&cap[1]);
                let nome = re.indice_nomeado.captures(chamada).map_or_else(
                    || format!("{nome_tabela}_{}_index", colunas.join("_")),
                    |c| c[1].to_string(),
                );
                tabela.indices.insert(nome, (colunas, false));
                continue;
            }

            if let Some(cap) = re.unico.captures(chamada) {
                let colunas = re.lista_de_strings(&cap[1]);
                let nome = re.unico_nomeado.captures(chamada).map_or_else(
                    || format!("{nome_tabela}_{}_unique", colunas.join("_")),
                    |c| c[1].to_string(),
                );
                tabela.indices.insert(nome, (colunas, true));
                continue;
            }

            let Some(cap) = re.coluna.captures(chamada) else {
                continue;
            };
            let (tipo, coluna) = (cap[1].to_string(), cap[2].to_string());
            if !matches!(
                tipo.as_str(),
                "increments"
                    | "bigIncrements"
                    | "integer"
                    | "bigInteger"
                    | "string"
                    | "text"
                    | "float"
                    | "double"
                    | "boolean"
                    | "jsonb"
                    | "timestamp"
            ) {
                continue;
            }

            // O knex deixa a coluna anulável quando ninguém diz o contrário. As
            // chaves e as colunas com `.notNullable()` são as exceções.
            let nullable = !chamada.contains(".notNullable()")
                && !matches!(tipo.as_str(), "increments" | "bigIncrements");

            tabela.colunas.insert(
                coluna.clone(),
                Coluna {
                    familia: familia(&tipo),
                    nullable,
                },
            );

            if chamada.contains(".unique()") {
                tabela.indices.insert(
                    format!("{nome_tabela}_{coluna}_unique"),
                    (vec![coluna.clone()], true),
                );
            }

            if let Some(alvo) = re.in_table.captures(chamada) {
                let acao = re
                    .on_delete
                    .captures(chamada)
                    .map_or_else(|| "NO ACTION".to_string(), |c| c[1].to_uppercase());
                tabela.fks.insert(coluna, (alvo[1].to_string(), acao));
            }
        }

        esquema.insert(nome_tabela, tabela);
    }

    esquema
}

// ---------------------------------------------------------------------------
// Lado Rust: leitura do banco vivo
// ---------------------------------------------------------------------------

async fn linhas(db: &DatabaseConnection, sql: &str) -> Vec<sea_orm::QueryResult> {
    db.query_all_raw(Statement::from_string(db.get_database_backend(), sql))
        .await
        .expect("consultar o catálogo do banco")
}

async fn esquema_postgres(db: &DatabaseConnection) -> Esquema {
    let mut esquema = Esquema::new();

    for row in linhas(
        db,
        "SELECT table_name::text AS t, column_name::text AS c, data_type::text AS d, \
         is_nullable::text AS n FROM information_schema.columns \
         WHERE table_schema = 'public'",
    )
    .await
    {
        let tabela: String = row.try_get("", "t").unwrap();
        let coluna: String = row.try_get("", "c").unwrap();
        let tipo: String = row.try_get("", "d").unwrap();
        let nullable: String = row.try_get("", "n").unwrap();
        esquema.entry(tabela).or_default().colunas.insert(
            coluna,
            Coluna {
                familia: familia(&tipo),
                nullable: nullable == "YES",
            },
        );
    }

    for row in linhas(
        db,
        "SELECT t.relname::text AS tabela, i.relname::text AS indice, \
         ix.indisunique AS unico, a.attname::text AS coluna, \
         array_position(ix.indkey, a.attnum) AS pos \
         FROM pg_index ix \
         JOIN pg_class i ON i.oid = ix.indexrelid \
         JOIN pg_class t ON t.oid = ix.indrelid \
         JOIN pg_namespace n ON n.oid = t.relnamespace \
         JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = ANY(ix.indkey) \
         WHERE n.nspname = 'public' AND NOT ix.indisprimary \
         ORDER BY tabela, indice, pos",
    )
    .await
    {
        let tabela: String = row.try_get("", "tabela").unwrap();
        let indice: String = row.try_get("", "indice").unwrap();
        let unico: bool = row.try_get("", "unico").unwrap();
        let coluna: String = row.try_get("", "coluna").unwrap();
        let entrada = esquema
            .entry(tabela)
            .or_default()
            .indices
            .entry(indice)
            .or_insert_with(|| (Vec::new(), unico));
        entrada.0.push(coluna);
        entrada.1 = unico;
    }

    for row in linhas(
        db,
        "SELECT c.conrelid::regclass::text AS tabela, a.attname::text AS coluna, \
         c.confrelid::regclass::text AS alvo, c.confdeltype::text AS acao \
         FROM pg_constraint c \
         JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attnum = ANY(c.conkey) \
         WHERE c.contype = 'f'",
    )
    .await
    {
        let tabela: String = row.try_get("", "tabela").unwrap();
        let coluna: String = row.try_get("", "coluna").unwrap();
        let alvo: String = row.try_get("", "alvo").unwrap();
        let acao: String = row.try_get("", "acao").unwrap();
        // https://www.postgresql.org/docs/current/catalog-pg-constraint.html
        let acao = match acao.as_str() {
            "c" => "CASCADE",
            "n" => "SET NULL",
            "d" => "SET DEFAULT",
            "r" => "RESTRICT",
            _ => "NO ACTION",
        };
        esquema
            .entry(tabela)
            .or_default()
            .fks
            .insert(coluna, (alvo, acao.to_string()));
    }

    esquema
}

async fn esquema_sqlite(db: &DatabaseConnection) -> Esquema {
    let mut esquema = Esquema::new();

    let nomes: Vec<String> = linhas(
        db,
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
    )
    .await
    .into_iter()
    .map(|r| r.try_get::<String>("", "name").unwrap())
    .collect();

    for nome in nomes {
        let mut tabela = Tabela::default();

        for row in linhas(db, &format!("PRAGMA table_info(\"{nome}\")")).await {
            let coluna: String = row.try_get("", "name").unwrap();
            let tipo: String = row.try_get("", "type").unwrap();
            let notnull: i32 = row.try_get("", "notnull").unwrap();
            tabela.colunas.insert(
                coluna,
                Coluna {
                    familia: familia(&tipo),
                    nullable: notnull == 0,
                },
            );
        }

        for row in linhas(db, &format!("PRAGMA index_list(\"{nome}\")")).await {
            let indice: String = row.try_get("", "name").unwrap();
            if indice.starts_with("sqlite_autoindex") {
                continue;
            }
            let unico: i32 = row.try_get("", "unique").unwrap();
            let colunas: Vec<String> = linhas(db, &format!("PRAGMA index_info(\"{indice}\")"))
                .await
                .into_iter()
                .filter_map(|r| r.try_get::<Option<String>>("", "name").ok().flatten())
                .collect();
            tabela.indices.insert(indice, (colunas, unico == 1));
        }

        for row in linhas(db, &format!("PRAGMA foreign_key_list(\"{nome}\")")).await {
            let coluna: String = row.try_get("", "from").unwrap();
            let alvo: String = row.try_get("", "table").unwrap();
            let acao: String = row.try_get("", "on_delete").unwrap();
            tabela.fks.insert(coluna, (alvo, acao.to_uppercase()));
        }

        esquema.insert(nome, tabela);
    }

    esquema
}

// ---------------------------------------------------------------------------
// Comparação
// ---------------------------------------------------------------------------

fn divergencia_aceita(tabela: &str, coluna: &str, aspecto: &str) -> bool {
    DIVERGENCIAS
        .iter()
        .any(|(t, c, a, _)| *t == tabela && *c == coluna && *a == aspecto)
        || DIVERGENCIAS_GLOBAIS
            .iter()
            .any(|(c, a, _)| *c == coluna && *a == aspecto)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::var("DATABASE_URL").map_err(|_| {
        "defina DATABASE_URL (postgres://… de preferência; sqlite://… com ressalva de tipo)"
    })?;

    let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("raiz do repositório");
    let adonis = esquema_adonis(&raiz.join("backend/database/migrations"));

    let db = Database::connect(&url).await?;
    let compara_tipos = db.get_database_backend() == DatabaseBackend::Postgres;
    let rust = match db.get_database_backend() {
        DatabaseBackend::Postgres => esquema_postgres(&db).await,
        _ => esquema_sqlite(&db).await,
    };

    if !compara_tipos {
        println!(
            "aviso: backend SQLite — tipos não são conferidos (tipagem dinâmica). \
             Rode contra Postgres para a verificação completa.\n"
        );
    }

    let ignoradas: BTreeSet<&str> = TABELAS_IGNORADAS.iter().map(|(t, _)| *t).collect();
    let mut problemas: Vec<String> = Vec::new();
    let mut conferidas = 0;

    for (tabela, esperada) in &adonis {
        if ignoradas.contains(tabela.as_str()) {
            continue;
        }
        let Some(atual) = rust.get(tabela) else {
            problemas.push(format!("tabela `{tabela}` não existe no esquema Rust"));
            continue;
        };
        conferidas += 1;

        for (coluna, esperado) in &esperada.colunas {
            let Some(obtido) = atual.colunas.get(coluna) else {
                problemas.push(format!("{tabela}.{coluna}: coluna ausente"));
                continue;
            };
            if compara_tipos
                && esperado.familia != obtido.familia
                && !divergencia_aceita(tabela, coluna, "tipo")
            {
                problemas.push(format!(
                    "{tabela}.{coluna}: tipo `{}` != `{}`",
                    esperado.familia, obtido.familia
                ));
            }
            if esperado.nullable != obtido.nullable
                && !divergencia_aceita(tabela, coluna, "nulabilidade")
            {
                problemas.push(format!(
                    "{tabela}.{coluna}: nulabilidade {} != {}",
                    esperado.nullable, obtido.nullable
                ));
            }
        }

        for coluna in atual.colunas.keys() {
            if !esperada.colunas.contains_key(coluna) {
                problemas.push(format!("{tabela}.{coluna}: coluna a mais no esquema Rust"));
            }
        }

        for (indice, (colunas, unico)) in &esperada.indices {
            let Some((obtidas, obtido_unico)) = atual.indices.get(indice) else {
                problemas.push(format!(
                    "{tabela}: índice `{indice}` ({}) ausente",
                    colunas.join(", ")
                ));
                continue;
            };
            if colunas != obtidas {
                problemas.push(format!(
                    "{tabela}.{indice}: colunas [{}] != [{}]",
                    colunas.join(", "),
                    obtidas.join(", ")
                ));
            }
            if unico != obtido_unico {
                problemas.push(format!(
                    "{tabela}.{indice}: unicidade {unico} != {obtido_unico}"
                ));
            }
        }

        for (coluna, (alvo, acao)) in &esperada.fks {
            let Some((obtido_alvo, obtida_acao)) = atual.fks.get(coluna) else {
                problemas.push(format!("{tabela}.{coluna}: FK para `{alvo}` ausente"));
                continue;
            };
            if alvo != obtido_alvo {
                problemas.push(format!(
                    "{tabela}.{coluna}: FK aponta para `{obtido_alvo}`, esperado `{alvo}`"
                ));
            }
            if acao != obtida_acao {
                problemas.push(format!(
                    "{tabela}.{coluna}: ON DELETE `{obtida_acao}` != `{acao}`"
                ));
            }
        }
    }

    println!("{conferidas} tabelas conferidas contra as migrations do AdonisJS.");
    for (tabela, motivo) in TABELAS_IGNORADAS {
        println!("  ignorada `{tabela}`: {motivo}");
    }
    for (tabela, coluna, aspecto, motivo) in DIVERGENCIAS {
        println!("  divergência aceita em {tabela}.{coluna} ({aspecto}): {motivo}");
    }
    for (coluna, aspecto, motivo) in DIVERGENCIAS_GLOBAIS {
        println!("  divergência aceita em *.{coluna} ({aspecto}): {motivo}");
    }

    if problemas.is_empty() {
        println!("\nEsquema em paridade.");
        Ok(())
    } else {
        println!("\n{} divergência(s) NÃO declarada(s):", problemas.len());
        for problema in &problemas {
            println!("  - {problema}");
        }
        Err("esquema divergente".into())
    }
}
