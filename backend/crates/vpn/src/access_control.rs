//! Proteções dos endpoints sensíveis de VPN (§8.10.4).
//!
//! Rate limit por solicitante e registro de auditoria de todo download de
//! configuração — os artefatos são credencial de acesso à rede, não payload
//! comum.

use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use serde::Serialize;

/// Janela deslizante: 10 requisições por 60 s (§7.13).
pub const RATE_LIMIT: usize = 10;
pub const RATE_WINDOW: Duration = Duration::from_secs(60);

/// Janela para rotas de autenticação: 10 requisições por 60 s em produção.
pub const AUTH_RATE_LIMIT: usize = if cfg!(test) { 10_000 } else { 10 };
pub const AUTH_RATE_WINDOW: Duration = Duration::from_secs(60);

/// Teto de chaves rastreadas em memória para evitar consumo desenfreado de RAM.
pub const MAX_TRACKED_ENTRIES: usize = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimitDecision {
    pub allowed: bool,
    pub remaining: usize,
    pub retry_after_seconds: u64,
}

/// Janela deslizante em memória — suficiente para um processo de API.
///
/// Não atravessa réplicas de propósito: o limite existe para conter abuso
/// interativo (um operador clicando repetidamente, um script mal escrito), e
/// não como defesa distribuída. Colocar isso no banco custaria uma escrita por
/// download de config.
pub struct SlidingWindowRateLimiter {
    limit: usize,
    window: Duration,
    hits: Mutex<HashMap<String, Vec<Instant>>>,
}

impl Default for SlidingWindowRateLimiter {
    fn default() -> Self {
        Self::new(RATE_LIMIT, RATE_WINDOW)
    }
}

impl SlidingWindowRateLimiter {
    #[must_use]
    pub fn new(limit: usize, window: Duration) -> Self {
        Self {
            limit,
            window,
            hits: Mutex::new(HashMap::new()),
        }
    }

    pub fn consume(&self, key: &str) -> RateLimitDecision {
        let now = Instant::now();
        let Ok(mut hits) = self.hits.lock() else {
            tracing::error!("RateLimiter: Mutex envenenado ao registrar consumo");
            // Mutex envenenado não pode bloquear o operador: deixa passar.
            return RateLimitDecision {
                allowed: true,
                remaining: self.limit,
                retry_after_seconds: 0,
            };
        };

        // Expurgo preventivo se atingir o teto de chaves rastreadas
        if hits.len() >= MAX_TRACKED_ENTRIES && !hits.contains_key(key) {
            hits.retain(|_, timestamps| {
                timestamps.retain(|hit| now.duration_since(*hit) < self.window);
                !timestamps.is_empty()
            });
            if hits.len() >= MAX_TRACKED_ENTRIES {
                if let Some(first_key) = hits.keys().next().cloned() {
                    hits.remove(&first_key);
                }
            }
        }

        let entry = hits.entry(key.to_string()).or_default();
        entry.retain(|hit| now.duration_since(*hit) < self.window);

        if entry.len() >= self.limit {
            let oldest = entry[0];
            let retry_after = self.window.saturating_sub(now.duration_since(oldest));
            return RateLimitDecision {
                allowed: false,
                remaining: 0,
                retry_after_seconds: retry_after.as_secs().max(1),
            };
        }

        entry.push(now);
        RateLimitDecision {
            allowed: true,
            remaining: self.limit - entry.len(),
            retry_after_seconds: 0,
        }
    }

    pub fn reset(&self) {
        if let Ok(mut hits) = self.hits.lock() {
            hits.clear();
        }
    }
}

/// Instância compartilhada pelos controllers sensíveis de VPN.
pub fn sensitive_endpoint_limiter() -> &'static SlidingWindowRateLimiter {
    static LIMITER: OnceLock<SlidingWindowRateLimiter> = OnceLock::new();
    LIMITER.get_or_init(SlidingWindowRateLimiter::default)
}

/// Detecta se o processo está rodando em suíte de testes.
pub fn is_test_environment() -> bool {
    cfg!(test)
        || std::env::var("LOCO_ENV")
            .map(|v| v == "test")
            .unwrap_or(false)
        || std::env::var("ENVIRONMENT")
            .map(|v| v == "test")
            .unwrap_or(false)
}

/// Instância compartilhada para proteção de endpoints de autenticação.
pub fn auth_endpoint_limiter() -> &'static SlidingWindowRateLimiter {
    static AUTH_LIMITER: OnceLock<SlidingWindowRateLimiter> = OnceLock::new();
    AUTH_LIMITER.get_or_init(|| {
        let limit = if is_test_environment() {
            100_000
        } else {
            AUTH_RATE_LIMIT
        };
        SlidingWindowRateLimiter::new(limit, AUTH_RATE_WINDOW)
    })
}

/// Extrai e sanitiza com segurança o IP de origem dos cabeçalhos HTTP.
pub fn extract_client_ip(headers: &axum::http::HeaderMap) -> String {
    let sanitize = |raw: &str| -> Option<String> {
        let cleaned: String = raw
            .chars()
            .filter(|c| !c.is_control() && !c.is_whitespace())
            .collect();
        if cleaned.is_empty() || cleaned.len() > 64 {
            None
        } else {
            Some(cleaned)
        }
    };

    if let Some(forwarded) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        if let Some(first) = forwarded.split(',').next() {
            if let Some(ip) = sanitize(first) {
                return ip;
            }
        }
    }

    if let Some(real_ip) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
        if let Some(ip) = sanitize(real_ip) {
            return ip;
        }
    }

    "desconhecido".to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VpnAuditAction {
    ConfigDownload,
    QrcodeDownload,
    KeyRotation,
    PeerRevoked,
    PeerCreated,
}

impl VpnAuditAction {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ConfigDownload => "config_download",
            Self::QrcodeDownload => "qrcode_download",
            Self::KeyRotation => "key_rotation",
            Self::PeerRevoked => "peer_revoked",
            Self::PeerCreated => "peer_created",
        }
    }
}

#[derive(Debug, Clone)]
pub struct VpnAuditEntry {
    pub action: VpnAuditAction,
    pub peer_id: Option<i64>,
    pub user_id: Option<String>,
    pub ip_address: Option<String>,
    pub details: serde_json::Value,
}

/// Trilha de auditoria: quem acessou, quando e qual peer.
///
/// Vai para o log estruturado (e não para uma tabela) porque o destino natural
/// é o coletor de logs da operação, onde já vivem as demais trilhas.
pub fn audit(entry: &VpnAuditEntry) {
    tracing::info!(
        audit = "vpn",
        action = entry.action.as_str(),
        peer_id = entry.peer_id,
        user_id = entry.user_id.as_deref().unwrap_or("anônimo"),
        request_ip = entry.ip_address.as_deref().unwrap_or("-"),
        details = %entry.details,
        "[VPN][auditoria] {} peer={}",
        entry.action.as_str(),
        entry.peer_id.map_or_else(|| "-".to_string(), |id| id.to_string())
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn libera_ate_o_limite_e_barra_depois() {
        let limiter = SlidingWindowRateLimiter::new(3, Duration::from_secs(60));
        for esperado in [2, 1, 0] {
            let decision = limiter.consume("user:1");
            assert!(decision.allowed);
            assert_eq!(decision.remaining, esperado);
        }
        let barrado = limiter.consume("user:1");
        assert!(!barrado.allowed);
        assert_eq!(barrado.remaining, 0);
        // `Retry-After` nunca é 0: o cliente precisa de um valor acionável.
        assert!(barrado.retry_after_seconds >= 1);
    }

    #[test]
    fn a_janela_e_por_solicitante() {
        let limiter = SlidingWindowRateLimiter::new(1, Duration::from_secs(60));
        assert!(limiter.consume("user:1").allowed);
        assert!(!limiter.consume("user:1").allowed);
        // Outro usuário tem sua própria janela.
        assert!(limiter.consume("user:2").allowed);
        assert!(limiter.consume("ip:203.0.113.9").allowed);
    }

    #[test]
    fn a_janela_desliza_e_libera_de_novo() {
        let limiter = SlidingWindowRateLimiter::new(1, Duration::from_millis(0));
        assert!(limiter.consume("user:1").allowed);
        // Com janela zerada, a batida anterior já saiu do intervalo.
        assert!(limiter.consume("user:1").allowed);
    }

    #[test]
    fn as_acoes_serializam_no_vocabulario_do_roadmap() {
        assert_eq!(VpnAuditAction::ConfigDownload.as_str(), "config_download");
        assert_eq!(VpnAuditAction::QrcodeDownload.as_str(), "qrcode_download");
        assert_eq!(VpnAuditAction::KeyRotation.as_str(), "key_rotation");
        assert_eq!(VpnAuditAction::PeerRevoked.as_str(), "peer_revoked");
        assert_eq!(VpnAuditAction::PeerCreated.as_str(), "peer_created");
    }
}
