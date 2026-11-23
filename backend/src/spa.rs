//! Serviço dos arquivos da SPA (Flutter Web PWA) pelo próprio processo da API.
//!
//! Ocupa o lugar do container nginx. O proxy que ele fazia deixou de existir
//! junto: a SPA e a API passam a atender na **mesma origem** (`:3333`), então
//! não há mais `/api/` para reescrever — nem CORS a liberar em produção.
//!
//! Os arquivos gerados pelo `flutter build web --no-web-resources-cdn`
//! (`index.html`, `flutter_bootstrap.js`, `main.dart.js`, `manifest.json`,
//! `assets/*`, `canvaskit/*`) usam nomes estáveis sem hash de conteúdo. Por
//! isso tudo é servido com `Cache-Control: no-cache`: o navegador armazena em
//! cache mas confirma com o servidor antes de usar, recebendo `304 Not
//! Modified` via `Last-Modified` do `ServeDir` quando não houve deploy e a
//! versão nova imediatamente quando houve.
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
/// certo em desenvolvimento: quem serve a SPA ali é o `flutter run -d chrome`.
pub const DEFAULT_WEB_ROOT: &str = "/app/web";

const REVALIDATE: &str = "no-cache";

// Headers de segurança aplicados a todos os arquivos estáticos da SPA.
// O Flutter Web empacotado com `--no-web-resources-cdn` carrega todos os
// scripts, fontes e CanvasKit da própria origem:
// - `'wasm-unsafe-eval'` permite compilar/instanciar `canvaskit.wasm` sem abrir
//   `'unsafe-eval'` para JavaScript;
// - `worker-src 'self' blob:` atende o service worker e workers locais;
// - `connect-src` / `font-src` / `img-src` permitem `data:` e `blob:` para
//   CanvasKit, QR codes e downloads;
// - `form-action 'self' https://github.com` permite o cadastro do GitHub App
//   via Manifest (ADR 021).
const X_CONTENT_TYPE_OPTIONS_VALUE: &str = "nosniff";
const REFERRER_POLICY_VALUE: &str = "strict-origin-when-cross-origin";
const X_FRAME_OPTIONS_VALUE: &str = "DENY";
const CSP_VALUE: &str = "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data: blob:; connect-src 'self' data: blob:; worker-src 'self' blob:; frame-ancestors 'none'; base-uri 'self'; form-action 'self' https://github.com";

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
/// probe remoto, ou o backend rodando contra o `flutter run` em desenvolvimento).
pub fn mount(router: Router, root: &Path) -> Router {
    let index = root.join("index.html");
    if !index.is_file() {
        tracing::debug!(root = %root.display(), "sem dist da SPA — servindo apenas a API");
        return router;
    }

    // O `fallback` do `ServeDir` é o que faz as rotas virtuais do GoRouter
    // funcionarem: `/devices/7` não é arquivo nenhum, e ainda assim precisa
    // devolver o `index.html` para a SPA resolver o caminho no cliente.
    let shell = ServeDir::new(root)
        .precompressed_gzip()
        .fallback(ServeFile::new(&index));

    tracing::info!(root = %root.display(), "SPA servida pelo processo da API");

    // `.fallback_service(shell)` precisa vir antes das camadas `.layer(...)`:
    // no Axum, `Router::layer` só envolve rotas/fallbacks já registrados;
    // chamar `.fallback_service` depois substituiria o fallback e descartaria
    // os headers de segurança (CSP, X-Content-Type-Options, etc.).
    let spa_service = Router::new()
        .fallback_service(shell)
        .layer(SetResponseHeaderLayer::overriding(
            CACHE_CONTROL,
            HeaderValue::from_static(REVALIDATE),
        ))
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
        ));

    // Último recurso do roteador: as rotas da API já foram registradas e
    // continuam ganhando de qualquer arquivo de mesmo caminho.
    router.fallback_service(spa_service)
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
        // Garante que as constantes de header estão presentes e compatíveis com
        // o Flutter Web CanvasKit (WASM local) e o manifest do GitHub App.
        assert!(!X_CONTENT_TYPE_OPTIONS_VALUE.is_empty());
        assert!(!REFERRER_POLICY_VALUE.is_empty());
        assert!(!X_FRAME_OPTIONS_VALUE.is_empty());
        assert!(CSP_VALUE.contains("'wasm-unsafe-eval'"));
        assert!(CSP_VALUE.contains("https://github.com"));

        let tmp = std::env::temp_dir().join(format!("netmonitor-spa-test-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("index.html"), "<html></html>").unwrap();
        drop(mount(Router::new(), &tmp));
        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[tokio::test]
    async fn serve_netmonitor_push_sw_com_headers_e_csp() {
        use axum::{body::Body, http::Request, http::StatusCode};
        use tower::ServiceExt;

        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("index.html"), "<html></html>").unwrap();
        std::fs::write(
            tmp.path().join("netmonitor_push_sw.js"),
            "self.addEventListener('push', function () {});",
        )
        .unwrap();

        let app = mount(Router::new(), tmp.path());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/netmonitor_push_sw.js")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let headers = response.headers();
        assert_eq!(
            headers.get(CACHE_CONTROL).and_then(|v| v.to_str().ok()),
            Some(REVALIDATE)
        );
        assert_eq!(
            headers
                .get(X_CONTENT_TYPE_OPTIONS)
                .and_then(|v| v.to_str().ok()),
            Some(X_CONTENT_TYPE_OPTIONS_VALUE)
        );
        assert_eq!(
            headers.get(REFERRER_POLICY).and_then(|v| v.to_str().ok()),
            Some(REFERRER_POLICY_VALUE)
        );
        assert_eq!(
            headers.get(X_FRAME_OPTIONS).and_then(|v| v.to_str().ok()),
            Some(X_FRAME_OPTIONS_VALUE)
        );
        let csp = headers
            .get(CONTENT_SECURITY_POLICY)
            .and_then(|v| v.to_str().ok())
            .expect("CSP header deve estar presente");
        assert!(csp.contains("worker-src 'self' blob:"));
        let content_type = headers
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .expect("Content-Type deve estar presente");
        assert!(
            content_type.contains("javascript"),
            "esperado MIME JS, veio: {content_type}"
        );
    }
}
