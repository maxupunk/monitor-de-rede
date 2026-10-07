//! Peças de SQL que não falam com servidor nenhum — e por isso têm teste de
//! unidade para cada caso: citação, ordem de dependência e a divisão de um
//! script em comandos.

pub mod order;
pub mod splitter;

use super::{DatabaseError, EngineFamily};

/// Cita um identificador vindo do catálogo, dobrando o delimitador interno.
///
/// Serve para nomes que **o servidor** devolveu (tabela, coluna), que podem ter
/// qualquer caractere.
#[must_use]
pub fn quote_ident(family: EngineFamily, name: &str) -> String {
    match family {
        EngineFamily::Postgres => format!("\"{}\"", name.replace('"', "\"\"")),
        EngineFamily::Mysql => format!("`{}`", name.replace('`', "``")),
    }
}

/// Literal de texto no padrão SQL (aspas simples dobradas).
///
/// Vale nos dois SGBDs para o texto que este módulo gera (nomes de sequência,
/// rótulos de enum): nenhum deles contém barra invertida que o MySQL leria
/// como escape — e, se contiver, ela é dobrada também.
#[must_use]
pub fn quote_literal(family: EngineFamily, value: &str) -> String {
    let escaped = value.replace('\'', "''");
    match family {
        EngineFamily::Postgres => format!("'{escaped}'"),
        EngineFamily::Mysql => format!("'{}'", escaped.replace('\\', "\\\\")),
    }
}

/// Valida um nome de banco que **o operador** digitou (criar, restaurar).
///
/// Mais estrito que [`quote_ident`] de propósito: este nome vai num `CREATE
/// DATABASE`, e um nome com espaço ou ponto e vírgula seria, na melhor das
/// hipóteses, um banco que ninguém consegue digitar depois.
///
/// # Errors
///
/// [`DatabaseError::InvalidName`] para qualquer coisa fora de
/// `[A-Za-z_][A-Za-z0-9_-]{0,62}`.
pub fn validate_database_name(name: &str) -> Result<&str, DatabaseError> {
    let valid = !name.is_empty()
        && name.len() <= 63
        && name
            .chars()
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if valid {
        Ok(name)
    } else {
        Err(DatabaseError::InvalidName(format!(
            "\"{name}\" — use letras, números, _ ou -, começando por letra"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cita_identificador_com_a_sintaxe_de_cada_sgbd() {
        assert_eq!(
            quote_ident(EngineFamily::Postgres, "pedidos"),
            "\"pedidos\""
        );
        assert_eq!(quote_ident(EngineFamily::Mysql, "pedidos"), "`pedidos`");
    }

    #[test]
    fn delimitador_dentro_do_nome_e_dobrado() {
        assert_eq!(quote_ident(EngineFamily::Postgres, "a\"b"), "\"a\"\"b\"");
        assert_eq!(quote_ident(EngineFamily::Mysql, "a`b"), "`a``b`");
    }

    #[test]
    fn literal_dobra_aspas_e_no_mysql_a_barra() {
        assert_eq!(quote_literal(EngineFamily::Postgres, "d'água"), "'d''água'");
        assert_eq!(quote_literal(EngineFamily::Mysql, "a\\b'c"), "'a\\\\b''c'");
    }

    #[test]
    fn recusa_nome_que_escaparia_do_ddl() {
        for hostile in [
            "app`; DROP DATABASE x; --",
            "app\"; DROP DATABASE x; --",
            "app nova",
            "app;x",
            "1app",
            "",
            "app'",
            &"a".repeat(64),
        ] {
            assert!(
                validate_database_name(hostile).is_err(),
                "aceitou {hostile:?}"
            );
        }
        for ok in ["app", "_app", "app_2", "app-restaurado", "App"] {
            assert!(validate_database_name(ok).is_ok(), "recusou {ok:?}");
        }
    }
}
