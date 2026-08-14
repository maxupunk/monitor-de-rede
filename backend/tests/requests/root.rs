//! Contrato de `GET /api/info` e do prefixo `/api` (§5.6).

use backend::app::App;
use loco_rs::testing::prelude::*;
use serial_test::serial;

#[tokio::test]
#[serial]
async fn info_identifica_o_servico() {
    request::<App, _, _>(|request, _ctx| async move {
        let response = request.get("/api/info").await;

        assert_eq!(response.status_code(), 200);
        let body: serde_json::Value = serde_json::from_str(&response.text()).unwrap();
        assert_eq!(
            body,
            serde_json::json!({
                "status": "online",
                "service": "Network Monitor API",
                "version": "1.0.0"
            }),
            "o payload é observável — quem monitora a API de fora depende dele"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn a_raiz_fica_livre_para_a_spa() {
    request::<App, _, _>(|request, _ctx| async move {
        // Sem `dist` no ambiente de teste, `spa::mount` não monta nada e a raiz
        // responde 404. O que este teste protege é o inverso: que **não** haja
        // rota registrada em `/` — uma rota venceria o `fallback_service` da
        // SPA e devolveria JSON a quem abrisse o endereço no navegador.
        assert_eq!(request.get("/").await.status_code(), 404);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn rotas_de_negocio_ficam_sob_api() {
    request::<App, _, _>(|request, _ctx| async move {
        // O prefixo do controller é relativo (`/auth`); o `/api` vem do
        // `AppRoutes::prefix`. Se alguém duplicar o prefixo, isto vira 404.
        let response = request
            .post("/api/auth/login")
            .json(&serde_json::json!({ "email": "ninguem@exemplo.com", "password": "x" }))
            .await;
        assert_ne!(
            response.status_code(),
            404,
            "`/api/auth/login` não está registrado — prefixo duplicado ou ausente"
        );

        // E não vazou para a raiz.
        assert_eq!(request.post("/auth/login").await.status_code(), 404);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn health_check_do_loco_continua_na_raiz() {
    request::<App, _, _>(|request, _ctx| async move {
        // `_ping` é registrado por `with_default_routes()` antes do prefixo;
        // movê-lo para `/api/_ping` quebraria o monitoramento do próprio Loco.
        assert_eq!(request.get("/_ping").await.status_code(), 200);
    })
    .await;
}
