//! O que só o navegador vê.
//!
//! Quando o navegador desiste de uma requisição (tempo esgotado) ou a conexão
//! cai no caminho (proxy, túnel), a central não fica sabendo: ela pode até
//! concluir o pedido depois, sem ninguém para receber a resposta. O operador
//! vê "falha de conexão" e o log da aplicação não tem nada.
//!
//! O navegador avisa, e o aviso entra no **mesmo** log da aplicação
//! (`tracing` → `device_logs` do dispositivo do sistema, em Logs › Este
//! servidor), ao lado do "requisição lenta" que a central registra para o
//! mesmo caminho. Não há tabela nem tela à parte.

use std::{sync::OnceLock, time::Duration};

use netmonitor_core::shared::rate_limit::SlidingWindowRateLimiter;

use crate::{
    dtos::client_failures::ClientFailureInput,
    services::shared::errors::{AppError, AppResult},
};

const MAX_PATH_CHARS: usize = 300;
const MAX_PAGE_CHARS: usize = 200;
/// Uma hora: o maior tempo de espera que faz sentido registrar.
const MAX_ELAPSED_MS: u64 = 60 * 60 * 1000;
const KNOWN_METHODS: [&str; 6] = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD"];

/// Avisos por usuário por minuto: uma rede caída faz cada tela aberta falhar
/// junto, e o log precisa do padrão, não de mil linhas iguais.
const REPORTS_PER_MINUTE: usize = 30;

/// O aviso validado, pronto para o log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientFailure {
    pub method: String,
    pub path: String,
    pub kind: &'static str,
    pub elapsed_ms: u64,
    pub page: Option<String>,
}

/// # Errors
///
/// `Validation` com método desconhecido, caminho fora da API ou texto com
/// caractere de controle (que forjaria linhas no log).
pub fn validate(input: ClientFailureInput) -> AppResult<ClientFailure> {
    let method = input.method.trim().to_ascii_uppercase();
    if !KNOWN_METHODS.contains(&method.as_str()) {
        return Err(AppError::validation("Método HTTP desconhecido"));
    }
    let path = input
        .path
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .trim()
        .to_string();
    if !path.starts_with('/') || path.chars().count() > MAX_PATH_CHARS || has_control(&path) {
        return Err(AppError::validation("Caminho da requisição inválido"));
    }
    let page = match input.page.map(|page| page.trim().to_string()) {
        Some(page) if page.chars().count() > MAX_PAGE_CHARS || has_control(&page) => {
            return Err(AppError::validation("Tela inválida"));
        }
        Some(page) if page.is_empty() => None,
        page => page,
    };
    Ok(ClientFailure {
        method,
        path,
        kind: input.kind.as_str(),
        elapsed_ms: input.elapsed_ms.min(MAX_ELAPSED_MS),
        page,
    })
}

fn has_control(text: &str) -> bool {
    text.chars().any(char::is_control)
}

fn limiter() -> &'static SlidingWindowRateLimiter {
    static LIMITER: OnceLock<SlidingWindowRateLimiter> = OnceLock::new();
    LIMITER.get_or_init(|| {
        SlidingWindowRateLimiter::for_environment(REPORTS_PER_MINUTE, Duration::from_secs(60))
    })
}

/// Grava o aviso no log da aplicação. Acima do limite do usuário, o aviso é
/// descartado em silêncio: quem avisa é o navegador, não uma pessoa.
pub fn record(user_id: i64, failure: &ClientFailure) {
    if !limiter().consume(&format!("user:{user_id}")).allowed {
        return;
    }
    tracing::warn!(
        user_id,
        method = %failure.method,
        path = %failure.path,
        kind = failure.kind,
        elapsed_ms = failure.elapsed_ms,
        page = failure.page.as_deref().unwrap_or("-"),
        "o navegador ficou sem resposta"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dtos::client_failures::ClientFailureKind;

    fn input(method: &str, path: &str, page: Option<&str>) -> ClientFailureInput {
        ClientFailureInput {
            method: method.into(),
            path: path.into(),
            kind: ClientFailureKind::Timeout,
            elapsed_ms: 15_000,
            page: page.map(str::to_string),
        }
    }

    #[test]
    fn aceita_o_aviso_e_corta_a_query() {
        let failure = validate(input(
            "post",
            "/api/docker/hosts/agent-9/containers/abc/stop?x=1",
            Some("/docker/compose"),
        ))
        .expect("aviso válido");
        assert_eq!(failure.method, "POST");
        assert_eq!(
            failure.path,
            "/api/docker/hosts/agent-9/containers/abc/stop"
        );
        assert_eq!(failure.kind, "tempo esgotado");
        assert_eq!(failure.page.as_deref(), Some("/docker/compose"));
    }

    #[test]
    fn recusa_o_que_forjaria_linha_no_log() {
        assert!(validate(input("GET", "/api/x\nlinha forjada", None)).is_err());
        assert!(validate(input("GET", "/api/x", Some("/tela\r\n"))).is_ok());
        assert!(validate(input("GET", "/api/x", Some("/te\u{1b}la"))).is_err());
        assert!(validate(input("TRACE", "/api/x", None)).is_err());
        assert!(validate(input("GET", "https://outro/api", None)).is_err());
        assert!(validate(input("GET", &format!("/{}", "a".repeat(400)), None)).is_err());
    }

    #[test]
    fn tela_vazia_vira_ausente_e_espera_tem_teto() {
        let mut long = input("GET", "/api/x", Some("  "));
        long.elapsed_ms = u64::MAX;
        let failure = validate(long).expect("válido");
        assert_eq!(failure.page, None);
        assert_eq!(failure.elapsed_ms, MAX_ELAPSED_MS);
    }
}
