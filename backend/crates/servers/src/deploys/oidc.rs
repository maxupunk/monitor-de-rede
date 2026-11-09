//! A prova de que um pedido de deploy veio do GitHub Actions do repositório.
//!
//! O workflow pede ao GitHub um token OIDC (`permissions: id-token: write`)
//! com o `audience` desta central e o manda no `Authorization`. O token é um
//! JWT RS256 assinado pelo GitHub, que diz **qual repositório, qual ref e
//! qual commit** — nenhum segredo fica guardado no GitHub nem aqui. A
//! central confere:
//!
//! * a assinatura, com as chaves públicas do GitHub (JWKS, em cache);
//! * o emissor (`token.actions.githubusercontent.com`) e o `audience`;
//! * a validade (`exp`, com 60 s de folga para relógio);
//! * que o mesmo token não foi usado antes (`jti`), dentro da validade.
//!
//! O repositório do token é o que escolhe a aplicação; o corpo da chamada
//! não decide nada além da mensagem do commit e da tag da imagem.

use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use async_trait::async_trait;
use jsonwebtoken::{decode, decode_header, jwk::JwkSet, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use tokio::sync::RwLock;

use netmonitor_core::shared::errors::{AppError, AppResult};

pub const ISSUER: &str = "https://token.actions.githubusercontent.com";
pub const JWKS_URL: &str = "https://token.actions.githubusercontent.com/.well-known/jwks";
/// Variável que troca o `audience` esperado.
pub const AUDIENCE_ENV: &str = "DEPLOY_OIDC_AUDIENCE";
pub const DEFAULT_AUDIENCE: &str = "netmonitor-deploy";
/// Folga de relógio na validade.
const LEEWAY_SECS: u64 = 60;
/// As chaves do GitHub mudam raramente; uma hora de cache basta.
const JWKS_TTL: Duration = Duration::from_secs(3600);
/// Chave desconhecida não refaz a busca mais que isso (um token forjado
/// com `kid` inventado não vira martelo no GitHub).
const JWKS_RETRY: Duration = Duration::from_secs(60);
const JWKS_TIMEOUT: Duration = Duration::from_secs(10);

/// O `audience` que esta central espera.
#[must_use]
pub fn audience() -> String {
    std::env::var(AUDIENCE_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_AUDIENCE.to_string())
}

/// O que o token do GitHub Actions diz.
#[derive(Debug, Clone, Deserialize)]
pub struct GithubClaims {
    /// `dono/repo`.
    pub repository: String,
    /// `refs/heads/main`, `refs/tags/v1.2.0`.
    #[serde(rename = "ref")]
    pub git_ref: String,
    pub sha: String,
    #[serde(default)]
    pub actor: Option<String>,
    #[serde(default)]
    pub event_name: Option<String>,
    #[serde(default)]
    pub jti: Option<String>,
    pub exp: i64,
}

/// De onde vêm as chaves públicas. Na produção, o JWKS do GitHub; nos
/// testes, uma chave local.
#[async_trait]
pub trait KeySource: Send + Sync {
    async fn key(&self, kid: &str) -> AppResult<Option<DecodingKey>>;
}

fn unauthorized(reason: &str) -> AppError {
    AppError::unauthorized(format!("Token do GitHub recusado: {reason}"))
}

/// Confere o token e devolve o que ele diz.
///
/// # Errors
///
/// `401` com o motivo: formato, chave desconhecida, assinatura, emissor,
/// `audience`, validade ou reuso.
pub async fn verify(token: &str, keys: &dyn KeySource, audience: &str) -> AppResult<GithubClaims> {
    let header = decode_header(token).map_err(|_| unauthorized("formato inválido"))?;
    if header.alg != Algorithm::RS256 {
        return Err(unauthorized("algoritmo diferente de RS256"));
    }
    let kid = header
        .kid
        .ok_or_else(|| unauthorized("sem identificador de chave"))?;
    let key = keys
        .key(&kid)
        .await?
        .ok_or_else(|| unauthorized("chave de assinatura desconhecida"))?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[ISSUER]);
    validation.set_audience(&[audience]);
    validation.leeway = LEEWAY_SECS;
    validation.set_required_spec_claims(&["exp", "iss", "aud"]);
    let claims = decode::<GithubClaims>(token, &key, &validation)
        .map_err(|error| {
            use jsonwebtoken::errors::ErrorKind;
            match error.kind() {
                ErrorKind::ExpiredSignature => unauthorized("expirado"),
                ErrorKind::InvalidAudience => unauthorized(&format!(
                    "audience diferente de \"{audience}\" (confira o passo do workflow)"
                )),
                ErrorKind::InvalidIssuer => unauthorized("não foi emitido pelo GitHub Actions"),
                _ => unauthorized("assinatura inválida"),
            }
        })?
        .claims;
    if let Some(jti) = &claims.jti {
        if !replay_guard().first_use(jti, claims.exp) {
            return Err(unauthorized("já usado"));
        }
    }
    Ok(claims)
}

/// Lembra os `jti` vistos até a validade deles acabar.
#[derive(Default)]
pub struct ReplayGuard {
    seen: Mutex<HashMap<String, i64>>,
}

impl ReplayGuard {
    /// `true` na primeira vez que o `jti` aparece.
    pub fn first_use(&self, jti: &str, exp: i64) -> bool {
        let now = chrono::Utc::now().timestamp();
        let mut seen = self
            .seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let leeway = i64::try_from(LEEWAY_SECS).unwrap_or(60);
        seen.retain(|_, expires| *expires + leeway >= now);
        if seen.contains_key(jti) {
            return false;
        }
        seen.insert(jti.to_string(), exp);
        true
    }
}

fn replay_guard() -> &'static ReplayGuard {
    static GUARD: OnceLock<ReplayGuard> = OnceLock::new();
    GUARD.get_or_init(ReplayGuard::default)
}

/// As chaves do GitHub, em cache.
pub struct GithubKeys {
    client: reqwest::Client,
    cache: RwLock<Option<(Instant, JwkSet)>>,
    last_fetch: Mutex<Option<Instant>>,
}

impl GithubKeys {
    fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(JWKS_TIMEOUT)
                .user_agent("netmonitor-deploy")
                .build()
                .unwrap_or_default(),
            cache: RwLock::new(None),
            last_fetch: Mutex::new(None),
        }
    }

    async fn fetch(&self) -> AppResult<JwkSet> {
        let set = self
            .client
            .get(JWKS_URL)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|error| {
                AppError::service_unavailable(format!(
                    "Não foi possível obter as chaves do GitHub: {error}"
                ))
            })?
            .json::<JwkSet>()
            .await
            .map_err(|_| AppError::service_unavailable("Chaves do GitHub ilegíveis"))?;
        Ok(set)
    }

    fn may_refetch(&self) -> bool {
        let mut last = self
            .last_fetch
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if last.is_some_and(|at| at.elapsed() < JWKS_RETRY) {
            return false;
        }
        *last = Some(Instant::now());
        true
    }
}

/// Procura a chave `kid` num JWKS.
#[must_use]
pub fn key_in(set: &JwkSet, kid: &str) -> Option<DecodingKey> {
    set.find(kid)
        .and_then(|jwk| DecodingKey::from_jwk(jwk).ok())
}

#[async_trait]
impl KeySource for GithubKeys {
    async fn key(&self, kid: &str) -> AppResult<Option<DecodingKey>> {
        if let Some((at, set)) = self.cache.read().await.as_ref() {
            if at.elapsed() < JWKS_TTL {
                if let Some(key) = key_in(set, kid) {
                    return Ok(Some(key));
                }
            }
        }
        if !self.may_refetch() {
            return Ok(self
                .cache
                .read()
                .await
                .as_ref()
                .and_then(|(_, set)| key_in(set, kid)));
        }
        let set = self.fetch().await?;
        let key = key_in(&set, kid);
        *self.cache.write().await = Some((Instant::now(), set));
        Ok(key)
    }
}

/// As chaves do GitHub do processo.
#[must_use]
pub fn github_keys() -> &'static GithubKeys {
    static KEYS: OnceLock<GithubKeys> = OnceLock::new();
    KEYS.get_or_init(GithubKeys::new)
}

#[cfg(test)]
pub(crate) mod testing {
    //! Uma chave RSA **só de teste** (`tests/fixtures/oidc_test_key.der`,
    //! PKCS#1) e o JWKS com a pública dela, para assinar tokens como o GitHub
    //! assinaria. Em DER porque ler PEM pediria a feature `use_pem` (e mais
    //! duas crates) só para os testes.

    use jsonwebtoken::{encode, EncodingKey, Header};
    use serde_json::{json, Value};

    use super::*;

    pub const KID: &str = "teste";
    const PRIVATE: &[u8] = include_bytes!("../../tests/fixtures/oidc_test_key.der");
    const JWKS: &str = include_str!("../../tests/fixtures/oidc_test_jwks.json");

    pub struct FixtureKeys;

    #[async_trait]
    impl KeySource for FixtureKeys {
        async fn key(&self, kid: &str) -> AppResult<Option<DecodingKey>> {
            let set: JwkSet = serde_json::from_str(JWKS).expect("jwks de teste");
            Ok(key_in(&set, kid))
        }
    }

    pub fn claims(repository: &str, git_ref: &str) -> Value {
        let now = chrono::Utc::now().timestamp();
        json!({
            "iss": ISSUER,
            "aud": DEFAULT_AUDIENCE,
            "repository": repository,
            "ref": git_ref,
            "sha": "a".repeat(40),
            "actor": "maxuel",
            "event_name": "push",
            "jti": uuid::Uuid::new_v4().to_string(),
            "iat": now,
            "exp": now + 300,
        })
    }

    pub fn sign(claims: &Value) -> String {
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(KID.to_string());
        encode(&header, claims, &EncodingKey::from_rsa_der(PRIVATE)).expect("token")
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{testing::*, *};

    #[tokio::test]
    async fn token_do_github_passa() {
        let token = sign(&claims("Master/SGCv4", "refs/heads/main"));
        let claims = verify(&token, &FixtureKeys, DEFAULT_AUDIENCE)
            .await
            .expect("válido");
        assert_eq!(claims.repository, "Master/SGCv4");
        assert_eq!(claims.git_ref, "refs/heads/main");
        assert_eq!(claims.actor.as_deref(), Some("maxuel"));
    }

    #[tokio::test]
    async fn audience_errado_diz_o_que_conferir() {
        let token = sign(&claims("a/b", "refs/heads/main"));
        let error = verify(&token, &FixtureKeys, "outra-central")
            .await
            .expect_err("audience");
        assert!(error.to_string().contains("audience"));
    }

    #[tokio::test]
    async fn emissor_e_validade_sao_conferidos() {
        let mut forged = claims("a/b", "refs/heads/main");
        forged["iss"] = json!("https://evil.example");
        assert!(verify(&sign(&forged), &FixtureKeys, DEFAULT_AUDIENCE)
            .await
            .is_err());

        let mut expired = claims("a/b", "refs/heads/main");
        let past = chrono::Utc::now().timestamp() - 3600;
        expired["exp"] = json!(past);
        let error = verify(&sign(&expired), &FixtureKeys, DEFAULT_AUDIENCE)
            .await
            .expect_err("expirado");
        assert!(error.to_string().contains("expirado"));
    }

    #[tokio::test]
    async fn o_mesmo_token_nao_vale_duas_vezes() {
        let token = sign(&claims("a/b", "refs/heads/main"));
        assert!(verify(&token, &FixtureKeys, DEFAULT_AUDIENCE).await.is_ok());
        let error = verify(&token, &FixtureKeys, DEFAULT_AUDIENCE)
            .await
            .expect_err("reuso");
        assert!(error.to_string().contains("já usado"));
    }

    #[tokio::test]
    async fn chave_desconhecida_e_lixo_sao_recusados() {
        assert!(verify("nao.e.jwt", &FixtureKeys, DEFAULT_AUDIENCE)
            .await
            .is_err());
        let mut header = jsonwebtoken::Header::new(Algorithm::HS256);
        header.kid = Some(KID.into());
        let hs = jsonwebtoken::encode(
            &header,
            &claims("a/b", "refs/heads/main"),
            &jsonwebtoken::EncodingKey::from_secret(b"segredo"),
        )
        .expect("hs256");
        let error = verify(&hs, &FixtureKeys, DEFAULT_AUDIENCE)
            .await
            .expect_err("hs256");
        assert!(error.to_string().contains("RS256"));
    }
}
