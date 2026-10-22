//! `LIKE '%termo%'` com o termo do usuário escapado, igual em SQLite e PostgreSQL.

use sea_orm::sea_query::{Expr, ExprTrait, Func, IntoColumnRef, LikeExpr, SimpleExpr};

/// `%` e `_` do usuário são escapados: sem isso, procurar `pppoe_client`
/// casaria com `pppoe-client` e `pppoeXclient`, porque o `_` do SQL vale por
/// qualquer caractere — e sublinhado é corriqueiro em texto de log.
fn pattern(term: &str) -> LikeExpr {
    let escaped = term
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    LikeExpr::new(format!("%{escaped}%")).escape('\\')
}

/// A coluna contém o termo (com a caixa de cada dialeto).
#[must_use]
pub fn contains(column: impl IntoColumnRef, term: &str) -> SimpleExpr {
    Expr::col(column).like(pattern(term))
}

/// Substring sem caixa, com o mesmo resultado em SQLite e PostgreSQL:
/// `LOWER(coluna) LIKE '%termo%'`. O `LIKE` puro difere entre eles (o do
/// SQLite ignora caixa ASCII, o do Postgres não). Só é exato para termo
/// ASCII — o `LOWER` do SQLite não dobra acentos.
#[must_use]
pub fn contains_ignore_case(column: impl IntoColumnRef, term: &str) -> SimpleExpr {
    Expr::expr(Func::lower(Expr::col(column))).like(pattern(&term.to_lowercase()))
}
