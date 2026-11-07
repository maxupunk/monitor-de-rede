//! Criptografia e autenticação Web Push (RFC 8291, RFC 8292, RFC 8188).

use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes128Gcm, Nonce,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use hkdf::Hkdf;
use p256::{
    ecdh::EphemeralSecret,
    ecdsa::{signature::Signer, SigningKey},
    EncodedPoint, PublicKey,
};
use rand::{rngs::OsRng, RngCore};
use serde_json::json;
use sha2::Sha256;

use crate::services::shared::errors::{AppError, AppResult};

/// Informações de subscrição enviadas pelo navegador.
#[derive(Debug, Clone)]
pub struct SubscriptionKeys {
    /// Chave pública P-256 do cliente (base64url uncompressed, 65 bytes).
    pub p256dh: String,
    /// Segredo de autenticação do cliente (base64url, 16 bytes).
    pub auth: String,
}

/// Par de chaves VAPID do servidor.
#[derive(Debug, Clone)]
pub struct VapidKeyPair {
    pub public_key_base64: String,
    pub private_key_base64: String,
    pub subject: String,
}

/// Codifica bytes em Base64 URL Safe sem padding.
#[must_use]
pub fn base64url_encode(data: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(data)
}

/// Decodifica string Base64 URL Safe (com ou sem padding).
///
/// # Errors
///
/// Retorna erro se a string não for Base64 URL válida.
pub fn base64url_decode(data: &str) -> AppResult<Vec<u8>> {
    let clean = data.trim().replace('=', "");
    URL_SAFE_NO_PAD
        .decode(clean.as_bytes())
        .map_err(|e| AppError::bad_request(format!("Base64URL inválido: {e}")))
}

/// Gera um novo par de chaves VAPID (NIST P-256) em formato Base64URL.
#[must_use]
pub fn generate_vapid_key_pair(subject: &str) -> VapidKeyPair {
    let signing_key = SigningKey::random(&mut OsRng);
    let verifying_key = signing_key.verifying_key();
    let encoded_public = EncodedPoint::from(verifying_key);

    let private_bytes = signing_key.to_bytes();
    let public_bytes = encoded_public.as_bytes();

    VapidKeyPair {
        public_key_base64: base64url_encode(public_bytes),
        private_key_base64: base64url_encode(&private_bytes),
        subject: subject.to_string(),
    }
}

/// Extrai a origem (esquema + host + porta) de um endpoint de push.
#[must_use]
pub fn extract_audience(endpoint: &str) -> String {
    if let Ok(url) = reqwest::Url::parse(endpoint) {
        if let Some(host) = url.host_str() {
            let scheme = url.scheme();
            if let Some(port) = url.port() {
                return format!("{scheme}://{host}:{port}");
            }
            return format!("{scheme}://{host}");
        }
    }
    endpoint.to_string()
}

/// Cria o cabeçalho Authorization VAPID (RFC 8292) para um dado endpoint.
///
/// # Errors
///
/// Retorna erro se as chaves VAPID forem inválidas.
pub fn build_vapid_header(
    vapid: &VapidKeyPair,
    endpoint: &str,
    expiration_timestamp: i64,
) -> AppResult<String> {
    let aud = extract_audience(endpoint);
    let private_bytes = base64url_decode(&vapid.private_key_base64)?;
    let signing_key = SigningKey::from_slice(&private_bytes)
        .map_err(|e| AppError::bad_request(format!("Chave privada VAPID inválida: {e}")))?;

    // Cabeçalho JWT: {"typ":"JWT","alg":"ES256"}
    let header_json = json!({
        "typ": "JWT",
        "alg": "ES256"
    });
    let header_b64 = base64url_encode(header_json.to_string().as_bytes());

    // Payload JWT com aud, exp e sub
    let payload_json = json!({
        "aud": aud,
        "exp": expiration_timestamp,
        "sub": vapid.subject
    });
    let payload_b64 = base64url_encode(payload_json.to_string().as_bytes());

    let message_to_sign = format!("{header_b64}.{payload_b64}");
    let signature: p256::ecdsa::Signature = signing_key.sign(message_to_sign.as_bytes());
    let sig_bytes = signature.to_bytes();
    let sig_b64 = base64url_encode(&sig_bytes);

    let token = format!("{message_to_sign}.{sig_b64}");
    Ok(format!("vapid t={}, k={}", token, vapid.public_key_base64))
}

/// Criptografa o payload da notificação usando AES-128-GCM (RFC 8291 + RFC 8188).
///
/// # Errors
///
/// Retorna erro se as chaves da subscrição forem inválidas ou a cifra falhar.
pub fn encrypt_payload(
    subscription: &SubscriptionKeys,
    payload_bytes: &[u8],
) -> AppResult<Vec<u8>> {
    let client_public_bytes = base64url_decode(&subscription.p256dh)?;
    let client_auth_bytes = base64url_decode(&subscription.auth)?;

    if client_public_bytes.len() != 65 {
        return Err(AppError::bad_request(
            "Chave pública do cliente (p256dh) deve ter 65 bytes",
        ));
    }
    if client_auth_bytes.len() < 16 {
        return Err(AppError::bad_request(
            "Segredo de autenticação do cliente (auth) deve ter pelo menos 16 bytes",
        ));
    }

    let client_public_key = PublicKey::from_sec1_bytes(&client_public_bytes).map_err(|e| {
        AppError::bad_request(format!(
            "Chave pública do cliente inválida na curva P-256: {e}"
        ))
    })?;

    // 1. Gera par efêmero do servidor
    let server_secret = EphemeralSecret::random(&mut OsRng);
    let server_public = PublicKey::from(&server_secret);
    let server_point = EncodedPoint::from(server_public);
    let server_public_bytes = server_point.as_bytes();

    // 2. ECDH Shared Secret
    let shared_secret = server_secret.diffie_hellman(&client_public_key);
    let shared_secret_bytes = shared_secret.raw_secret_bytes();

    // 3. Derivação IKM (RFC 8291 Section 3.4)
    // info = "WebPush: info\0" || client_public_key || server_public_key
    let mut key_info = Vec::with_capacity(14 + 65 + 65);
    key_info.extend_from_slice(b"WebPush: info\0");
    key_info.extend_from_slice(&client_public_bytes);
    key_info.extend_from_slice(server_public_bytes);

    let hkdf_auth = Hkdf::<Sha256>::new(Some(&client_auth_bytes), shared_secret_bytes);
    let mut ikm = [0u8; 32];
    hkdf_auth
        .expand(&key_info, &mut ikm)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Erro no HKDF IKM: {e}")))?;

    // 4. Sal aleatório de 16 bytes para RFC 8188
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);

    // 5. Derivação do Content Encryption Key (CEK) e Nonce
    let hkdf_salt = Hkdf::<Sha256>::new(Some(&salt), &ikm);

    let mut cek = [0u8; 16];
    hkdf_salt
        .expand(b"Content-Encoding: aes128gcm\0", &mut cek)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Erro no HKDF CEK: {e}")))?;

    let mut nonce_bytes = [0u8; 12];
    hkdf_salt
        .expand(b"Content-Encoding: nonce\0", &mut nonce_bytes)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Erro no HKDF Nonce: {e}")))?;

    // 6. Formatação do registro único com delimitador 0x02
    let mut record_plaintext = Vec::with_capacity(payload_bytes.len() + 1);
    record_plaintext.extend_from_slice(payload_bytes);
    record_plaintext.push(0x02); // Delimitador de fim de registro

    // 7. Cifra AES-128-GCM
    let cipher = Aes128Gcm::new_from_slice(&cek)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Erro ao inicializar Aes128Gcm: {e}")))?;
    let nonce = Nonce::from(nonce_bytes);
    let ciphertext = cipher
        .encrypt(&nonce, Payload::from(record_plaintext.as_slice()))
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Erro ao cifrar payload WebPush: {e}")))?;

    // 8. Montagem do corpo binário RFC 8188:
    // salt (16) || rs (4) || idlen (1) || keyid (65) || ciphertext
    let rs: u32 = 4096;
    let mut body = Vec::with_capacity(16 + 4 + 1 + 65 + ciphertext.len());
    body.extend_from_slice(&salt);
    body.extend_from_slice(&rs.to_be_bytes());
    body.push(65); // idlen = 65 bytes
    body.extend_from_slice(server_public_bytes);
    body.extend_from_slice(&ciphertext);

    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geracao_e_codificacao_de_chaves_vapid() {
        let keys = generate_vapid_key_pair("mailto:admin@example.com");
        assert_eq!(keys.subject, "mailto:admin@example.com");

        let pub_bytes = base64url_decode(&keys.public_key_base64).unwrap();
        let priv_bytes = base64url_decode(&keys.private_key_base64).unwrap();

        assert_eq!(pub_bytes.len(), 65);
        assert_eq!(pub_bytes[0], 0x04); // Ponto uncompressed
        assert_eq!(priv_bytes.len(), 32);
    }

    #[test]
    fn extracao_de_audience() {
        assert_eq!(
            extract_audience("https://fcm.googleapis.com/fcm/send/sub123"),
            "https://fcm.googleapis.com"
        );
        assert_eq!(
            extract_audience("https://updates.push.services.mozilla.com/wpush/v2/abc"),
            "https://updates.push.services.mozilla.com"
        );
        assert_eq!(
            extract_audience("https://push.example.com:8443/push/v1"),
            "https://push.example.com:8443"
        );
    }

    #[test]
    fn geracao_do_cabecalho_vapid() {
        let keys = generate_vapid_key_pair("mailto:test@netmonitor.local");
        let header =
            build_vapid_header(&keys, "https://fcm.googleapis.com/fcm/send/123", 1700000000)
                .unwrap();

        assert!(header.starts_with("vapid t="));
        assert!(header.contains(", k="));
        assert!(header.ends_with(&keys.public_key_base64));
    }

    #[test]
    fn cifragem_e_formato_rfc8188() {
        // Gera par de teste simulando o navegador
        let client_secret = SigningKey::random(&mut OsRng);
        let client_pub = EncodedPoint::from(client_secret.verifying_key());
        let mut client_auth = [0u8; 16];
        OsRng.fill_bytes(&mut client_auth);

        let sub = SubscriptionKeys {
            p256dh: base64url_encode(client_pub.as_bytes()),
            auth: base64url_encode(&client_auth),
        };

        let message = b"{\"title\":\"Alerta de Teste\",\"body\":\"Servidor Offline\"}";
        let encrypted = encrypt_payload(&sub, message).unwrap();

        // Verifica o cabeçalho RFC 8188
        assert!(encrypted.len() > 16 + 4 + 1 + 65);
        let idlen = encrypted[20];
        assert_eq!(idlen, 65);
        let server_pub_tag = encrypted[21];
        assert_eq!(server_pub_tag, 0x04);
    }
}
