//! Cadastro do primeiro usuário — o bootstrap da instalação.
//!
//! # O problema
//!
//! Banco vazio não tem usuário, e sem usuário não existe login possível. As
//! duas saídas comuns são ruins: semear um `admin/admin123` deixa toda
//! instalação com a mesma credencial pública, e depender de
//! `cargo loco db seed` obriga quem instala a ter acesso ao terminal do
//! servidor antes de conseguir abrir a tela.
//!
//! # A saída daqui
//!
//! Enquanto não existir nenhum usuário, o sistema fica **em instalação**: o
//! frontend manda o operador para a tela de cadastro em vez da de login, e o
//! `POST /api/auth/setup` aceita nome, e-mail e senha — desde que acompanhados
//! do **token de instalação**. É ele que separa quem instalou o servidor de
//! quem apenas alcançou a porta 3333 primeiro.
//!
//! O token tem duas origens, nesta ordem de precedência:
//!
//! 1. [`SETUP_TOKEN_ENV`] no ambiente — decisão explícita de quem opera, e
//!    portanto vence sempre. É o caminho para provisionamento automatizado,
//!    em que o valor já é conhecido antes de o container subir.
//! 2. Sorteado no primeiro boot e guardado em `system_settings`. Sobrevive a
//!    reinício (senão o operador perderia o token entre ler o log e digitar) e
//!    aparece no log de boot e na task `auth:setup_token`.
//!
//! Concluído o cadastro, o token é revogado: o `/setup` deixa de existir na
//! prática, porque a primeira coisa que ele checa é se o banco ainda está
//! vazio.
//!
//! # Por que o primeiro usuário já nasce verificado
//!
//! O ciclo de verificação por e-mail depende de SMTP, e um servidor
//! recém-instalado normalmente ainda não tem um configurado — exigir o
//! round-trip travaria a instalação no passo seguinte ao cadastro. Quem
//! apresentou o token de instalação já provou ter acesso ao servidor, que é
//! uma prova mais forte do que a posse da caixa de e-mail.

use async_trait::async_trait;
use loco_rs::{hash, model::ModelError};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, PaginatorTrait, QueryFilter,
};
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::models::{
    _entities::{system_settings, users},
    system_settings::Model as SystemSetting,
    users::RegisterParams,
};
use crate::services::shared::{
    crypto::constant_time_eq,
    errors::{AppError, AppResult, FieldError},
};

/// Variável de ambiente que fixa o token de instalação.
pub const SETUP_TOKEN_ENV: &str = "SETUP_TOKEN";

/// Chave em `system_settings` onde o token sorteado é guardado.
pub const SETUP_TOKEN_KEY: &str = "auth.setup_token";

/// Tamanho do token sorteado. 32 caracteres alfanuméricos são ~190 bits — fora
/// de alcance para força bruta e ainda transcrevíveis à mão a partir do log.
const SETUP_TOKEN_LENGTH: usize = 32;

/// Dados do cadastro inicial.
///
/// A validação mora no próprio DTO porque ela é a mesma em qualquer chamador —
/// o handler HTTP hoje, uma task amanhã. `AppError` sabe converter
/// [`validator::ValidationErrors`] no corpo por campo que o frontend lê.
#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct SetupParams {
    #[validate(length(min = 2, message = "O nome precisa ter ao menos 2 caracteres."))]
    pub name: String,
    #[validate(email(message = "Informe um e-mail válido."))]
    pub email: String,
    #[validate(length(min = 8, message = "A senha precisa ter ao menos 8 caracteres."))]
    pub password: String,
    #[validate(length(min = 1, message = "Informe o token de instalação."))]
    pub token: String,
}

/// Como o operador descobre o token vigente. Serve ao log de boot e à task,
/// que precisam dizer *onde* olhar sem imprimir o segredo quando ele é do
/// operador.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupTokenOrigin {
    /// Veio de [`SETUP_TOKEN_ENV`].
    Environment,
    /// Sorteado pelo sistema e guardado em `system_settings`.
    Generated,
}

/// De onde sai o token exigido no primeiro acesso.
///
/// É um trait, e não um `match` embutido no serviço, porque as duas origens
/// diferem em tudo que importa — uma persiste, a outra não; uma pode ser
/// revogada, a outra pertence a quem operou o deploy. Uma terceira origem
/// (cofre externo, por exemplo) entra implementando este trait, sem tocar em
/// [`SetupService`].
#[async_trait]
pub trait SetupTokenStore: Send + Sync {
    /// Token vigente, criando-o se ainda não existir.
    ///
    /// # Errors
    ///
    /// Propaga erro do banco quando o token precisa ser persistido.
    async fn current(&self) -> AppResult<String>;

    /// Invalida o token após o cadastro concluído.
    ///
    /// # Errors
    ///
    /// Propaga erro do banco.
    async fn revoke(&self) -> AppResult<()>;

    /// Origem, para as mensagens de log e da CLI.
    fn origin(&self) -> SetupTokenOrigin;
}

/// Token fixado pelo operador em [`SETUP_TOKEN_ENV`].
pub struct EnvSetupToken {
    token: String,
}

impl EnvSetupToken {
    #[must_use]
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
        }
    }
}

#[async_trait]
impl SetupTokenStore for EnvSetupToken {
    async fn current(&self) -> AppResult<String> {
        Ok(self.token.clone())
    }

    /// Sem efeito, de propósito: o processo não reescreve o ambiente de quem o
    /// iniciou. Quem fixou a variável a remove quando quiser — e o `/setup` já
    /// está fechado de qualquer forma, porque o banco deixou de estar vazio.
    async fn revoke(&self) -> AppResult<()> {
        Ok(())
    }

    fn origin(&self) -> SetupTokenOrigin {
        SetupTokenOrigin::Environment
    }
}

/// Token sorteado pelo sistema, persistido em `system_settings`.
pub struct GeneratedSetupToken<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> GeneratedSetupToken<'a> {
    #[must_use]
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl SetupTokenStore for GeneratedSetupToken<'_> {
    async fn current(&self) -> AppResult<String> {
        if let Some(row) = SystemSetting::get(self.db, SETUP_TOKEN_KEY).await? {
            if let Some(token) = row.value.filter(|value| !value.trim().is_empty()) {
                return Ok(token);
            }
        }

        // Duas requisições simultâneas em banco vazio podem sortear valores
        // diferentes e uma sobrescrever a outra. Não é problema na prática: o
        // boot materializa o token antes de a porta abrir, e o perdedor da
        // corrida só teria gravado um token que ninguém chegou a ler.
        let token = hash::random_string(SETUP_TOKEN_LENGTH);
        SystemSetting::set(self.db, SETUP_TOKEN_KEY, Some(token.clone())).await?;
        Ok(token)
    }

    async fn revoke(&self) -> AppResult<()> {
        system_settings::Entity::delete_many()
            .filter(system_settings::Column::Key.eq(SETUP_TOKEN_KEY))
            .exec(self.db)
            .await?;
        Ok(())
    }

    fn origin(&self) -> SetupTokenOrigin {
        SetupTokenOrigin::Generated
    }
}

/// Escolhe a origem do token conforme o ambiente.
///
/// Um `SETUP_TOKEN` presente porém em branco conta como ausente: é o resultado
/// de um `SETUP_TOKEN=` esquecido num `.env`, e aceitá-lo como token válido
/// deixaria a instalação aberta a qualquer um que mandasse string vazia.
#[must_use]
pub fn token_store(db: &DatabaseConnection) -> Box<dyn SetupTokenStore + '_> {
    match std::env::var(SETUP_TOKEN_ENV) {
        Ok(value) if !value.trim().is_empty() => Box::new(EnvSetupToken::new(value.trim())),
        _ => Box::new(GeneratedSetupToken::new(db)),
    }
}

/// Orquestra o cadastro inicial: decide se ele ainda é permitido, confere o
/// token e cria o usuário.
pub struct SetupService<'a> {
    db: &'a DatabaseConnection,
    tokens: Box<dyn SetupTokenStore + 'a>,
}

impl<'a> SetupService<'a> {
    /// Serviço com a origem de token que o ambiente indicar.
    #[must_use]
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self {
            tokens: token_store(db),
            db,
        }
    }

    /// Serviço com a origem de token injetada — o gancho que deixa o fluxo
    /// testável sem mexer em variável de ambiente do processo.
    #[must_use]
    pub fn with_token_store(
        db: &'a DatabaseConnection,
        tokens: Box<dyn SetupTokenStore + 'a>,
    ) -> Self {
        Self { db, tokens }
    }

    /// `true` enquanto não existir nenhum usuário.
    ///
    /// # Errors
    ///
    /// Propaga erro do banco.
    pub async fn is_pending(&self) -> AppResult<bool> {
        Ok(users::Entity::find().count(self.db).await? == 0)
    }

    /// Token vigente. Só faz sentido enquanto a instalação está pendente.
    ///
    /// # Errors
    ///
    /// Propaga erro do banco.
    pub async fn token(&self) -> AppResult<String> {
        self.tokens.current().await
    }

    /// Origem do token vigente.
    #[must_use]
    pub fn token_origin(&self) -> SetupTokenOrigin {
        self.tokens.origin()
    }

    /// Cria o primeiro usuário e encerra a instalação.
    ///
    /// A ordem das checagens é deliberada: o estado da instalação vem antes da
    /// conferência do token para que um sistema já instalado responda `409` em
    /// vez de virar um oráculo de tokens para quem ficar tentando.
    ///
    /// # Errors
    ///
    /// * `422` quando nome, e-mail ou senha não passam na validação;
    /// * `409` quando já existe usuário (instalação concluída);
    /// * `401` quando o token não confere.
    pub async fn complete(&self, params: &SetupParams) -> AppResult<users::Model> {
        params.validate()?;

        if !self.is_pending().await? {
            return Err(AppError::conflict(
                "A instalação já foi concluída. Use a tela de login.",
            ));
        }

        let expected = self.tokens.current().await?;
        if !constant_time_eq(params.token.trim(), &expected) {
            tracing::warn!(
                email = %params.email,
                "cadastro inicial recusado: token de instalação inválido"
            );
            return Err(AppError::unauthorized("Token de instalação inválido."));
        }

        // `create_with_password` normaliza nome e e-mail e recusa duplicata.
        let user = users::Model::create_with_password(
            self.db,
            &RegisterParams {
                name: params.name.clone(),
                email: params.email.clone(),
                password: params.password.clone(),
            },
        )
        .await
        .map_err(map_model_error)?;

        let user = user
            .into_active_model()
            .verified(self.db)
            .await
            .map_err(map_model_error)?;

        self.tokens.revoke().await?;

        tracing::info!(
            user_pid = %user.pid,
            email = %user.email,
            "instalação concluída: primeiro usuário criado"
        );

        Ok(user)
    }
}

/// Traduz o erro do modelo do Loco para o contrato HTTP da casa (§5.5).
///
/// Sem este `match` tudo cairia no `From<loco_rs::Error>` genérico, que só sabe
/// devolver `500`: o cliente veria "erro interno do servidor" onde o certo é
/// "esse e-mail já existe" ou a lista de campos reprovados.
pub(crate) fn map_model_error(err: ModelError) -> AppError {
    match err {
        ModelError::EntityAlreadyExists => {
            AppError::conflict("Já existe um usuário com este e-mail.")
        }
        ModelError::EntityNotFound => AppError::not_found("Usuário não encontrado."),
        // O `ActiveModelBehavior::before_save` de `users` valida nome e e-mail;
        // a reprovação chega aqui e precisa voltar por campo, não como 500.
        ModelError::Validation(errors) => AppError::ValidationFields(
            errors
                .errors
                .into_iter()
                .flat_map(|(field, erros)| {
                    erros.into_iter().map(move |erro| FieldError {
                        field: field.clone(),
                        message: erro.message.unwrap_or(erro.code),
                    })
                })
                .collect(),
        ),
        ModelError::DbErr(err) => AppError::from(err),
        other => AppError::Internal(anyhow::anyhow!(other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Origem de token de mentira: nem toca no banco, nem no ambiente.
    struct FixedToken {
        token: String,
        revoked: std::sync::atomic::AtomicBool,
    }

    impl FixedToken {
        fn new(token: &str) -> Self {
            Self {
                token: token.to_string(),
                revoked: std::sync::atomic::AtomicBool::new(false),
            }
        }
    }

    #[async_trait]
    impl SetupTokenStore for FixedToken {
        async fn current(&self) -> AppResult<String> {
            Ok(self.token.clone())
        }
        async fn revoke(&self) -> AppResult<()> {
            self.revoked
                .store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }
        fn origin(&self) -> SetupTokenOrigin {
            SetupTokenOrigin::Generated
        }
    }

    fn params(name: &str, email: &str, password: &str, token: &str) -> SetupParams {
        SetupParams {
            name: name.into(),
            email: email.into(),
            password: password.into(),
            token: token.into(),
        }
    }

    #[test]
    fn validacao_recusa_senha_curta_nome_curto_e_email_invalido() {
        let erros = params("A", "não-é-email", "1234", "")
            .validate()
            .unwrap_err();
        let campos = erros.field_errors();
        for campo in ["name", "email", "password", "token"] {
            assert!(campos.contains_key(campo), "faltou reprovar `{campo}`");
        }
    }

    #[test]
    fn validacao_aceita_o_caso_bom() {
        assert!(params("Maxuel", "admin@casa.com", "senha-forte-1", "tok")
            .validate()
            .is_ok());
    }

    #[tokio::test]
    async fn token_do_ambiente_nao_e_revogavel_mas_responde_o_valor() {
        let store = EnvSetupToken::new("do-operador");
        assert_eq!(store.current().await.unwrap(), "do-operador");
        assert!(store.revoke().await.is_ok());
        assert_eq!(store.current().await.unwrap(), "do-operador");
        assert_eq!(store.origin(), SetupTokenOrigin::Environment);
    }

    #[tokio::test]
    async fn store_de_mentira_marca_revogacao() {
        let store = FixedToken::new("abc");
        assert_eq!(store.current().await.unwrap(), "abc");
        store.revoke().await.unwrap();
        assert!(store.revoked.load(std::sync::atomic::Ordering::SeqCst));
    }
}
