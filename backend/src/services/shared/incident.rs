//! Todo erro interno vira um **incidente**: um código curto que aparece na
//! tela e no log, ligado ao método, ao caminho e à causa.
//!
//! Sem isto, um 500 chegava ao operador de três jeitos diferentes — o corpo do
//! nosso `AppError`, o `internal_server_error: Internal Server Error` do Loco
//! (erro do framework ou pânico pego pelo `CatchPanic`) ou a conexão cortada —
//! e nenhum deles dizia **onde** procurar. Aqui os três convergem:
//!
//! * o middleware [`track`] dá um código a cada requisição (num `task_local`),
//!   pega o pânico que escapar do handler e troca **todo** corpo de 500 pelo
//!   nosso, com o código;
//! * o gancho de pânico ([`install_panic_hook`]) registra mensagem e
//!   `arquivo:linha` com o código da requisição em andamento — o que o
//!   `CatchPanic` do Loco não registra;
//! * o `AppError::Internal` registra a causa com o mesmo código
//!   ([`current_incident`]).
//!
//! O detalhe continua só no log: a tela recebe o código e onde procurá-lo.

use std::{any::Any, panic::AssertUnwindSafe, sync::Once};

use axum::{
    extract::Request,
    http::{header::HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use futures::FutureExt;

/// Cabeçalho com o código, para quem lê a resposta sem o corpo.
pub const INCIDENT_HEADER: &str = "x-incident-id";

#[derive(Debug, Clone)]
struct RequestInfo {
    incident: String,
    method: String,
    path: String,
}

tokio::task_local! {
    static REQUEST: RequestInfo;
}

/// O código do incidente da requisição em andamento, se houver uma.
#[must_use]
pub fn current_incident() -> Option<String> {
    REQUEST.try_with(|info| info.incident.clone()).ok()
}

/// Código curto e legível: 8 caracteres hexadecimais em maiúsculas.
fn new_code() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..8].to_ascii_uppercase()
}

/// A mensagem que a tela mostra para qualquer 500.
#[must_use]
pub fn public_message(incident: &str) -> String {
    format!(
        "Erro interno do servidor (incidente {incident}). O detalhe ficou no log da aplicação: \
         procure por {incident} em Logs › Este servidor ou no `docker compose logs netmonitor`."
    )
}

fn incident_response(incident: &str) -> Response {
    let mut response = (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({
            "message": public_message(incident),
            "incidentId": incident,
        })),
    )
        .into_response();
    if let Ok(value) = HeaderValue::from_str(incident) {
        response.headers_mut().insert(INCIDENT_HEADER, value);
    }
    response
}

/// Marca, na resposta, que a causa já foi registrada com o código — pelo
/// `AppError::Internal`. Sem a marca, o middleware registra o 500 ele mesmo.
#[derive(Debug, Clone, Copy)]
pub struct IncidentLogged;

/// Middleware de toda a aplicação. Ver a nota do módulo.
pub async fn track(request: Request, next: Next) -> Response {
    let info = RequestInfo {
        incident: new_code(),
        method: request.method().to_string(),
        path: request.uri().path().to_string(),
    };
    let outcome = REQUEST
        .scope(
            info.clone(),
            AssertUnwindSafe(next.run(request)).catch_unwind(),
        )
        .await;
    let response = match outcome {
        Ok(response) => response,
        // A causa (mensagem e `arquivo:linha`) já foi registrada pelo gancho.
        Err(_) => return incident_response(&info.incident),
    };
    if response.status() != StatusCode::INTERNAL_SERVER_ERROR {
        return response;
    }
    if response.extensions().get::<IncidentLogged>().is_none() {
        // 500 que não passou pelo nosso `AppError`: erro do framework ou
        // pânico pego pelo `CatchPanic` do Loco (o gancho já deu o local).
        tracing::error!(
            incident = %info.incident,
            method = %info.method,
            path = %info.path,
            "erro interno fora do AppError (framework ou pânico)"
        );
    }
    incident_response(&info.incident)
}

/// Texto de um pânico, que pode ser `&str`, `String` ou outra coisa.
fn panic_text(payload: &(dyn Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(ToString::to_string)
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "pânico sem mensagem".to_string())
}

/// Registra todo pânico do processo pelo `tracing` — com o local e, dentro de
/// uma requisição, o código do incidente. O gancho anterior (o que escreve no
/// stderr) continua sendo chamado.
pub fn install_panic_hook() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let location = info.location().map_or_else(
                || "local desconhecido".to_string(),
                |location| format!("{}:{}", location.file(), location.line()),
            );
            let message = panic_text(info.payload());
            match REQUEST.try_with(Clone::clone) {
                Ok(request) => tracing::error!(
                    incident = %request.incident,
                    method = %request.method,
                    path = %request.path,
                    panic = %message,
                    location = %location,
                    "pânico ao atender a requisição"
                ),
                Err(_) => tracing::error!(
                    panic = %message,
                    location = %location,
                    "pânico fora de requisição"
                ),
            }
            previous(info);
        }));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::to_bytes, routing::get, Router};
    use tower::ServiceExt;

    async fn call(router: Router) -> (StatusCode, Option<String>, serde_json::Value) {
        let response = router
            .layer(axum::middleware::from_fn(track))
            .oneshot(
                Request::builder()
                    .uri("/x")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let header = response
            .headers()
            .get(INCIDENT_HEADER)
            .map(|value| value.to_str().unwrap().to_string());
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (
            status,
            header,
            serde_json::from_slice(&bytes).unwrap_or_default(),
        )
    }

    #[tokio::test]
    async fn panico_no_handler_vira_500_com_codigo_e_nao_derruba_a_conexao() {
        install_panic_hook();
        let router = Router::new().route(
            "/x",
            get(|| async {
                let vazio: Vec<u8> = Vec::new();
                // Índice fora do vetor: o pânico típico de produção.
                #[allow(clippy::indexing_slicing)]
                let _ = vazio[3];
                "nunca"
            }),
        );
        let (status, header, body) = call(router).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        let code = header.expect("cabeçalho do incidente");
        assert_eq!(body["incidentId"], code);
        assert!(body["message"].as_str().unwrap().contains(&code));
    }

    #[tokio::test]
    async fn quinhentos_do_framework_ganha_o_nosso_corpo_com_codigo() {
        let router = Router::new().route(
            "/x",
            get(|| async { loco_rs::Error::InternalServerError.into_response() }),
        );
        let (status, header, body) = call(router).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(header.is_some());
        assert!(body.get("error").is_none(), "nada do corpo do Loco: {body}");
        assert!(body["message"]
            .as_str()
            .unwrap()
            .starts_with("Erro interno"));
    }

    #[tokio::test]
    async fn o_app_error_interno_registra_com_o_mesmo_codigo_da_resposta() {
        let visto = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
        let no_handler = visto.clone();
        let router = Router::new().route(
            "/x",
            get(move || async move {
                *no_handler.lock().unwrap() = current_incident();
                crate::services::shared::errors::AppError::Internal(anyhow::anyhow!("falhou"))
                    .into_response()
            }),
        );
        let (status, header, body) = call(router).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        let visto = visto
            .lock()
            .unwrap()
            .clone()
            .expect("código dentro da requisição");
        assert_eq!(
            header.as_deref(),
            Some(visto.as_str()),
            "o log e a tela citam o mesmo incidente"
        );
        assert_eq!(body["incidentId"], visto);
    }

    #[tokio::test]
    async fn respostas_que_nao_sao_500_passam_intactas() {
        let router =
            Router::new().route("/x", get(|| async { (StatusCode::BAD_REQUEST, "regra") }));
        let (status, header, _) = call(router).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(header.is_none());
    }
}
