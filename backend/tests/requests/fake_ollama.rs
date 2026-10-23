//! Um Ollama falso em `127.0.0.1:0` para os testes do Decisor.
//!
//! Lista o `laya:multilingual` como instalado e responde `/v1/systemone` pelo
//! `Responder` dado; a pergunta de aquecimento (`warmup`) é a carga do
//! modelo. Conta as chamadas e guarda
//! os estados recebidos, para o teste provar que houve (ou não) consulta.

// Cada arquivo de teste usa uma parte diferente do servidor falso.
#![allow(dead_code)]

use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use backend::services::ai::decider::config::AiDeciderSettings;
use serde_json::{json, Value};

/// Responde uma pergunta: (estado, id da pergunta, pergunta) → resposta.
pub type Responder = Arc<dyn Fn(&str, &str, &Value) -> Value + Send + Sync>;

#[derive(Clone)]
pub struct FakeOllama {
    pub base_url: String,
    pub calls: Arc<AtomicUsize>,
    pub states: Arc<Mutex<Vec<String>>>,
    /// Quantas vezes o modelo foi carregado (a pergunta de aquecimento).
    pub loads: Arc<AtomicUsize>,
}

impl FakeOllama {
    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    pub fn loads(&self) -> usize {
        self.loads.load(Ordering::SeqCst)
    }

    /// Configuração ligada apontando para este servidor.
    pub fn settings(&self) -> AiDeciderSettings {
        AiDeciderSettings {
            enabled: true,
            base_url: self.base_url.clone(),
            // O que o Ollama falso tem instalado (o padrão do Decisor é outro).
            model: "laya:322m-multilingual-bf16".into(),
            timeout_ms: 5_000,
            ..AiDeciderSettings::default()
        }
    }
}

#[derive(Clone)]
struct Shared {
    responder: Responder,
    calls: Arc<AtomicUsize>,
    states: Arc<Mutex<Vec<String>>>,
    loads: Arc<AtomicUsize>,
    loaded: Arc<AtomicBool>,
    load_delay: Duration,
}

/// Com o modelo já na memória — o caso de quase todos os testes.
pub async fn start(responder: Responder) -> FakeOllama {
    start_with(responder, true, Duration::ZERO).await
}

/// Com o modelo fora da memória: carregar leva `load_delay`.
pub async fn start_cold(responder: Responder, load_delay: Duration) -> FakeOllama {
    start_with(responder, false, load_delay).await
}

async fn start_with(responder: Responder, loaded: bool, load_delay: Duration) -> FakeOllama {
    async fn decide(
        State(shared): State<Shared>,
        Json(body): Json<Value>,
    ) -> (StatusCode, Json<Value>) {
        let warmup = body["questions"]
            .as_object()
            .is_some_and(|questions| questions.len() == 1 && questions.contains_key("warmup"));
        if warmup {
            // O Decisor carrega o modelo com uma pergunta de aquecimento.
            tokio::time::sleep(shared.load_delay).await;
            shared.loaded.store(true, Ordering::SeqCst);
            shared.loads.fetch_add(1, Ordering::SeqCst);
            return (
                StatusCode::OK,
                Json(json!({
                    "model": "laya:322m-multilingual-bf16",
                    "answers": { "warmup": { "type": "noul", "noul": 0.5 } },
                })),
            );
        }
        shared.calls.fetch_add(1, Ordering::SeqCst);
        let state = match &body["state"] {
            Value::String(text) => text.clone(),
            other => other.to_string(),
        };
        shared.states.lock().unwrap().push(state.clone());
        let answers: serde_json::Map<String, Value> = body["questions"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(id, question)| (id.clone(), (shared.responder)(&state, id, question)))
            .collect();
        (
            StatusCode::OK,
            Json(json!({ "model": "laya:322m-multilingual-bf16", "answers": answers })),
        )
    }

    async fn ps(State(shared): State<Shared>) -> Json<Value> {
        let models = if shared.loaded.load(Ordering::SeqCst) {
            json!([{ "name": "laya:322m-multilingual-bf16" }])
        } else {
            json!([])
        };
        Json(json!({ "models": models }))
    }

    let calls = Arc::new(AtomicUsize::new(0));
    let states = Arc::new(Mutex::new(Vec::new()));
    let loads = Arc::new(AtomicUsize::new(0));
    let app = Router::new()
        .route(
            "/api/tags",
            get(|| async {
                Json(json!({ "models": [{ "name": "laya:322m-multilingual-bf16" }] }))
            }),
        )
        .route("/api/ps", get(ps))
        .route("/v1/systemone", post(decide))
        .with_state(Shared {
            responder,
            calls: calls.clone(),
            states: states.clone(),
            loads: loads.clone(),
            loaded: Arc::new(AtomicBool::new(loaded)),
            load_delay,
        });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    FakeOllama {
        base_url: format!("http://{address}"),
        calls,
        states,
        loads,
    }
}

fn choice(label: &str, confidence: f64) -> Value {
    json!({
        "type": "choice",
        "choice": label,
        "confidence": confidence,
        "probabilities": { label: confidence },
    })
}

/// Responde por palavras-chave, sem acento de caixa:
///
/// - **escolha**: a primeira regra `(trecho, rótulo)` cujo trecho aparece no
///   estado e cujo rótulo está entre os critérios; com rótulo `"*"`, o
///   critério cuja descrição contém o trecho. Sem regra, o primeiro critério
///   com confiança baixa (0,2).
/// - **sim/não**: 0,93 quando uma regra `(trecho, id da pergunta)` casa com o
///   estado (trecho vazio casa sempre); 0,08 no resto.
pub fn by_keyword(rules: &[(&str, &str)]) -> Responder {
    let rules: Vec<(String, String)> = rules
        .iter()
        .map(|(needle, label)| (needle.to_lowercase(), (*label).to_string()))
        .collect();
    Arc::new(move |state, id, question| {
        let state = state.to_lowercase();
        match question["type"].as_str() {
            Some("choice") => {
                let criteria = question["criteria"]
                    .as_object()
                    .cloned()
                    .unwrap_or_default();
                for (needle, label) in &rules {
                    if label == "*" {
                        if let Some((key, _)) = criteria.iter().find(|(_, text)| {
                            text.as_str()
                                .is_some_and(|text| text.to_lowercase().contains(needle.as_str()))
                        }) {
                            return choice(key, 0.91);
                        }
                    } else if state.contains(needle.as_str()) && criteria.contains_key(label) {
                        return choice(label, 0.91);
                    }
                }
                let first = criteria.keys().next().cloned().unwrap_or_default();
                choice(&first, 0.2)
            }
            _ => {
                let yes = rules
                    .iter()
                    .any(|(needle, label)| label == id && state.contains(needle.as_str()));
                json!({ "type": "noul", "noul": if yes { 0.93 } else { 0.08 } })
            }
        }
    })
}
