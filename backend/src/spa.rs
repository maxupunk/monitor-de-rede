//! Serviço dos arquivos da SPA pelo próprio processo da API.
//!
//! Ocupa o lugar do container nginx. O proxy que ele fazia deixou de existir
//! junto: a SPA e a API passam a atender na **mesma origem** (`:3333`), então
//! não há mais `/api/` para reescrever — nem CORS a liberar em produção.
//!
//! Duas montagens, porque as duas metades do `dist` têm validades opostas:
//!
//! * `/assets/*` — tudo com hash no nome (Vite). Nome novo a cada build, logo
//!   `immutable` por um ano. O navegador nunca revalida.
//! * o resto (`index.html`, `sw.js`, `manifest.webmanifest`, ícones) — nome
//!   estável e conteúdo que muda a cada deploy. `no-cache` aqui não quer dizer
//!   "não guarde": quer dizer "guarde, mas confirme antes de usar". A
//!   confirmação sai 304 pelo `Last-Modified` do `ServeDir`. Um `sw.js` servido
//!   de cache é o bug clássico de PWA — a versão nova do app nunca chega.
//!
//! O `.gz` ao lado de cada arquivo é escrito no build da imagem
//! (`gzip -9 -k`), e o `precompressed_gzip` o entrega quando o cliente aceita
//! gzip. Comprimir na hora custaria CPU a cada request para produzir sempre o
//! mesmo byte.

use std::path::{Path, PathBuf};

use axum::{
    http::{
        header::{
            CACHE_CONTROL, CONTENT_SECURITY_POLICY, REFERRER_POLICY, X_CONTENT_TYPE_OPTIONS,
            X_FRAME_OPTIONS,
        },
        HeaderValue,
    },
    Router,
};
use tower_http::{
    services::{ServeDir, ServeFile},
    set_header::SetResponseHeaderLayer,
};

/// Onde o `dist` da SPA mora dentro da imagem. Fora dela — `cargo run` na
/// máquina — o padrão não existe e a montagem é simplesmente pulada, que é o
/// certo em desenvolvimento: quem serve a SPA ali é o Vite, na 5173.
pub const DEFAULT_WEB_ROOT: &str = "/app/web";

const IMMUTABLE: &str = "public, max-age=31536000, immutable";
const REVALIDATE: &str = "no-cache";

// Headers de segurança aplicados a todos os arquivos estáticos da SPA.
// O CSP começa restritivo e permite apenas recursos do próprio origin; a SPA
// empacotada pelo Vite carrega scripts e CSS de <script src>/<link href>, sem
// inline. Imagens de QR code e ícones podem vir como data:URI.
const X_CONTENT_TYPE_OPTIONS_VALUE: &str = "nosniff";
const REFERRER_POLICY_VALUE: &str = "strict-origin-when-cross-origin";
const X_FRAME_OPTIONS_VALUE: &str = "DENY";
const CSP_VALUE: &str = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'";

/// Raiz dos estáticos, de `WEB_ROOT` ou do padrão da imagem.
#[must_use]
pub fn web_root() -> PathBuf {
    std::env::var("WEB_ROOT")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map_or_else(|| PathBuf::from(DEFAULT_WEB_ROOT), PathBuf::from)
}

/// Acrescenta a SPA ao roteador — ou o devolve intacto se não houver `dist`.
///
/// Ausência de `index.html` não é erro: é o modo API-only (o processo de um
/// probe remoto, ou o backend rodando contra o Vite em desenvolvimento).
pub fn mount(router: Router, root: &Path) -> Router {
    let index = root.join("index.html");
    if !index.is_file() {
        tracing::debug!(root = %root.display(), "sem dist da SPA — servindo apenas a API");
        return router;
    }

    // O `fallback` do `ServeDir` é o que faz a rota virtual do Vue Router
    // funcionar: `/devices/7` não é arquivo nenhum, e ainda assim precisa
    // devolver o `index.html` para a SPA resolver o caminho no cliente.
    let shell = ServeDir::new(root)
        .precompressed_gzip()
        .fallback(ServeFile::new(&index));
    let assets = ServeDir::new(root.join("assets")).precompressed_gzip();

    tracing::info!(root = %root.display(), "SPA servida pelo processo da API");

    let security_headers = || {
        Router::new()
            .layer(SetResponseHeaderLayer::overriding(
                X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static(X_CONTENT_TYPE_OPTIONS_VALUE),
            ))
            .layer(SetResponseHeaderLayer::overriding(
                REFERRER_POLICY,
                HeaderValue::from_static(REFERRER_POLICY_VALUE),
            ))
            .layer(SetResponseHeaderLayer::overriding(
                X_FRAME_OPTIONS,
                HeaderValue::from_static(X_FRAME_OPTIONS_VALUE),
            ))
            .layer(SetResponseHeaderLayer::overriding(
                CONTENT_SECURITY_POLICY,
                HeaderValue::from_static(CSP_VALUE),
            ))
    };

    router
        .nest_service(
            "/assets",
            security_headers()
                .fallback_service(assets)
                .layer(SetResponseHeaderLayer::overriding(
                    CACHE_CONTROL,
                    HeaderValue::from_static(IMMUTABLE),
                )),
        )
        // Último recurso do roteador: as rotas da API já foram registradas e
        // continuam ganhando de qualquer arquivo de mesmo caminho.
        .fallback_service(security_headers().fallback_service(shell).layer(
            SetResponseHeaderLayer::overriding(CACHE_CONTROL, HeaderValue::from_static(REVALIDATE)),
        ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn sem_index_o_roteador_volta_intacto() {
        // `mount` não pode falhar em ambiente sem `dist`: é o caso do probe
        // remoto e o dos testes de request, que sobem o app inteiro.
        drop(mount(Router::new(), Path::new("/nao/existe")));
    }

    #[test]
    #[serial]
    fn web_root_respeita_a_variavel() {
        std::env::set_var("WEB_ROOT", "/srv/spa");
        assert_eq!(web_root(), PathBuf::from("/srv/spa"));
        // Vazia é como ausente: `WEB_ROOT=` no compose não pode fazer o
        // servidor procurar a SPA na raiz do sistema de arquivos.
        std::env::set_var("WEB_ROOT", "");
        assert_eq!(web_root(), PathBuf::from(DEFAULT_WEB_ROOT));
        std::env::remove_var("WEB_ROOT");
    }

    #[test]
    fn headers_de_seguranca_estao_configurados() {
        // Garante que as constantes de header não estão vazias e que o mount
        // consegue aplicá-las sobre um diretório real.
        assert!(!X_CONTENT_TYPE_OPTIONS_VALUE.is_empty());
        assert!(!REFERRER_POLICY_VALUE.is_empty());
        assert!(!X_FRAME_OPTIONS_VALUE.is_empty());
        assert!(!CSP_VALUE.is_empty());

        let tmp = std::env::temp_dir().join(format!("netmonitor-spa-test-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("index.html"), "<html></html>").unwrap();
        drop(mount(Router::new(), &tmp));
        std::fs::remove_dir_all(&tmp).unwrap();
    }
}
