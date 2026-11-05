//! O papel de um usuário (`admin`, `operator`, `viewer`): quem autoriza na
//! API e nas ferramentas da IA.

use std::str::FromStr;

use crate::shared::errors::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Admin,
    Operator,
    Viewer,
}

impl Role {
    pub const ADMIN: &'static str = "admin";
    pub const OPERATOR: &'static str = "operator";
    pub const VIEWER: &'static str = "viewer";

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Admin => Self::ADMIN,
            Self::Operator => Self::OPERATOR,
            Self::Viewer => Self::VIEWER,
        }
    }

    #[must_use]
    pub const fn can_write(self) -> bool {
        matches!(self, Self::Admin | Self::Operator)
    }

    #[must_use]
    pub const fn can_manage_users(self) -> bool {
        matches!(self, Self::Admin)
    }

    #[must_use]
    pub const fn can_manage_docker(self) -> bool {
        matches!(self, Self::Admin)
    }

    /// Plugins de dispositivo executam comandos no equipamento com a
    /// credencial dele — mesma régua do Docker.
    #[must_use]
    pub const fn can_manage_plugins(self) -> bool {
        matches!(self, Self::Admin)
    }
}

impl FromStr for Role {
    type Err = AppError;

    fn from_str(value: &str) -> AppResult<Self> {
        match value {
            Self::ADMIN => Ok(Self::Admin),
            Self::OPERATOR => Ok(Self::Operator),
            Self::VIEWER => Ok(Self::Viewer),
            _ => Err(AppError::validation(
                "Perfil inválido. Use admin, operator ou viewer.",
            )),
        }
    }
}
