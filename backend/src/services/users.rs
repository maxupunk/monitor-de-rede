//! Regras de usuários e autorização por perfil.
//!
//! O controller HTTP só traduz entrada e saída. Normalização, proteção do
//! último administrador e política de acesso ficam aqui para serem usadas por
//! qualquer interface futura sem duplicar regra de negócio.

use std::str::FromStr;

use axum::http::Method;
use chrono::Local;
use loco_rs::{hash, model::ModelError};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, ModelTrait,
    PaginatorTrait, QueryFilter, QueryOrder, Set,
};
use validator::Validate;

use crate::{
    dtos::users::{CreateUserInput, UpdateUserInput},
    models::{
        _entities::users,
        users::{self as user_model, RegisterParams},
    },
    services::{
        auth::setup::map_model_error,
        shared::errors::{AppError, AppResult},
    },
};

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

/// Política única das rotas protegidas.
#[must_use]
pub fn request_is_allowed(role: Role, method: &Method, path: &str) -> bool {
    if path == "/api/users" || path.starts_with("/api/users/") {
        return role.can_manage_users();
    }

    if path == "/api/push" || path.starts_with("/api/push/") {
        return true;
    }

    if path == "/api/docker" || path.starts_with("/api/docker/") {
        if path.ends_with("/export") {
            return role.can_manage_docker();
        }
        return matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
            || role.can_manage_docker();
    }

    matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS) || role.can_write()
}

pub struct UserService<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> UserService<'a> {
    #[must_use]
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn list(&self) -> AppResult<Vec<users::Model>> {
        Ok(users::Entity::find()
            .order_by_asc(users::Column::Name)
            .all(self.db)
            .await?)
    }

    pub async fn find(&self, id: i64) -> AppResult<users::Model> {
        users::Entity::find_by_id(id)
            .one(self.db)
            .await?
            .ok_or_else(|| AppError::not_found("Usuário não encontrado."))
    }

    pub async fn create(&self, input: &CreateUserInput) -> AppResult<users::Model> {
        input.validate()?;
        let role = Role::from_str(input.role.trim())?;
        let user = user_model::Model::create_with_password_and_role(
            self.db,
            &RegisterParams {
                name: input.name.clone(),
                email: input.email.clone(),
                password: input.password.clone(),
            },
            role.as_str(),
        )
        .await
        .map_err(map_model_error)?;

        let mut active = user.into_active_model();
        active.active = Set(input.active.unwrap_or(true));
        active.email_verified_at = Set(Some(Local::now().into()));
        active.update(self.db).await.map_err(AppError::from)
    }

    pub async fn update(
        &self,
        actor_pid: &str,
        id: i64,
        input: &UpdateUserInput,
    ) -> AppResult<users::Model> {
        input.validate()?;
        let current = self.find(id).await?;
        let role = Role::from_str(input.role.trim())?;
        let active = input.active;

        if current.pid.to_string() == actor_pid && (role.as_str() != current.role || !active) {
            return Err(AppError::conflict(
                "Você não pode alterar seu próprio perfil nem desativar sua conta.",
            ));
        }

        self.ensure_active_admin_survives(&current, role, active)
            .await?;

        let email = input.email.trim().to_lowercase();
        let duplicate = users::Entity::find()
            .filter(users::Column::Email.eq(&email))
            .filter(users::Column::Id.ne(id))
            .one(self.db)
            .await?;
        if duplicate.is_some() {
            return Err(AppError::conflict("Já existe um usuário com este e-mail."));
        }

        let mut model = current.into_active_model();
        model.name = Set(input.name.trim().to_string());
        model.email = Set(email);
        model.role = Set(role.as_str().to_string());
        model.active = Set(active);

        if let Some(password) = input
            .password
            .as_deref()
            .filter(|password| !password.trim().is_empty())
        {
            user_model::validate_password(password).map_err(validation_error_for_password)?;
            model.password =
                Set(hash::hash_password(password)
                    .map_err(|error| AppError::Internal(error.into()))?);
        }

        model
            .update(self.db)
            .await
            .map_err(|error| map_model_error(ModelError::DbErr(error)))
    }

    pub async fn delete(&self, actor_pid: &str, id: i64) -> AppResult<()> {
        let current = self.find(id).await?;
        if current.pid.to_string() == actor_pid {
            return Err(AppError::conflict(
                "Você não pode excluir sua própria conta.",
            ));
        }
        self.ensure_active_admin_survives(&current, Role::Viewer, false)
            .await?;
        current.delete(self.db).await?;
        Ok(())
    }

    async fn ensure_active_admin_survives(
        &self,
        current: &users::Model,
        next_role: Role,
        next_active: bool,
    ) -> AppResult<()> {
        let removes_active_admin = current.active
            && current.role == Role::ADMIN
            && (!next_active || next_role != Role::Admin);
        if !removes_active_admin {
            return Ok(());
        }

        let active_admins = users::Entity::find()
            .filter(users::Column::Role.eq(Role::ADMIN))
            .filter(users::Column::Active.eq(true))
            .count(self.db)
            .await?;
        if active_admins <= 1 {
            return Err(AppError::conflict(
                "Mantenha ao menos um administrador ativo no sistema.",
            ));
        }
        Ok(())
    }
}

fn validation_error_for_password(error: validator::ValidationError) -> AppError {
    let mut errors = validator::ValidationErrors::new();
    errors.add("password", error);
    errors.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfis_possuem_capacidades_minimas() {
        assert!(Role::Admin.can_manage_users());
        assert!(Role::Admin.can_write());
        assert!(Role::Operator.can_write());
        assert!(!Role::Operator.can_manage_users());
        assert!(!Role::Viewer.can_write());
    }

    #[test]
    fn politica_bloqueia_escrita_do_visualizador_e_usuarios_do_operador() {
        assert!(request_is_allowed(
            Role::Viewer,
            &Method::GET,
            "/api/devices"
        ));
        assert!(!request_is_allowed(
            Role::Viewer,
            &Method::POST,
            "/api/devices"
        ));
        assert!(!request_is_allowed(
            Role::Operator,
            &Method::GET,
            "/api/users"
        ));
        assert!(request_is_allowed(
            Role::Admin,
            &Method::DELETE,
            "/api/users/2"
        ));
    }
}
