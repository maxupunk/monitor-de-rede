//! O erro da aplicação no fio e de volta.
//!
//! As conversões moram no núcleo, dono do [`AppError`]: o contrato de fio
//! (`agent-protocol`) não depende de crate nenhum do workspace.

use agent_protocol::{docker::DockerError, ErrorCode, RemoteError};

use crate::shared::errors::AppError;

/// Erro de um serviço do agente (motor de vazão, por exemplo) no fio. Regra de
/// negócio vai como `Engine` com a mensagem exata, para a central reconhecer
/// erros conhecidos (EC-SRP5, por exemplo) pelo texto.
impl From<AppError> for RemoteError {
    fn from(error: AppError) -> Self {
        let code = match &error {
            AppError::Validation(_) | AppError::ValidationFields(_) => ErrorCode::Validation,
            AppError::NotFound(_) => ErrorCode::NotFound,
            AppError::Conflict(_) | AppError::MissingCredential { .. } => ErrorCode::Conflict,
            AppError::Unauthorized(_) | AppError::Forbidden(_) => ErrorCode::Forbidden,
            AppError::ServiceUnavailable(_) | AppError::RateLimited { .. } => {
                ErrorCode::Unavailable
            }
            AppError::BusinessRule(_) => ErrorCode::Engine,
            AppError::Internal(_) => {
                return Self::new(ErrorCode::Internal, "Erro interno no agente");
            }
        };
        Self::new(code, error.to_string())
    }
}

impl From<RemoteError> for AppError {
    fn from(error: RemoteError) -> Self {
        match error.code {
            ErrorCode::Timeout | ErrorCode::Disconnected | ErrorCode::Unavailable => {
                Self::service_unavailable(error.message)
            }
            ErrorCode::Forbidden => Self::forbidden(error.message),
            ErrorCode::NotFound => Self::not_found(error.message),
            ErrorCode::Validation => Self::validation(error.message),
            _ => DockerError::from(error).into(),
        }
    }
}

impl From<DockerError> for AppError {
    fn from(error: DockerError) -> Self {
        match error {
            DockerError::Disabled | DockerError::Unavailable => {
                Self::service_unavailable(error.to_string())
            }
            DockerError::NotFound => Self::not_found(error.to_string()),
            DockerError::Conflict => Self::conflict(error.to_string()),
            DockerError::Validation(message) => Self::validation(message),
            DockerError::Forbidden(message) => Self::forbidden(message),
            DockerError::Unsupported(message) => Self::business_rule(message),
            DockerError::Engine => Self::Internal(anyhow::anyhow!(error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erro_de_servico_vira_codigo_do_protocolo_com_a_mensagem() {
        let rule = RemoteError::from(AppError::business_rule("EC-SRP5"));
        assert_eq!(rule.code, ErrorCode::Engine);
        assert_eq!(rule.message, "EC-SRP5");
        assert_eq!(
            RemoteError::from(AppError::conflict("ocupado")).code,
            ErrorCode::Conflict
        );
        assert_eq!(
            RemoteError::from(AppError::service_unavailable("mudo")).code,
            ErrorCode::Unavailable
        );
        let internal = RemoteError::from(AppError::Internal(anyhow::anyhow!("segredo")));
        assert_eq!(internal.code, ErrorCode::Internal);
        assert!(!internal.message.contains("segredo"));
    }
}
