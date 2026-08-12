//! `backend_rust-cli task auth_setup_token` — mostra o token do cadastro inicial.
//!
//! Existe para o caso em que a linha do boot já rolou para fora do terminal (ou
//! o `docker logs` foi truncado) e a tela de cadastro está pedindo um segredo
//! que ninguém tem mais. Não gera token novo por conta própria: reimprime o
//! vigente, para que duas pessoas lendo a saída em momentos diferentes vejam o
//! mesmo valor.

use loco_rs::prelude::*;

use crate::services::auth::setup::{SetupService, SetupTokenOrigin, SETUP_TOKEN_ENV};

pub struct AuthSetupToken;

#[async_trait]
impl Task for AuthSetupToken {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "auth_setup_token".into(),
            detail: "Mostra o token exigido no cadastro do primeiro usuário".into(),
        }
    }

    async fn run(&self, ctx: &AppContext, _vars: &task::Vars) -> Result<()> {
        let service = SetupService::new(&ctx.db);

        if !service.is_pending().await? {
            println!("A instalação já foi concluída — o token não é mais aceito.");
            println!("Para criar outros usuários, entre no sistema e use o cadastro de usuários.");
            return Ok(());
        }

        println!("----------------------------------------------------");
        match service.token_origin() {
            SetupTokenOrigin::Environment => {
                println!("O token vem da variável de ambiente {SETUP_TOKEN_ENV}.");
                println!("Consulte o valor onde ela foi definida (.env, compose, secret).");
            }
            SetupTokenOrigin::Generated => {
                println!("SETUP_TOKEN: {}", service.token().await?);
                println!("Use-o na tela de cadastro do primeiro usuário.");
            }
        }
        println!("----------------------------------------------------");

        Ok(())
    }
}
