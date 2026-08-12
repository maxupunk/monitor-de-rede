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

/// Variável que guarda a chave de cifra.
pub const ENCRYPTION_KEY_ENV: &str = "ENCRYPTION_KEY";

/// Nome anterior da mesma variável.
///
/// `APP_KEY` é convenção do AdonisJS, e ficou no repositório por inércia depois
/// que o backend virou Rust. O nome mudou; o **valor** não pode mudar, porque é
/// dele que sai a chave que decifra as chaves privadas do WireGuard já gravadas.
/// Por isso o fallback continua sendo lido: uma instalação existente que só
/// tenha `APP_KEY` no ambiente segue funcionando, com um aviso pedindo a
/// renomeação. Remover este fallback torna ilegível todo peer VPN já cadastrado.
const LEGACY_ENCRYPTION_KEY_ENV: &str = "APP_KEY";

/// Chave usada quando nenhuma das duas está definida fora de produção. Existe só
/// para `cargo test`/`cargo loco start` funcionarem numa cópia recém-clonada.
const DEV_ENCRYPTION_KEY: &str = "netmonitor-development-app-key-do-not-use-in-production";

static ENCRYPTION_KEY: OnceLock<[u8; 32]> = OnceLock::new();

/// Chave simétrica de 32 bytes derivada de [`ENCRYPTION_KEY_ENV`] por SHA-256.
///
/// Deriva por SHA-256 em vez de exigir 32 bytes exatos: assim a variável pode
/// ser uma string de tamanho livre, e ninguém precisa contar caracteres para
/// subir o serviço.
///
/// # Panics
///
/// Em `production` sem chave definida (nem no nome novo, nem no antigo). É
/// intencional: subir o serviço com uma chave que está no código-fonte exporia
/// as chaves privadas do WireGuard.
#[must_use]
pub fn encryption_key() -> &'static [u8; 32] {
    ENCRYPTION_KEY.get_or_init(|| {
        let secret = resolve_secret();
        Sha256::digest(secret.as_bytes()).into()
    })
}

fn resolve_secret() -> String {
    if let Some(secret) = non_empty_env(ENCRYPTION_KEY_ENV) {
        return secret;
    }

    if let Some(secret) = non_empty_env(LEGACY_ENCRYPTION_KEY_ENV) {
        tracing::warn!(
            "{LEGACY_ENCRYPTION_KEY_ENV} está obsoleta (era o nome do AdonisJS): renomeie para \
             {ENCRYPTION_KEY_ENV} mantendo o MESMO valor — trocar o valor torna ilegível tudo \
             que já foi cifrado"
        );
        return secret;
    }

    let is_production = std::env::var("LOCO_ENV").map(|env| env == "production") == Ok(true);
    assert!(
        !is_production,
        "{ENCRYPTION_KEY_ENV} é obrigatória em production: sem ela as chaves privadas do \
         WireGuard seriam cifradas com uma chave que está no código-fonte"
    );
    tracing::warn!("{ENCRYPTION_KEY_ENV} não definida — usando a chave de desenvolvimento");
    DEV_ENCRYPTION_KEY.to_string()
}

/// Lê uma variável tratando string vazia como ausente — um `ENCRYPTION_KEY=`
/// esquecido no `.env` não pode virar uma chave válida de zero bytes.
fn non_empty_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

fn cipher() -> XChaCha20Poly1305 {
    // `new_from_slice` em vez de `Key::from_slice`: o segundo é `GenericArray`,
    // depreciado na transição do `generic-array` 1.x. O tamanho é infalível —
    // `encryption_key()` devolve exatamente 32 bytes por construção.
    XChaCha20Poly1305::new_from_slice(encryption_key()).expect("encryption_key tem 32 bytes")
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

/// Compara dois segredos sem vazar, pelo tempo gasto, o quanto eles se parecem.
///
/// O `==` de `&str` para no primeiro byte diferente. Quem consegue medir o
/// tempo de resposta descobre com isso quantos bytes iniciais do palpite estão
/// certos e reconstrói o segredo byte a byte, em vez de tentar o espaço
/// inteiro. Comparar os digests em vez das entradas resolve de uma vez o
/// tamanho: `Sha256` sempre devolve 32 bytes, então o laço roda o mesmo número
/// de vezes mesmo quando os textos têm comprimentos diferentes — e o próprio
/// comprimento do segredo deixa de ser observável.
#[must_use]
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (Sha256::digest(a.as_bytes()), Sha256::digest(b.as_bytes()));
    // O `fold` acumula com OR: qualquer byte diferente acende um bit que os
    // seguintes não conseguem apagar, e nenhum caminho sai do laço mais cedo.
    a.iter()
        .zip(b.iter())
        .fold(0u8, |diff, (x, y)| diff | (x ^ y))
        == 0
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
    fn variavel_vazia_conta_como_ausente() {
        // Um `ENCRYPTION_KEY=` esquecido no `.env` não pode virar chave válida.
        assert!(non_empty_env("VARIAVEL_QUE_NAO_EXISTE_NO_AMBIENTE").is_none());
    }

    #[test]
    fn constant_time_eq_so_aceita_o_segredo_exato() {
        assert!(constant_time_eq(
            "token-de-instalacao",
            "token-de-instalacao"
        ));
        assert!(constant_time_eq("", ""));
        // Prefixo correto não passa: é justamente o caso que o `==` entregaria
        // pelo tempo de resposta.
        assert!(!constant_time_eq(
            "token-de-instalaca",
            "token-de-instalacao"
        ));
        assert!(!constant_time_eq(
            "token-de-instalacaO",
            "token-de-instalacao"
        ));
        assert!(!constant_time_eq("", "token-de-instalacao"));
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
