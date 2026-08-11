//! `cargo loco task vpn_secrets_import` — re-cifra os segredos da VPN na
//! migração de dados (§17 D6, Fase 9).
//!
//! **Por que este comando existe.** O AdonisJS cifrava
//! `vpn_servers.private_key_encrypted` e `vpn_peers.preshared_key_encrypted`
//! com o `encryption` do framework (AES-256-CBC + HMAC sobre a `APP_KEY`); o
//! backend Rust usa XChaCha20-Poly1305 com chave derivada da mesma `APP_KEY`.
//! Os dois formatos não se leem, e um `pg_dump`/`pg_restore` carrega o
//! criptograma antigo intacto — que, depois do corte, ninguém consegue abrir.
//!
//! O caminho é em dois passos, e o primeiro **tem** de rodar no AdonisJS,
//! porque só ele sabe decifrar:
//!
//! ```sh
//! # 1) No backend/ ainda vivo: exporta em claro para um arquivo temporário.
//! node ace vpn:export-secrets > /tmp/vpn-secrets.json
//!
//! # 2) No backend-rust/, já com o banco restaurado:
//! backend_rust-cli task vpn_secrets_import file:/tmp/vpn-secrets.json
//!
//! # 3) Apague o arquivo — ele contém as chaves em texto claro.
//! shred -u /tmp/vpn-secrets.json
//! ```
//!
//! Sem o passo 1 (por exemplo, se a `APP_KEY` antiga se perdeu), a saída é
//! rotacionar: `POST /api/vpn/peers/:id/rotate` em cada peer e reconfigurar o
//! servidor. O comando avisa quanto sobrou por fazer.

use loco_rs::prelude::*;
use sea_orm::{ActiveModelTrait, EntityTrait};
use serde::Deserialize;

use crate::models::{vpn_peers, vpn_servers};

/// Formato do arquivo produzido pelo exportador do AdonisJS.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SecretsFile {
    #[serde(default)]
    servers: Vec<ServerSecret>,
    #[serde(default)]
    peers: Vec<PeerSecret>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerSecret {
    id: i64,
    private_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PeerSecret {
    id: i64,
    /// `None` quando o peer nunca teve chave pré-compartilhada.
    preshared_key: Option<String>,
}

pub struct VpnSecretsImport;

#[async_trait]
impl Task for VpnSecretsImport {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "vpn_secrets_import".into(),
            detail: "Re-cifra os segredos da VPN exportados do backend AdonisJS".into(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &task::Vars) -> Result<()> {
        let path = vars
            .cli_arg("file")
            .map_err(|_| Error::Message("informe --file com o JSON exportado".into()))?;
        let raw = tokio::fs::read_to_string(path)
            .await
            .map_err(|error| Error::Message(format!("não foi possível ler {path}: {error}")))?;
        let secrets: SecretsFile = serde_json::from_str(&raw)
            .map_err(|error| Error::Message(format!("JSON inválido em {path}: {error}")))?;

        let mut servers_updated = 0;
        for secret in &secrets.servers {
            let Some(server) = vpn_servers::Entity::find_by_id(secret.id)
                .one(&ctx.db)
                .await?
            else {
                println!("aviso: servidor #{} não existe mais; pulado", secret.id);
                continue;
            };
            let mut active: vpn_servers::ActiveModel = server.into();
            active.set_private_key(&secret.private_key)?;
            active.update(&ctx.db).await?;
            servers_updated += 1;
        }

        let mut peers_updated = 0;
        for secret in &secrets.peers {
            let Some(peer) = vpn_peers::Entity::find_by_id(secret.id)
                .one(&ctx.db)
                .await?
            else {
                println!("aviso: peer #{} não existe mais; pulado", secret.id);
                continue;
            };
            let mut active: vpn_peers::ActiveModel = peer.into();
            active.set_preshared_key(secret.preshared_key.as_deref())?;
            active.update(&ctx.db).await?;
            peers_updated += 1;
        }

        println!("{servers_updated} servidor(es) e {peers_updated} peer(s) re-cifrados.");

        // Conferência final: qualquer linha que ainda não decifre precisa de
        // rotação manual, e o operador tem de saber disso **antes** do corte —
        // não quando um cliente tentar reconectar.
        let pending = pending_reencryption(ctx).await?;
        if pending.is_empty() {
            println!("Todos os segredos da VPN decifram com a APP_KEY atual.");
        } else {
            println!("\nATENÇÃO — ainda ilegíveis, precisam de rotação manual:");
            for item in &pending {
                println!("  {item}");
            }
            return Err(Error::Message(format!(
                "{} segredo(s) da VPN não decifram; rotacione antes de cortar",
                pending.len()
            )));
        }
        Ok(())
    }
}

/// Lista o que ainda não decifra com a chave atual.
async fn pending_reencryption(ctx: &AppContext) -> Result<Vec<String>> {
    let mut pending = Vec::new();
    for server in vpn_servers::Entity::find().all(&ctx.db).await? {
        if server.private_key().is_err() {
            pending.push(format!(
                "vpn_servers #{} ({})",
                server.id, server.interface_name
            ));
        }
    }
    for peer in vpn_peers::Entity::find().all(&ctx.db).await? {
        if peer.preshared_key().is_err() {
            pending.push(format!("vpn_peers #{}", peer.id));
        }
    }
    Ok(pending)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_arquivo_exportado_e_lido_em_camel_case() {
        let secrets: SecretsFile = serde_json::from_str(
            r#"{
                "servers": [{ "id": 1, "privateKey": "CHAVE" }],
                "peers": [
                    { "id": 2, "presharedKey": "PSK" },
                    { "id": 3, "presharedKey": null }
                ]
            }"#,
        )
        .expect("arquivo do exportador");
        assert_eq!(secrets.servers[0].private_key, "CHAVE");
        assert_eq!(secrets.peers[0].preshared_key.as_deref(), Some("PSK"));
        assert!(secrets.peers[1].preshared_key.is_none());
    }

    #[test]
    fn arquivo_sem_uma_das_secoes_ainda_e_valido() {
        // Uma instalação sem peers exporta só `servers` — e vice-versa.
        let secrets: SecretsFile =
            serde_json::from_str(r#"{ "servers": [] }"#).expect("seções são opcionais");
        assert!(secrets.peers.is_empty());
    }
}
