//! Geração de chaves WireGuard (Curve25519) sem depender do binário `wg`.
//!
//! O motivo é poder desenvolver fora do container, onde `wg genkey` não existe
//! — e, no container, não precisar de `NET_ADMIN` só para gerar uma chave. O
//! `x25519-dalek` resolve sem vaivém por PKCS#8: a chave é literalmente o
//! escalar de 32 bytes.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::RngCore;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::services::shared::errors::{AppError, AppResult};

/// Tamanho, em bytes, de qualquer chave WireGuard (base64 de 32 bytes = 44 chars).
pub const WG_KEY_BYTES: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireGuardKeyPair {
    pub private_key: String,
    pub public_key: String,
}

/// Equivalente a `wg genkey` + `wg pubkey`.
#[must_use]
pub fn generate_key_pair() -> WireGuardKeyPair {
    let mut bytes = [0_u8; WG_KEY_BYTES];
    rand::thread_rng().fill_bytes(&mut bytes);
    // `StaticSecret::from` aplica o clamping da Curve25519 (zera 3 bits baixos,
    // fixa os dois altos) — é o que `wg genkey` também faz. Sem ele, a chave
    // privada exportada não corresponderia à pública derivada pelo peer.
    let secret = StaticSecret::from(bytes);
    let public = PublicKey::from(&secret);

    WireGuardKeyPair {
        private_key: BASE64.encode(secret.to_bytes()),
        public_key: BASE64.encode(public.as_bytes()),
    }
}

/// Equivalente a `wg pubkey` — deriva a chave pública a partir da privada.
///
/// # Errors
///
/// Falha quando a string não é uma chave WireGuard válida.
pub fn derive_public_key(private_key_b64: &str) -> AppResult<String> {
    let bytes = decode_key(private_key_b64)?;
    let secret = StaticSecret::from(bytes);
    Ok(BASE64.encode(PublicKey::from(&secret).as_bytes()))
}

/// Equivalente a `wg genpsk`.
#[must_use]
pub fn generate_preshared_key() -> String {
    let mut bytes = [0_u8; WG_KEY_BYTES];
    rand::thread_rng().fill_bytes(&mut bytes);
    BASE64.encode(bytes)
}

/// `true` quando a string é uma chave WireGuard válida (32 bytes em base64).
#[must_use]
pub fn is_valid_key(key: &str) -> bool {
    decode_key(key).is_ok()
}

/// Decodifica exigindo o formato exato do `wg`: 43 caracteres do alfabeto
/// base64 padrão + `=`, resolvendo para 32 bytes.
fn decode_key(key: &str) -> AppResult<[u8; WG_KEY_BYTES]> {
    let invalid = || AppError::validation(format!("Chave WireGuard inválida: {key}"));
    if key.len() != 44 || !key.ends_with('=') {
        return Err(invalid());
    }
    if !key[..43]
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'+' || byte == b'/')
    {
        return Err(invalid());
    }
    let decoded = BASE64.decode(key).map_err(|_| invalid())?;
    decoded.try_into().map_err(|_| invalid())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_par_gerado_e_consistente_com_a_derivacao() {
        let pair = generate_key_pair();
        assert!(is_valid_key(&pair.private_key));
        assert!(is_valid_key(&pair.public_key));
        assert_eq!(
            derive_public_key(&pair.private_key).unwrap(),
            pair.public_key
        );
    }

    #[test]
    fn duas_geracoes_nao_colidem() {
        assert_ne!(
            generate_key_pair().private_key,
            generate_key_pair().private_key
        );
        assert_ne!(generate_preshared_key(), generate_preshared_key());
    }

    #[test]
    fn a_chave_tem_o_formato_que_o_wg_aceita() {
        let pair = generate_key_pair();
        assert_eq!(pair.private_key.len(), 44);
        assert!(pair.private_key.ends_with('='));
        assert_eq!(
            BASE64.decode(&pair.private_key).unwrap().len(),
            WG_KEY_BYTES
        );
    }

    #[test]
    fn chave_invalida_e_recusada_em_vez_de_truncada() {
        for candidata in [
            "",
            "curta=",
            // 44 chars, mas com caractere fora do alfabeto base64.
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA!=",
            // base64 url-safe: o `wg` não aceita.
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA-_=",
            // 44 chars sem o `=` final.
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        ] {
            assert!(!is_valid_key(candidata), "aceitou {candidata:?}");
            assert!(derive_public_key(candidata).is_err());
        }
    }

    #[test]
    fn a_preshared_key_tambem_tem_32_bytes() {
        let psk = generate_preshared_key();
        assert!(is_valid_key(&psk));
    }
}
