//! O GitHub App da central, criado pelo *manifest*: o "Conectar GitHub".
//!
//! O fluxo, todo pelo navegador de quem clicou — a central não precisa estar
//! na internet:
//!
//! 1. a tela envia um formulário ao GitHub com o [`manifest`] (nome,
//!    permissões, endereço de volta) e um `state` de uso único
//!    ([`begin`]);
//! 2. o GitHub cria o app **na conta ou organização** de quem confirmou e
//!    volta com um `code`, que a central troca pela chave privada
//!    ([`convert`], dentro de uma hora);
//! 3. quem clicou instala o app escolhendo os repositórios; o GitHub volta
//!    com o `installation_id`.
//!
//! Daí em diante a central assina um JWT com a chave do app ([`AppKey`]) e
//! pede **tokens de instalação** de uma hora ([`installation_token`]), com o
//! mínimo de permissão para cada uso ([`Scope`]). Nenhum token de pessoa,
//! nada que expire sem aviso, nada preso a quem configurou.
//!
//! O webhook do app fica desligado: o aviso de versão nova chega pelo token
//! OIDC do Actions ([`crate::deploys::oidc`]).

use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use netmonitor_core::shared::errors::{AppError, AppResult};

use super::{Client, GitHubError};
use crate::deploys::workflow::HOOK_PATH;

/// Para onde o GitHub devolve o navegador (a criação e a instalação): uma
/// rota da tela, que conclui com a sessão de quem clicou.
pub const CALLBACK_PATH: &str = "/deploys/github";
/// O GitHub dá uma hora para trocar o `code`.
const STATE_TTL: Duration = Duration::from_secs(60 * 60);
/// O JWT do app vale no máximo 10 minutos; 9 deixa folga para o relógio.
const JWT_LIFETIME_SECS: i64 = 9 * 60;

/// Endereço da central como o navegador a vê: `https://host[:porta]`.
///
/// # Errors
///
/// `Validation` com algo que não é uma origem http(s).
pub fn normalize_origin(raw: &str) -> AppResult<String> {
    let invalid = || AppError::validation("Endereço da central inválido: use https://host");
    let url = reqwest::Url::parse(raw.trim()).map_err(|_| invalid())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.path().trim_end_matches('/').is_empty()
    {
        return Err(invalid());
    }
    Ok(url.origin().ascii_serialization())
}

/// O login de uma organização do GitHub.
///
/// # Errors
///
/// `Validation`.
pub fn validate_login(login: &str) -> AppResult<()> {
    let valid = !login.is_empty()
        && login.len() <= 39
        && !login.starts_with('-')
        && login.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    if valid {
        Ok(())
    } else {
        Err(AppError::validation(
            "Organização inválida: use o login dela no GitHub (letras, números e hífen)",
        ))
    }
}

/// As permissões pedidas. Só leitura por padrão; a escrita (abrir o PR do
/// workflow) é escolha de quem conecta.
#[must_use]
pub fn permissions(allow_write: bool) -> Map<String, Value> {
    let mut out = Map::new();
    out.insert("metadata".into(), "read".into());
    // Ver se o build do workflow passou.
    out.insert("actions".into(), "read".into());
    if allow_write {
        out.insert("contents".into(), "write".into());
        out.insert("pull_requests".into(), "write".into());
        // Sem ela o GitHub recusa gravar em `.github/workflows/`.
        out.insert("workflows".into(), "write".into());
    } else {
        out.insert("contents".into(), "read".into());
    }
    out
}

/// O manifest enviado ao GitHub. `name` precisa ser único no GitHub inteiro
/// (o GitHub deixa trocar na tela dele).
#[must_use]
pub fn manifest(origin: &str, name: &str, allow_write: bool) -> Value {
    let callback = format!("{origin}{CALLBACK_PATH}");
    json!({
        "name": name,
        "url": origin,
        "description": "Deploy contínuo pelo NetMonitor: lê o compose, acompanha o build e abre o PR do workflow.",
        "hook_attributes": { "url": format!("{origin}{HOOK_PATH}"), "active": false },
        "redirect_url": callback,
        "setup_url": callback,
        "setup_on_update": true,
        "public": false,
        "default_permissions": permissions(allow_write),
        "default_events": [],
    })
}

/// Um nome que dificilmente existe: `NetMonitor 3f9a1c`.
#[must_use]
pub fn suggested_name() -> String {
    let id = uuid::Uuid::new_v4().simple().to_string();
    format!("NetMonitor {}", &id[..6])
}

/// Onde o formulário é enviado: a conta de quem está logado no GitHub ou
/// uma organização.
#[must_use]
pub fn form_url(organization: Option<&str>, state: &str) -> String {
    match organization {
        Some(org) => {
            format!("https://github.com/organizations/{org}/settings/apps/new?state={state}")
        }
        None => format!("https://github.com/settings/apps/new?state={state}"),
    }
}

fn pending() -> &'static Mutex<HashMap<String, Instant>> {
    static PENDING: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Um `state` novo, de uso único, que vale uma hora.
#[must_use]
pub fn begin() -> String {
    let state = uuid::Uuid::new_v4().simple().to_string();
    let mut map = pending()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    map.retain(|_, created| created.elapsed() < STATE_TTL);
    map.insert(state.clone(), Instant::now());
    state
}

/// Consome o `state`: só vale uma vez e só se esta central o emitiu.
///
/// # Errors
///
/// `Validation` com `state` desconhecido ou vencido (outra aba, central
/// reiniciada, mais de uma hora).
pub fn finish(state: &str) -> AppResult<()> {
    let mut map = pending()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match map.remove(state) {
        Some(created) if created.elapsed() < STATE_TTL => Ok(()),
        _ => Err(AppError::validation(
            "Este retorno do GitHub não foi pedido por esta central (ou já venceu). Clique em \
             \"Conectar GitHub\" de novo.",
        )),
    }
}

/// Dono do app ou da instalação.
#[derive(Debug, Clone, Deserialize)]
pub struct Account {
    pub login: String,
    /// `User` ou `Organization`.
    #[serde(rename = "type")]
    pub kind: String,
}

/// O que o GitHub devolve na troca do `code`.
#[derive(Debug, Clone, Deserialize)]
pub struct Conversion {
    pub id: i64,
    pub slug: String,
    pub client_id: String,
    pub pem: String,
    pub owner: Account,
    #[serde(default)]
    pub permissions: HashMap<String, String>,
}

impl Conversion {
    /// O app recebeu as permissões de escrita (quem criou pode tê-las tirado
    /// na tela do GitHub).
    #[must_use]
    pub fn can_write(&self) -> bool {
        can_write(&self.permissions)
    }
}

/// Escrita completa: conteúdo, PRs e workflows.
#[must_use]
pub fn can_write(permissions: &HashMap<String, String>) -> bool {
    ["contents", "pull_requests", "workflows"]
        .iter()
        .all(|key| permissions.get(*key).map(String::as_str) == Some("write"))
}

/// Troca o `code` pela chave do app.
///
/// # Errors
///
/// `Validation` com `code` malformado; [`GitHubError`] (o `code` vale uma
/// hora e uma vez só).
pub async fn convert(client: &Client, code: &str) -> AppResult<Conversion> {
    if code.is_empty() || code.len() > 100 || !code.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(AppError::validation("Código do GitHub inválido"));
    }
    Ok(client
        .write(
            Method::POST,
            &format!("/app-manifests/{code}/conversions"),
            &json!({}),
        )
        .await?)
}

#[derive(Serialize)]
struct Claims<'a> {
    iat: i64,
    exp: i64,
    iss: &'a str,
}

/// A chave privada do app, pronta para assinar.
pub struct AppKey {
    client_id: String,
    key: EncodingKey,
}

impl AppKey {
    /// # Errors
    ///
    /// `Validation` quando o PEM não é uma chave RSA.
    pub fn from_pem(client_id: &str, pem: &str) -> AppResult<Self> {
        let key = EncodingKey::from_rsa_pem(pem.as_bytes())
            .map_err(|_| AppError::validation("A chave privada do GitHub App é inválida"))?;
        Ok(Self {
            client_id: client_id.to_string(),
            key,
        })
    }

    /// O JWT do app (RS256, 9 minutos, emissor = client ID).
    ///
    /// # Errors
    ///
    /// Falha ao assinar.
    pub fn jwt(&self) -> AppResult<String> {
        let now = chrono::Utc::now().timestamp();
        encode(
            &Header::new(Algorithm::RS256),
            &Claims {
                // Um minuto para trás: o relógio do GitHub pode estar atrás.
                iat: now - 60,
                exp: now + JWT_LIFETIME_SECS,
                iss: &self.client_id,
            },
            &self.key,
        )
        .map_err(|error| AppError::Internal(anyhow::anyhow!("JWT do GitHub App: {error}")))
    }

    /// Um cliente que fala como o app (`/app/...`).
    ///
    /// # Errors
    ///
    /// Falha ao assinar ou ao montar o cliente.
    pub fn client(&self, base: &Client) -> AppResult<Client> {
        Ok(base.with_token(self.jwt()?))
    }
}

/// Uma instalação do app.
#[derive(Debug, Clone, Deserialize)]
pub struct Installation {
    pub id: i64,
    pub account: Account,
    /// A tela do GitHub onde se escolhem os repositórios.
    pub html_url: String,
    /// `all` ou `selected`.
    pub repository_selection: String,
    /// O que quem instalou aceitou (pode ser menos que o pedido).
    #[serde(default)]
    pub permissions: HashMap<String, String>,
}

/// A instalação, se ela é deste app.
///
/// # Errors
///
/// [`GitHubError`], menos `NotFound`.
pub async fn installation(app: &Client, id: i64) -> AppResult<Option<Installation>> {
    Ok(app.find(&format!("/app/installations/{id}")).await?)
}

/// As instalações do app (um app privado só se instala na conta dona: zero
/// ou uma).
///
/// # Errors
///
/// [`GitHubError`].
pub async fn installations(app: &Client) -> AppResult<Vec<Installation>> {
    Ok(app.get("/app/installations").await?)
}

/// O que o token de instalação pode fazer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Clonar o repositório no agente: só leitura do conteúdo, e só dele.
    Clone,
    /// Ler o que o assistente mostra: conteúdo e execuções do Actions.
    Read,
    /// Tudo o que a instalação tem (abrir o PR do workflow).
    Full,
}

impl Scope {
    fn permissions(self) -> Option<Value> {
        match self {
            Self::Clone => Some(json!({ "contents": "read", "metadata": "read" })),
            Self::Read => {
                Some(json!({ "contents": "read", "metadata": "read", "actions": "read" }))
            }
            Self::Full => None,
        }
    }
}

#[derive(Deserialize)]
struct AccessToken {
    token: String,
}

/// Por que o GitHub recusou o token: falta permissão na instalação ou o
/// repositório não está nela.
fn token_refused(message: &str, repository: Option<&str>) -> AppError {
    if message.to_lowercase().contains("permission") {
        return AppError::business_rule(
            "A instalação do GitHub App não tem todas as permissões: no GitHub, aceite as \
             permissões pedidas (Conexões → configurações do app) e clique em conferir",
        );
    }
    match repository {
        Some(name) => AppError::business_rule(format!(
            "O GitHub App não tem acesso ao repositório {name}: inclua-o na instalação \
             (Conexões → escolher os repositórios)"
        )),
        None => AppError::business_rule(message.to_string()),
    }
}

/// Um token de instalação (uma hora), limitado a um repositório quando
/// `repository` vem.
///
/// # Errors
///
/// `BusinessRule` quando o repositório não está na instalação;
/// [`GitHubError`].
pub async fn installation_token(
    app: &Client,
    installation_id: i64,
    repository: Option<&str>,
    scope: Scope,
) -> AppResult<String> {
    let mut body = Map::new();
    if let Some(name) = repository {
        body.insert("repositories".into(), json!([name]));
    }
    if let Some(permissions) = scope.permissions() {
        body.insert("permissions".into(), permissions);
    }
    let path = format!("/app/installations/{installation_id}/access_tokens");
    match app
        .write::<_, AccessToken>(Method::POST, &path, &Value::Object(body))
        .await
    {
        Ok(token) => Ok(token.token),
        Err(GitHubError::Rejected(message)) => Err(token_refused(&message, repository)),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use base64::Engine as _;
    use jsonwebtoken::{decode, Validation};

    use super::*;
    use crate::deploys::oidc::key_in;

    const PRIVATE: &[u8] = include_bytes!("../../../tests/fixtures/oidc_test_key.der");
    const JWKS: &str = include_str!("../../../tests/fixtures/oidc_test_jwks.json");

    fn pem() -> String {
        let body = base64::engine::general_purpose::STANDARD.encode(PRIVATE);
        let lines: Vec<&str> = body
            .as_bytes()
            .chunks(64)
            .map(|chunk| std::str::from_utf8(chunk).expect("base64"))
            .collect();
        format!(
            "-----BEGIN RSA PRIVATE KEY-----\n{}\n-----END RSA PRIVATE KEY-----\n",
            lines.join("\n")
        )
    }

    #[test]
    fn origem_da_central() {
        assert_eq!(
            normalize_origin("https://central.exemplo.com/").expect("ok"),
            "https://central.exemplo.com"
        );
        assert_eq!(
            normalize_origin("http://10.0.0.5:3333").expect("ok"),
            "http://10.0.0.5:3333"
        );
        for invalid in [
            "ftp://x",
            "https://x/deploys",
            "https://user:senha@x",
            "nada",
            "https://x?a=1",
        ] {
            assert!(normalize_origin(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn manifest_so_leitura_por_padrao() {
        let value = manifest("https://central.exemplo.com", "NetMonitor abc123", false);
        assert_eq!(
            value["redirect_url"],
            "https://central.exemplo.com/deploys/github"
        );
        assert_eq!(value["setup_url"], value["redirect_url"]);
        assert_eq!(value["hook_attributes"]["active"], false);
        assert_eq!(value["public"], false);
        let permissions = &value["default_permissions"];
        assert_eq!(permissions["contents"], "read");
        assert!(permissions.get("workflows").is_none());
        assert!(permissions.get("pull_requests").is_none());
    }

    #[test]
    fn manifest_com_escrita_pede_o_minimo_para_o_pr() {
        let value = manifest("https://c", "n", true);
        let permissions = &value["default_permissions"];
        assert_eq!(permissions["contents"], "write");
        assert_eq!(permissions["pull_requests"], "write");
        assert_eq!(permissions["workflows"], "write");
        assert!(permissions.get("administration").is_none());
    }

    #[test]
    fn escrita_exige_as_tres_permissoes() {
        let mut granted: HashMap<String, String> = [
            ("contents", "write"),
            ("pull_requests", "write"),
            ("workflows", "write"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        assert!(can_write(&granted));
        granted.insert("contents".into(), "read".into());
        assert!(!can_write(&granted));
    }

    #[test]
    fn state_vale_uma_vez() {
        let state = begin();
        assert!(finish(&state).is_ok());
        assert!(finish(&state).is_err());
        assert!(finish("inventado").is_err());
    }

    #[test]
    fn formulario_da_conta_ou_da_organizacao() {
        assert_eq!(
            form_url(None, "s1"),
            "https://github.com/settings/apps/new?state=s1"
        );
        assert_eq!(
            form_url(Some("master-sistemas"), "s1"),
            "https://github.com/organizations/master-sistemas/settings/apps/new?state=s1"
        );
        assert!(validate_login("master-sistemas").is_ok());
        assert!(validate_login("../x").is_err());
        assert!(validate_login("").is_err());
    }

    #[test]
    fn nome_sugerido_cabe_no_limite_do_github() {
        let name = suggested_name();
        assert!(name.starts_with("NetMonitor "));
        assert!(name.len() <= 34);
    }

    #[test]
    fn jwt_assinado_com_a_chave_do_app() {
        let key = AppKey::from_pem("Iv1.abc", &pem()).expect("chave");
        let token = key.jwt().expect("jwt");
        let set: jsonwebtoken::jwk::JwkSet = serde_json::from_str(JWKS).expect("jwks");
        let kid = set.keys[0].common.key_id.clone().expect("kid");
        let decoding = key_in(&set, &kid).expect("chave pública");
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_required_spec_claims(&["exp", "iss"]);
        validation.set_issuer(&["Iv1.abc"]);
        let claims = decode::<Value>(&token, &decoding, &validation).expect("válido");
        let iat = claims.claims["iat"].as_i64().expect("iat");
        let exp = claims.claims["exp"].as_i64().expect("exp");
        assert!(
            exp - iat <= 10 * 60,
            "o GitHub recusa JWT de mais de 10 minutos"
        );
    }

    #[test]
    fn chave_invalida_e_erro_de_validacao() {
        assert!(AppKey::from_pem("x", "não é PEM").is_err());
    }

    #[test]
    fn recusa_do_token_em_palavras() {
        let permission = token_refused(
            "O GitHub recusou: The permissions requested are not granted to this installation.",
            Some("SGCv4"),
        );
        assert!(permission.to_string().contains("permissões"));
        let repository = token_refused(
            "O GitHub recusou: There is at least one repository that does not exist",
            Some("SGCv4"),
        );
        assert!(repository.to_string().contains("SGCv4"));
    }

    #[test]
    fn token_de_clone_so_le() {
        assert_eq!(
            Scope::Clone.permissions().expect("limitado"),
            json!({ "contents": "read", "metadata": "read" })
        );
        assert!(Scope::Full.permissions().is_none());
    }
}
