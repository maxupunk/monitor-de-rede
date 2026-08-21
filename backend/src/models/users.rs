use async_trait::async_trait;
use chrono::{offset::Local, Duration};
use loco_rs::{auth::jwt, hash, prelude::*};
use serde::{Deserialize, Serialize};
use serde_json::Map;
use uuid::Uuid;

pub use super::_entities::users::{self, ActiveModel, Entity, Model};

pub const MAGIC_LINK_LENGTH: i8 = 32;
pub const MAGIC_LINK_EXPIRATION_MIN: i8 = 5;
pub const RESET_TOKEN_EXPIRATION_MIN: i64 = 30;

/// Valida a política de senha do sistema: no mínimo 8 caracteres e ao menos 1 letra maiúscula.
pub fn validate_password(password: &str) -> std::result::Result<(), validator::ValidationError> {
    if password.len() < 8 {
        let mut err = validator::ValidationError::new("length");
        err.message = Some("A senha precisa ter ao menos 8 caracteres.".into());
        return Err(err);
    }
    if !password.chars().any(|c| c.is_ascii_uppercase()) {
        let mut err = validator::ValidationError::new("uppercase");
        err.message = Some("A senha precisa conter ao menos uma letra maiúscula.".into());
        return Err(err);
    }
    Ok(())
}

#[derive(Deserialize, Serialize)]
pub struct LoginParams {
    pub email: String,
    pub password: String,
}

impl std::fmt::Debug for LoginParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoginParams")
            .field("email", &self.email)
            .field("password", &"[REDACTED]")
            .finish()
    }
}

#[derive(Deserialize, Serialize, Validate)]
pub struct RegisterParams {
    #[validate(email(message = "Informe um e-mail válido."))]
    pub email: String,
    #[validate(custom(function = "validate_password"))]
    pub password: String,
    #[validate(length(min = 2, message = "O nome precisa ter ao menos 2 caracteres."))]
    pub name: String,
}

impl std::fmt::Debug for RegisterParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegisterParams")
            .field("email", &self.email)
            .field("name", &self.name)
            .field("password", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Validate, Deserialize)]
pub struct Validator {
    #[validate(length(min = 2, message = "O nome precisa ter ao menos 2 caracteres."))]
    pub name: String,
    #[validate(email(message = "Informe um e-mail válido."))]
    pub email: String,
}

impl Validatable for ActiveModel {
    fn validator(&self) -> Box<dyn Validate> {
        Box::new(Validator {
            name: self.name.as_ref().to_owned(),
            email: self.email.as_ref().to_owned(),
        })
    }
}

#[async_trait::async_trait]
impl ActiveModelBehavior for super::_entities::users::ActiveModel {
    async fn before_save<C>(self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        self.validate()?;
        if insert {
            let mut this = self;
            this.pid = ActiveValue::Set(Uuid::new_v4());
            this.api_key = ActiveValue::Set(format!("lo-{}", Uuid::new_v4()));
            Ok(this)
        } else {
            Ok(self)
        }
    }
}

#[async_trait]
impl Authenticable for Model {
    async fn find_by_api_key(db: &DatabaseConnection, api_key: &str) -> ModelResult<Self> {
        Self::find_by_api_key(db, api_key).await
    }

    async fn find_by_claims_key(db: &DatabaseConnection, claims_key: &str) -> ModelResult<Self> {
        Self::find_by_pid(db, claims_key).await
    }
}

impl Model {
    /// finds a user by the provided email
    ///
    /// # Errors
    ///
    /// When could not find user by the given token or DB query error
    pub async fn find_by_email(db: &DatabaseConnection, email: &str) -> ModelResult<Self> {
        let user = users::Entity::find()
            .filter(
                model::query::condition()
                    .eq(users::Column::Email, email)
                    .build(),
            )
            .one(db)
            .await?;
        user.ok_or_else(|| ModelError::EntityNotFound)
    }

    /// finds a user by the provided verification token
    ///
    /// # Errors
    ///
    /// When could not find user by the given token or DB query error
    pub async fn find_by_verification_token(
        db: &DatabaseConnection,
        token: &str,
    ) -> ModelResult<Self> {
        let user = users::Entity::find()
            .filter(
                model::query::condition()
                    .eq(users::Column::EmailVerificationToken, token)
                    .build(),
            )
            .one(db)
            .await?;
        user.ok_or_else(|| ModelError::EntityNotFound)
    }

    /// finds a user by the magic token and verify and token expiration
    ///
    /// # Errors
    ///
    /// When could not find user by the given token or DB query error ot token expired
    pub async fn find_by_magic_token(db: &DatabaseConnection, token: &str) -> ModelResult<Self> {
        let user = users::Entity::find()
            .filter(
                query::condition()
                    .eq(users::Column::MagicLinkToken, token)
                    .build(),
            )
            .one(db)
            .await?;

        let user = user.ok_or_else(|| ModelError::EntityNotFound)?;
        if let Some(expired_at) = user.magic_link_expiration {
            if expired_at >= Local::now() {
                Ok(user)
            } else {
                tracing::debug!(
                    user_pid = user.pid.to_string(),
                    token_expiration = expired_at.to_string(),
                    "magic token expired for the user."
                );
                Err(ModelError::msg("magic token expired"))
            }
        } else {
            tracing::error!(
                user_pid = user.pid.to_string(),
                "magic link expiration time not exists"
            );
            Err(ModelError::msg("expiration token not exists"))
        }
    }

    /// finds a user by the provided reset token
    ///
    /// # Errors
    ///
    /// When could not find user by the given token or DB query error
    pub async fn find_by_reset_token(db: &DatabaseConnection, token: &str) -> ModelResult<Self> {
        let user = users::Entity::find()
            .filter(
                model::query::condition()
                    .eq(users::Column::ResetToken, token)
                    .build(),
            )
            .one(db)
            .await?;

        let user = user.ok_or_else(|| ModelError::EntityNotFound)?;
        if let Some(sent_at) = user.reset_sent_at {
            if sent_at + Duration::minutes(RESET_TOKEN_EXPIRATION_MIN) >= Local::now() {
                Ok(user)
            } else {
                tracing::debug!(
                    user_pid = user.pid.to_string(),
                    "reset token expired for the user."
                );
                Err(ModelError::msg("reset token expired"))
            }
        } else {
            tracing::error!(
                user_pid = user.pid.to_string(),
                "reset token sent time does not exist"
            );
            Err(ModelError::msg("reset token sent time not exists"))
        }
    }

    /// finds a user by the provided pid
    ///
    /// # Errors
    ///
    /// When could not find user  or DB query error
    pub async fn find_by_pid(db: &DatabaseConnection, pid: &str) -> ModelResult<Self> {
        let parse_uuid = Uuid::parse_str(pid).map_err(|e| ModelError::Any(e.into()))?;
        let user = users::Entity::find()
            .filter(
                model::query::condition()
                    .eq(users::Column::Pid, parse_uuid)
                    .build(),
            )
            .one(db)
            .await?;
        user.ok_or_else(|| ModelError::EntityNotFound)
    }

    /// finds a user by the provided api key
    ///
    /// # Errors
    ///
    /// When could not find user by the given token or DB query error
    pub async fn find_by_api_key(db: &DatabaseConnection, api_key: &str) -> ModelResult<Self> {
        let user = users::Entity::find()
            .filter(
                model::query::condition()
                    .eq(users::Column::ApiKey, api_key)
                    .build(),
            )
            .one(db)
            .await?;
        user.ok_or_else(|| ModelError::EntityNotFound)
    }

    /// Verifies whether the provided plain password matches the hashed password
    ///
    /// # Errors
    ///
    /// when could not verify password
    #[must_use]
    pub fn verify_password(&self, password: &str) -> bool {
        hash::verify_password(password, &self.password)
    }

    /// Asynchronously creates a user with a password and saves it to the
    /// database.
    ///
    /// # Errors
    ///
    /// When could not save the user into the DB
    pub async fn create_with_password(
        db: &DatabaseConnection,
        params: &RegisterParams,
    ) -> ModelResult<Self> {
        Self::create_with_password_and_role(db, params, "admin").await
    }

    /// Cria um usuário já com o perfil informado na mesma transação.
    ///
    /// O chamador valida o vocabulário do perfil. Gravar o papel junto com a
    /// conta impede que uma falha intermediária deixe um novo usuário com o
    /// `admin` padrão da migration.
    pub async fn create_with_password_and_role(
        db: &DatabaseConnection,
        params: &RegisterParams,
        role: &str,
    ) -> ModelResult<Self> {
        validator::Validate::validate(params).map_err(|e| ModelError::Validation(e.into()))?;

        // Único ponto de gravação de usuário no sistema, e por isso o lugar
        // certo para normalizar o e-mail. `find_by_email` compara por igualdade
        // exata: sem isto, quem se cadastrasse como `Admin@Casa.com` nunca mais
        // entraria digitando `admin@casa.com`, e o mesmo endereço em duas
        // caixas diferentes passaria pela checagem de duplicidade abaixo.
        let email = params.email.trim().to_lowercase();
        let name = params.name.trim().to_string();

        let txn = db.begin().await?;

        if users::Entity::find()
            .filter(
                model::query::condition()
                    .eq(users::Column::Email, &email)
                    .build(),
            )
            .one(&txn)
            .await?
            .is_some()
        {
            return Err(ModelError::EntityAlreadyExists {});
        }

        let password_hash =
            hash::hash_password(&params.password).map_err(|e| ModelError::Any(e.into()))?;
        let user = users::ActiveModel {
            email: ActiveValue::set(email),
            password: ActiveValue::set(password_hash),
            name: ActiveValue::set(name),
            role: ActiveValue::set(role.to_string()),
            ..Default::default()
        }
        .insert(&txn)
        .await?;

        txn.commit().await?;

        Ok(user)
    }

    /// Creates a JWT
    ///
    /// # Errors
    ///
    /// when could not convert user claims to jwt token
    pub fn generate_jwt(&self, secret: &str, expiration: u64) -> ModelResult<String> {
        jwt::JWT::new(secret)
            .generate_token(expiration, self.pid.to_string(), Map::new())
            .map_err(ModelError::from)
    }
}

impl ActiveModel {
    /// Sets the email verification information for the user and
    /// updates it in the database.
    ///
    /// This method is used to record the timestamp when the email verification
    /// was sent and generate a unique verification token for the user.
    ///
    /// # Errors
    ///
    /// when has DB query error
    pub async fn set_email_verification_sent(
        mut self,
        db: &DatabaseConnection,
    ) -> ModelResult<Model> {
        self.email_verification_sent_at = ActiveValue::set(Some(Local::now().into()));
        self.email_verification_token = ActiveValue::Set(Some(Uuid::new_v4().to_string()));
        self.update(db).await.map_err(ModelError::from)
    }

    /// Sets the information for a reset password request,
    /// generates a unique reset password token, and updates it in the
    /// database.
    ///
    /// This method records the timestamp when the reset password token is sent
    /// and generates a unique token for the user.
    ///
    /// # Arguments
    ///
    /// # Errors
    ///
    /// when has DB query error
    pub async fn set_forgot_password_sent(mut self, db: &DatabaseConnection) -> ModelResult<Model> {
        self.reset_sent_at = ActiveValue::set(Some(Local::now().into()));
        self.reset_token = ActiveValue::Set(Some(Uuid::new_v4().to_string()));
        self.update(db).await.map_err(ModelError::from)
    }

    /// Records the verification time when a user verifies their
    /// email and updates it in the database.
    ///
    /// This method sets the timestamp when the user successfully verifies their
    /// email.
    ///
    /// # Errors
    ///
    /// when has DB query error
    pub async fn verified(mut self, db: &DatabaseConnection) -> ModelResult<Model> {
        self.email_verified_at = ActiveValue::set(Some(Local::now().into()));
        // Invalidate the verification token so it cannot be replayed after use.
        self.email_verification_token = ActiveValue::Set(None);
        self.update(db).await.map_err(ModelError::from)
    }

    /// Resets the current user password with a new password and
    /// updates it in the database.
    ///
    /// This method hashes the provided password and sets it as the new password
    /// for the user.
    ///
    /// # Errors
    ///
    /// when has DB query error or could not hashed the given password
    pub async fn reset_password(
        mut self,
        db: &DatabaseConnection,
        password: &str,
    ) -> ModelResult<Model> {
        validate_password(password).map_err(|e| {
            let mut errors = validator::ValidationErrors::new();
            errors.add("password", e);
            ModelError::Validation(errors.into())
        })?;
        self.password =
            ActiveValue::set(hash::hash_password(password).map_err(|e| ModelError::Any(e.into()))?);
        self.reset_token = ActiveValue::Set(None);
        self.reset_sent_at = ActiveValue::Set(None);
        self.update(db).await.map_err(ModelError::from)
    }

    /// Creates a magic link token for passwordless authentication.
    ///
    /// Generates a random token with a specified length and sets an expiration time
    /// for the magic link. This method is used to initiate the magic link authentication flow.
    ///
    /// # Errors
    /// - Returns an error if database update fails
    pub async fn create_magic_link(mut self, db: &DatabaseConnection) -> ModelResult<Model> {
        let random_str = hash::random_string(MAGIC_LINK_LENGTH as usize);
        let expired = Local::now() + Duration::minutes(MAGIC_LINK_EXPIRATION_MIN.into());

        self.magic_link_token = ActiveValue::set(Some(random_str));
        self.magic_link_expiration = ActiveValue::set(Some(expired.into()));
        self.update(db).await.map_err(ModelError::from)
    }

    /// Verifies and invalidates the magic link after successful authentication.
    ///
    /// Clears the magic link token and expiration time after the user has
    /// successfully authenticated using the magic link.
    ///
    /// # Errors
    /// - Returns an error if database update fails
    pub async fn clear_magic_link(mut self, db: &DatabaseConnection) -> ModelResult<Model> {
        self.magic_link_token = ActiveValue::set(None);
        self.magic_link_expiration = ActiveValue::set(None);
        self.update(db).await.map_err(ModelError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_de_login_nao_exibe_a_senha() {
        let params = LoginParams {
            email: "user@example.com".to_string(),
            password: "SenhaForte1".to_string(),
        };
        let debug = format!("{:?}", params);
        assert!(debug.contains("user@example.com"));
        assert!(!debug.contains("SenhaForte1"));
        assert!(debug.contains("[REDACTED]"));
    }

    #[test]
    fn debug_de_registro_nao_exibe_a_senha() {
        let params = RegisterParams {
            email: "user@example.com".to_string(),
            password: "SenhaForte1".to_string(),
            name: "User".to_string(),
        };
        let debug = format!("{:?}", params);
        assert!(debug.contains("user@example.com"));
        assert!(!debug.contains("SenhaForte1"));
        assert!(debug.contains("[REDACTED]"));
    }
}
