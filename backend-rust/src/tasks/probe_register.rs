//! `cargo loco task probe_register` — registra um agente e imprime o token.
//!
//! Paridade com `node ace probe:register`. O token cru é exibido **uma única
//! vez**: o banco guarda só o `sha256`, então não há como recuperá-lo depois.

use chrono::Utc;
use loco_rs::prelude::*;
use rand::RngCore;
use sea_orm::{ActiveModelTrait, Set};

use crate::{models::probes, services::shared::crypto::sha256_hex};

/// 32 bytes de entropia, como o `crypto.randomBytes(32)` do backend anterior.
const TOKEN_BYTES: usize = 32;

pub struct ProbeRegister;

/// Gera o token cru do probe.
#[must_use]
pub fn generate_token() -> String {
    let mut bytes = [0_u8; TOKEN_BYTES];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

#[async_trait]
impl Task for ProbeRegister {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "probe_register".into(),
            detail: "Registra um novo agente probe e gera o token de autenticação".into(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &task::Vars) -> Result<()> {
        let name = vars
            .cli_arg("name")
            .map_err(|_| Error::Message("informe --name com o nome do probe".into()))?
            .trim()
            .to_string();
        if name.is_empty() {
            return Err(Error::Message("o nome do probe não pode ser vazio".into()));
        }
        let site_id = vars
            .cli_arg("site_id")
            .ok()
            .and_then(|value| value.parse::<i64>().ok());

        let raw_token = generate_token();
        let probe = probes::ActiveModel {
            name: Set(name),
            site_id: Set(site_id),
            token_hash: Set(sha256_hex(&raw_token)),
            status: Set("pending".into()),
            registered_at: Set(Some(Utc::now().into())),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await?;

        // `println!` e não `tracing`: é a saída útil do comando, e o operador
        // precisa dela mesmo com o log desligado.
        println!(
            "Probe \"{}\" (ID #{}) registrado com sucesso!",
            probe.name, probe.id
        );
        println!("----------------------------------------------------");
        println!("PROBE_TOKEN: {raw_token}");
        println!("Guarde este token! Ele não será exibido novamente.");
        println!("----------------------------------------------------");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_token_tem_a_entropia_do_backend_anterior() {
        let token = generate_token();
        assert_eq!(token.len(), TOKEN_BYTES * 2);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(token, generate_token());
    }

    #[test]
    fn o_banco_guarda_apenas_o_hash() {
        let token = generate_token();
        let hash = sha256_hex(&token);
        assert_ne!(hash, token);
        assert_eq!(hash.len(), 64);
    }
}
