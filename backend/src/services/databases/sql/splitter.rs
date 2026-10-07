//! Divide um script SQL em comandos, linha a linha.
//!
//! A restauração não manda o arquivo inteiro ao servidor: manda um comando de
//! cada vez, para mostrar o andamento, apontar **qual** comando falhou e não
//! segurar um dump de gigabytes na memória. Para isso é preciso saber onde um
//! comando termina — e o `;` só termina um comando fora de:
//!
//! * literais (`'...'`; no MySQL também `"..."`, ambos com escape por `\`;
//!   no PostgreSQL `E'...'` com escape e `'...'` sem);
//! * identificadores citados (`"..."` no PostgreSQL, `` `...` `` no MySQL);
//! * comentários (`--`, `/* */` — aninhados no PostgreSQL —, `#` no MySQL);
//! * corpos `$tag$ ... $tag$` do PostgreSQL (funções);
//! * blocos `DELIMITER ;;` do MySQL (procedures e triggers, cujo corpo tem `;`).
//!
//! É o mesmo formato que `psql` e `mysql` leem: um dump daqui restaura à mão,
//! e um script escrito à mão restaura daqui.

use crate::services::databases::EngineFamily;

#[derive(Debug, Clone, PartialEq, Eq)]
enum State {
    Normal,
    /// Literal entre aspas simples; `escapes` quando `\` escapa.
    Single {
        escapes: bool,
    },
    /// Aspas duplas: identificador no PostgreSQL, literal no MySQL.
    Double {
        escapes: bool,
    },
    Backtick,
    LineComment,
    Block {
        depth: u32,
    },
    Dollar(String),
}

/// Divisor incremental. Alimente linha a linha com [`push_line`](Self::push_line).
#[derive(Debug)]
pub struct StatementSplitter {
    family: EngineFamily,
    delimiter: String,
    buffer: String,
    state: State,
    /// O comando em curso tem algo além de espaço e comentário?
    has_content: bool,
}

impl StatementSplitter {
    #[must_use]
    pub fn new(family: EngineFamily) -> Self {
        Self {
            family,
            delimiter: ";".to_string(),
            buffer: String::new(),
            state: State::Normal,
            has_content: false,
        }
    }

    /// Consome uma linha (sem o `\n`) e devolve os comandos que ela fechou,
    /// já sem o delimitador.
    pub fn push_line(&mut self, line: &str) -> Vec<String> {
        if self.family == EngineFamily::Mysql && self.state == State::Normal && !self.has_content {
            if let Some(delimiter) = delimiter_directive(line) {
                self.delimiter = delimiter;
                self.buffer.clear();
                return Vec::new();
            }
        }

        let mut text = String::with_capacity(line.len() + 1);
        text.push_str(line);
        text.push('\n');

        let mut done = Vec::new();
        let mut index = 0;
        while index < text.len() {
            let rest = &text[index..];
            let c = rest.chars().next().expect("índice dentro do texto");
            let step = match &self.state {
                State::Normal => self.normal(rest, &mut done),
                State::Single { escapes } => self.quoted(rest, '\'', *escapes),
                State::Double { escapes } => self.quoted(rest, '"', *escapes),
                State::Backtick => self.quoted(rest, '`', false),
                // Comentário de linha não vai ao servidor: só a quebra fica.
                State::LineComment => {
                    if c == '\n' {
                        self.state = State::Normal;
                        self.take(rest, 1)
                    } else {
                        c.len_utf8()
                    }
                }
                State::Block { depth } => {
                    let depth = *depth;
                    if rest.starts_with("*/") {
                        self.state = if depth <= 1 {
                            State::Normal
                        } else {
                            State::Block { depth: depth - 1 }
                        };
                        self.take(rest, 2)
                    } else if rest.starts_with("/*") && self.family == EngineFamily::Postgres {
                        self.state = State::Block { depth: depth + 1 };
                        self.take(rest, 2)
                    } else {
                        self.take(rest, c.len_utf8())
                    }
                }
                State::Dollar(tag) => {
                    if rest.starts_with(tag.as_str()) {
                        let len = tag.len();
                        self.state = State::Normal;
                        self.take(rest, len)
                    } else {
                        self.take(rest, c.len_utf8())
                    }
                }
            };
            index += step;
        }
        done
    }

    /// O que sobrou no fim do arquivo, se for um comando sem delimitador final.
    pub fn finish(&mut self) -> Option<String> {
        let leftover = std::mem::take(&mut self.buffer);
        let had_content = std::mem::replace(&mut self.has_content, false);
        let statement = leftover.trim();
        (had_content && !statement.is_empty()).then(|| statement.to_string())
    }

    /// Copia `len` bytes de `rest` para o comando em curso.
    fn take(&mut self, rest: &str, len: usize) -> usize {
        self.buffer.push_str(&rest[..len]);
        len
    }

    fn normal(&mut self, rest: &str, done: &mut Vec<String>) -> usize {
        if rest.starts_with(self.delimiter.as_str()) {
            let statement = std::mem::take(&mut self.buffer);
            if std::mem::replace(&mut self.has_content, false) {
                let statement = statement.trim();
                if !statement.is_empty() {
                    done.push(statement.to_string());
                }
            }
            return self.delimiter.len();
        }

        let c = rest.chars().next().expect("resto não vazio");
        let mysql = self.family == EngineFamily::Mysql;
        match c {
            '\'' => {
                let escapes = mysql || self.ends_with_escape_prefix();
                self.has_content = true;
                self.state = State::Single { escapes };
            }
            '"' => {
                self.has_content = true;
                self.state = State::Double { escapes: mysql };
            }
            '`' if mysql => {
                self.has_content = true;
                self.state = State::Backtick;
            }
            '-' if rest.starts_with("--") && (!mysql || starts_mysql_comment(rest)) => {
                self.state = State::LineComment;
                return 2;
            }
            '#' if mysql => {
                self.state = State::LineComment;
                return 1;
            }
            '/' if rest.starts_with("/*") => {
                self.state = State::Block { depth: 1 };
                return self.take(rest, 2);
            }
            '$' if !mysql && !self.ends_with_ident_char() => {
                if let Some(tag) = dollar_tag(rest) {
                    let len = tag.len();
                    self.has_content = true;
                    self.state = State::Dollar(tag);
                    return self.take(rest, len);
                }
                self.has_content = true;
            }
            c if !c.is_whitespace() => self.has_content = true,
            _ => {}
        }
        self.take(rest, c.len_utf8())
    }

    /// Dentro de aspas: fecha no delimitador não dobrado e não escapado.
    fn quoted(&mut self, rest: &str, quote: char, escapes: bool) -> usize {
        let c = rest.chars().next().expect("resto não vazio");
        if escapes && c == '\\' {
            let next = rest[1..].chars().next().map_or(0, char::len_utf8);
            return self.take(rest, 1 + next);
        }
        if c == quote {
            // Delimitador dobrado é o próprio caractere, não o fim.
            if rest[1..].starts_with(quote) {
                return self.take(rest, 2);
            }
            self.state = State::Normal;
        }
        self.take(rest, c.len_utf8())
    }

    /// O literal que começa agora é `E'...'` (PostgreSQL, com escapes)?
    fn ends_with_escape_prefix(&self) -> bool {
        let mut chars = self.buffer.chars().rev();
        matches!(chars.next(), Some('E' | 'e'))
            && !chars
                .next()
                .is_some_and(|c| c.is_alphanumeric() || c == '_')
    }

    /// `$` colado a um identificador (`a$b`) não abre bloco.
    fn ends_with_ident_char(&self) -> bool {
        self.buffer
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric() || c == '_')
    }
}

/// No MySQL `--` só é comentário seguido de espaço ou fim de linha.
fn starts_mysql_comment(rest: &str) -> bool {
    matches!(
        rest[2..].chars().next(),
        Some(' ' | '\t' | '\n' | '\r') | None
    )
}

/// `$$` ou `$tag$` no começo de `rest`.
fn dollar_tag(rest: &str) -> Option<String> {
    let body = &rest[1..];
    let end = body.find('$')?;
    let name = &body[..end];
    let valid = name.is_empty()
        || (name
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_')
            && name.chars().all(|c| c.is_alphanumeric() || c == '_'));
    valid.then(|| format!("${name}$"))
}

/// `DELIMITER ;;` — diretiva do cliente MySQL, não do servidor.
fn delimiter_directive(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let (word, rest) = trimmed.split_once(char::is_whitespace)?;
    (word.eq_ignore_ascii_case("DELIMITER") && !rest.trim().is_empty())
        .then(|| rest.trim().to_string())
}

/// `COPY ... FROM stdin` — as linhas seguintes são dados até `\.`.
#[must_use]
pub fn is_copy_from_stdin(statement: &str) -> bool {
    let upper = statement.trim_start().to_ascii_uppercase();
    upper.starts_with("COPY ") && upper.trim_end().ends_with("FROM STDIN")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(family: EngineFamily, script: &str) -> Vec<String> {
        let mut splitter = StatementSplitter::new(family);
        let mut out = Vec::new();
        for line in script.lines() {
            out.extend(splitter.push_line(line));
        }
        out.extend(splitter.finish());
        out
    }

    const PG: EngineFamily = EngineFamily::Postgres;
    const MY: EngineFamily = EngineFamily::Mysql;

    #[test]
    fn separa_comandos_simples_e_varios_na_mesma_linha() {
        assert_eq!(
            split(PG, "SELECT 1; SELECT 2;\nSELECT 3;"),
            vec!["SELECT 1", "SELECT 2", "SELECT 3"]
        );
    }

    #[test]
    fn ponto_e_virgula_dentro_de_literal_nao_corta() {
        assert_eq!(
            split(PG, "INSERT INTO t VALUES ('a;b', 'd''água;');"),
            vec!["INSERT INTO t VALUES ('a;b', 'd''água;')"]
        );
        assert_eq!(
            split(MY, r"INSERT INTO t VALUES ('a\';b', 'c;');"),
            vec![r"INSERT INTO t VALUES ('a\';b', 'c;')"]
        );
    }

    #[test]
    fn literal_com_quebra_de_linha_atravessa_linhas() {
        // O `QUOTE()` do MySQL não escapa `\n`: o valor quebra a linha do dump.
        assert_eq!(
            split(MY, "INSERT INTO t VALUES ('linha 1;\nlinha 2');\nSELECT 1;"),
            vec!["INSERT INTO t VALUES ('linha 1;\nlinha 2')", "SELECT 1"]
        );
    }

    #[test]
    fn barra_invertida_so_escapa_no_mysql_e_no_e_do_postgres() {
        // No PostgreSQL padrão, `'a\'` é um literal completo com uma barra.
        assert_eq!(
            split(PG, r"SELECT 'a\'; SELECT 2;"),
            vec![r"SELECT 'a\'", "SELECT 2"]
        );
        assert_eq!(
            split(PG, r"SELECT E'a\';b'; SELECT 2;"),
            vec![r"SELECT E'a\';b'", "SELECT 2"]
        );
    }

    #[test]
    fn comentarios_nao_cortam_nem_viram_comando() {
        assert_eq!(
            split(
                PG,
                "-- cabeçalho; nada aqui\n/* bloco; /* aninhado; */ ainda; */\nSELECT 1;"
            ),
            vec!["/* bloco; /* aninhado; */ ainda; */\nSELECT 1"]
        );
        assert_eq!(
            split(MY, "# comentário;\n-- outro;\nSELECT 1;"),
            vec!["SELECT 1"]
        );
        // `--1` no MySQL é menos menos um, não comentário.
        assert_eq!(split(MY, "SELECT 2--1;"), vec!["SELECT 2--1"]);
    }

    #[test]
    fn corpo_de_funcao_em_dollar_quote_fica_inteiro() {
        let script = "CREATE FUNCTION f() RETURNS int LANGUAGE plpgsql AS $function$\n\
                      BEGIN\n  PERFORM 1; RETURN $$x;$$::int;\nEND;\n$function$;\nSELECT 2;";
        let out = split(PG, script);
        assert_eq!(out.len(), 2, "{out:?}");
        assert!(out[0].ends_with("$function$"));
        assert_eq!(out[1], "SELECT 2");
    }

    #[test]
    fn parametro_posicional_nao_abre_dollar_quote() {
        assert_eq!(
            split(PG, "SELECT $1; SELECT 2;"),
            vec!["SELECT $1", "SELECT 2"]
        );
    }

    #[test]
    fn identificadores_citados_com_delimitador() {
        assert_eq!(
            split(PG, r#"SELECT "a;b"""; SELECT 2;"#),
            vec![r#"SELECT "a;b""""#, "SELECT 2"]
        );
        assert_eq!(
            split(MY, "SELECT `a;b``c`; SELECT 2;"),
            vec!["SELECT `a;b``c`", "SELECT 2"]
        );
    }

    #[test]
    fn bloco_delimiter_do_mysql_mantem_o_corpo_da_procedure() {
        let script = "DELIMITER ;;\n\
                      CREATE PROCEDURE p() BEGIN\n  SELECT 1;\n  SELECT 2;\nEND ;;\n\
                      DELIMITER ;\nSELECT 3;";
        let out = split(MY, script);
        assert_eq!(out.len(), 2, "{out:?}");
        assert!(out[0].starts_with("CREATE PROCEDURE"));
        assert!(out[0].contains("SELECT 2;"));
        assert_eq!(out[1], "SELECT 3");
    }

    #[test]
    fn comando_sem_delimitador_no_fim_do_arquivo_nao_se_perde() {
        assert_eq!(
            split(PG, "SELECT 1;\nSELECT 2"),
            vec!["SELECT 1", "SELECT 2"]
        );
    }

    #[test]
    fn reconhece_o_copy_from_stdin() {
        assert!(is_copy_from_stdin(
            "COPY \"public\".\"t\" (\"a\", \"b\") FROM stdin"
        ));
        assert!(!is_copy_from_stdin("COPY t TO STDOUT"));
        assert!(!is_copy_from_stdin("SELECT 'COPY x FROM stdin'"));
    }
}
