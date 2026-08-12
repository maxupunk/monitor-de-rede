//! Anuncia, no boot, o token exigido no cadastro do primeiro usuário.
//!
//! Sem isto o token sorteado ficaria só no banco, e quem acabou de subir o
//! serviço não teria como descobri-lo — a tela de cadastro pediria um segredo
//! que ninguém viu. O log do container é o canal que quem instala sempre tem à
//! mão; a task `auth:setup_token` cobre quem chegou depois e perdeu a linha.
//!
//! Roda como `Initializer` (só no `start`, §`initializers::mod`): num `db
//! migrate` ou numa task o aviso seria ruído.

use async_trait::async_trait;
use loco_rs::{
    app::{AppContext, Initializer},
    Result,
};

use crate::services::auth::setup::{SetupService, SetupTokenOrigin, SETUP_TOKEN_ENV};

pub struct SetupInitializer;

#[async_trait]
impl Initializer for SetupInitializer {
    fn name(&self) -> String {
        "setup".to_string()
    }

    async fn before_run(&self, ctx: &AppContext) -> Result<()> {
        let service = SetupService::new(&ctx.db);

        // Banco indisponível não pode derrubar o boot por causa de um aviso.
        match service.is_pending().await {
            Ok(false) => return Ok(()),
            Ok(true) => {}
            Err(error) => {
                tracing::warn!(%error, "não foi possível verificar o estado da instalação");
                return Ok(());
            }
        }

        match service.token_origin() {
            // O operador fixou o valor e já o conhece: repeti-lo no log só
            // colocaria um segredo de longa vida em disco, onde ele não estava.
            SetupTokenOrigin::Environment => {
                tracing::warn!(
                    variavel = SETUP_TOKEN_ENV,
                    "instalação pendente: cadastre o primeiro usuário usando o token definido no ambiente"
                );
            }
            SetupTokenOrigin::Generated => match service.token().await {
                Ok(token) => tracing::warn!(
                    setup_token = %token,
                    "instalação pendente: cadastre o primeiro usuário com este token"
                ),
                Err(error) => {
                    tracing::warn!(%error, "não foi possível emitir o token de instalação");
                }
            },
        }

        Ok(())
    }
}
