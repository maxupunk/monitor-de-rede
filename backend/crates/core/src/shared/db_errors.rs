//! Classificação de erros do banco que valem nos dois dialetos (PostgreSQL e
//! SQLite), sem acoplar o serviço ao driver.

use sea_orm::{DbErr, SqlErr};

/// Violação de unicidade em PostgreSQL (23505) e SQLite
/// (`SQLITE_CONSTRAINT_UNIQUE`).
///
/// Primeiro pelo `SqlErr` do `sea-orm`, que lê o código do driver; o texto é
/// a rede de segurança para o erro que chega já convertido em mensagem
/// (`DbErr::Custom`, erro embrulhado por outra camada).
#[must_use]
pub fn is_unique_violation(error: &DbErr) -> bool {
    if matches!(error.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) {
        return true;
    }
    let message = error.to_string().to_lowercase();
    message.contains("23505")
        || message.contains("sqlite_constraint_unique")
        || message.contains("unique constraint")
        || message.contains("duplicate key")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconhece_colisao_de_unicidade_nos_dois_bancos() {
        assert!(is_unique_violation(&DbErr::Custom(
            "error returned from database: (code: 2067) UNIQUE constraint failed: devices.ip_address"
                .into()
        )));
        assert!(is_unique_violation(&DbErr::Custom(
            "duplicate key value violates unique constraint (23505)".into()
        )));
    }

    #[test]
    fn erro_comum_de_banco_nao_e_confundido_com_colisao() {
        assert!(!is_unique_violation(&DbErr::Custom(
            "connection closed".into()
        )));
        assert!(!is_unique_violation(&DbErr::RecordNotFound("sumiu".into())));
    }
}
