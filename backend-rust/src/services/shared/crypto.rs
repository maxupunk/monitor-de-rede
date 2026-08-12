//! Cifra em repouso e hashes.
//!
//! Duas operações distintas convivem aqui e **não** podem ser confundidas:
//!
//! * [`encrypt`]/[`decrypt`] — reversível, XChaCha20-Poly1305. Usado nas chaves
//!   privadas do WireGuard (`vpn_servers.private_key_encrypted`,
//!   `vpn_peers.preshared_key_encrypted`), que precisam voltar em texto claro
//!   para montar o `wg0.conf`.
//! * [`sha256_hex`] — irreversível. Usado no `probes.token_hash`, comparado por
//!   igualdade na autenticação de probe (§7.10).
//!
//! > **Bancos anteriores ao corte para o backend Rust:** o formato de cifra do
//! > backend AdonisJS não é este. Um `pg_dump` cru daquela época deixa
//! > `private_key_encrypted` ilegível aqui; os valores precisam passar pelo
//! > `task vpn_secrets_import`. Não se aplica a banco criado por estas
//! > migrations. Ver `docs/historico/corte_backend_rust.md`.

use base64::Engine as _;
use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    XChaCha20Poly1305, XNonce,
};
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

use crate::services::shared::errors::{AppError, AppResult};

/// Tamanho do nonce do XChaCha20 — 24 bytes, grande o bastante para ser
/// sorteado aleatoriamente sem risco prático de colisão (o do ChaCha20
/// "normal", de 12 bytes, exigiria um contador persistido).
const NONCE_LEN: usize = 24;

/// Chave usada quando `APP_KEY` não está definida fora de produção. Existe só
/// para `cargo test`/`cargo loco start` funcionarem numa cópia recém-clonada.
const DEV_APP_KEY: &str = "netmonitor-development-app-key-do-not-use-in-production";

static APP_KEY: OnceLock<[u8; 32]> = OnceLock::new();

/// Chave simétrica de 32 bytes derivada de `APP_KEY` por SHA-256.
///
/// Deriva por SHA-256 em vez de exigir 32 bytes exatos: assim `APP_KEY` pode
/// ser uma string de tamanho livre, e ninguém precisa contar caracteres para
/// subir o serviço.
///
/// # Panics
///
/// Em `production` sem `APP_KEY` definida. É intencional: subir o serviço com
/// uma chave conhecida publicamente exporia as chaves privadas do WireGuard.
#[must_use]
pub fn app_key() -> &'static [u8; 32] {
    APP_KEY.get_or_init(|| {
        let secret = std::env::var("APP_KEY").unwrap_or_else(|_| {
            let is_production = std::env::var("LOCO_ENV")
                .map(|env| env == "production")
                .unwrap_or(false);
            assert!(
                !is_production,
                "APP_KEY é obrigatória em production: sem ela as chaves privadas do WireGuard \
                 seriam cifradas com uma chave que está no código-fonte"
            );
            tracing::warn!("APP_KEY não definida — usando a chave de desenvolvimento");
            DEV_APP_KEY.to_string()
        });
        Sha256::digest(secret.as_bytes()).into()
    })
}

fn cipher() -> XChaCha20Poly1305 {
    // `new_from_slice` em vez de `Key::from_slice`: o segundo é `GenericArray`,
    // depreciado na transição do `generic-array` 1.x. O tamanho é infalível —
    // `app_key()` devolve exatamente 32 bytes por construção.
    XChaCha20Poly1305::new_from_slice(app_key()).expect("app_key tem 32 bytes")
}

/// Cifra `plain` e devolve `base64(nonce || ciphertext)`.
///
/// O nonce viaja junto porque cada chamada sorteia um novo — sem ele o
/// `decrypt` não teria como reconstruir o estado do cifrador.
///
/// # Errors
///
/// Falha se a cifra não conseguir alocar/autenticar o bloco (na prática, só
/// por falta de memória).
pub fn encrypt(plain: &str) -> AppResult<String> {
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let ciphertext = cipher()
        .encrypt(&nonce, plain.as_bytes())
        .map_err(|e| AppError::Internal(anyhow::anyhow!("falha ao cifrar: {e}")))?;

    let mut payload = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    payload.extend_from_slice(&nonce);
    payload.extend_from_slice(&ciphertext);
    Ok(base64::engine::general_purpose::STANDARD.encode(payload))
}

/// Reverte [`encrypt`].
///
/// # Errors
///
/// [`AppError::Internal`] quando o base64 é inválido, o payload é curto demais
/// para conter o nonce, ou a autenticação Poly1305 falha — o último caso
/// significa chave errada ou dado adulterado, e **não** deve ser silenciado.
pub fn decrypt(cipher_text: &str) -> AppResult<String> {
    let payload = base64::engine::general_purpose::STANDARD
        .decode(cipher_text)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("base64 inválido: {e}")))?;

    if payload.len() <= NONCE_LEN {
        return Err(AppError::Internal(anyhow::anyhow!(
            "payload cifrado truncado: {} bytes",
            payload.len()
        )));
    }

    let (nonce_bytes, ciphertext) = payload.split_at(NONCE_LEN);
    let mut nonce = XNonce::default();
    nonce.copy_from_slice(nonce_bytes);

    let plain = cipher().decrypt(&nonce, ciphertext).map_err(|_| {
        AppError::Internal(anyhow::anyhow!(
            "falha ao decifrar: chave incorreta ou dado adulterado"
        ))
    })?;

    String::from_utf8(plain)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("texto decifrado não é UTF-8: {e}")))
}

/// SHA-256 em hexadecimal minúsculo — o formato gravado em `probes.token_hash`.
/// Mudar a codificação invalida todo token de probe já emitido, que não é
/// recuperável: o banco não guarda o token cru.
#[must_use]
pub fn sha256_hex(input: &str) -> String {
    hex::encode(Sha256::digest(input.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ciclo_completo_devolve_o_texto_original() {
        let secret = "cD8mQfN2vZ0aJ1kR5tY7uI9oP3sD6fG8hJ0kL2mN4bV=";
        let cipher_text = encrypt(secret).unwrap();
        assert_eq!(decrypt(&cipher_text).unwrap(), secret);
    }

    #[test]
    fn preserva_acentuacao_e_texto_vazio() {
        for plain in ["", "configuração ação ünïcödé 🔐"] {
            let cipher_text = encrypt(plain).unwrap();
            assert_eq!(decrypt(&cipher_text).unwrap(), plain);
        }
    }

    #[test]
    fn nonce_aleatorio_por_chamada_impede_cifra_deterministica() {
        // Dois textos iguais não podem produzir o mesmo criptograma: isso
        // revelaria que dois peers compartilham a mesma preshared key.
        let a = encrypt("mesma-chave").unwrap();
        let b = encrypt("mesma-chave").unwrap();
        assert_ne!(a, b);
        assert_eq!(decrypt(&a).unwrap(), decrypt(&b).unwrap());
    }

    #[test]
    fn payload_adulterado_falha_em_vez_de_devolver_lixo() {
        let cipher_text = encrypt("chave-privada").unwrap();
        let mut bytes = base64::engine::general_purpose::STANDARD
            .decode(&cipher_text)
            .unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xff;
        let adulterado = base64::engine::general_purpose::STANDARD.encode(bytes);
        assert!(decrypt(&adulterado).is_err());
    }

    #[test]
    fn entrada_invalida_nao_derruba_o_processo() {
        assert!(decrypt("não é base64!!").is_err());
        // Só o nonce, sem criptograma.
        let so_nonce = base64::engine::general_purpose::STANDARD.encode([0u8; NONCE_LEN]);
        assert!(decrypt(&so_nonce).is_err());
    }

    #[test]
    fn sha256_hex_bate_com_o_vetor_conhecido() {
        // Mesmo valor produzido por `crypto.createHash('sha256')` no Node.
        assert_eq!(
            sha256_hex("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(sha256_hex("abc").len(), 64);
    }
}
