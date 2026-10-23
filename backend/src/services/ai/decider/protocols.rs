//! Os formatos de API dos provedores de decisão.
//!
//! As decisões falam só a língua interna ([`Question`] e [`DecideResponse`],
//! a do Ollaya). Cada formato traduz o pedido e a resposta, sem rede — o
//! cliente (`decider::client`) faz a chamada. Formato novo é um tipo novo que
//! implementa [`WireFormat`]; nada mais muda.
//!
//! | formato | caminho | diferenças da língua interna |
//! |---|---|---|
//! | Ollaya | `/api/decide` | nenhuma |
//! | System One (TypeSafe) | `/v1/systemone` | nenhuma (campos a mais na resposta são ignorados) |
//! | OpenRouter Decisions | `/alpha/decisions` | nenhuma: o corpo do System One em outro caminho |
//! | Cloudflare | `/accounts/{conta}/ai/run/@cf/cloudflare/{modelo}` | resposta dentro de `result` |
//! | OpenAI Decisions | `/v1/decisions` | `input`, perguntas em lista com `name`, `predicate`, `choices` e `probabilities` em lista |

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::{json, Value};

use super::{
    config::AiDeciderSettings,
    providers::DecisionProtocol,
    schema::{Answer, DecideRequest, DecideResponse, Question},
};
use crate::services::shared::errors::{AppError, AppResult};

/// Uma chamada pronta: para onde e com que corpo.
#[derive(Debug, Clone, PartialEq)]
pub struct WireRequest {
    pub url: String,
    pub body: Value,
}

pub trait WireFormat: Send + Sync {
    /// Monta a chamada.
    ///
    /// # Errors
    ///
    /// Falta um dado que o formato exige (o ID da conta na Cloudflare).
    fn request(
        &self,
        settings: &AiDeciderSettings,
        state: &str,
        questions: &BTreeMap<String, Question>,
    ) -> AppResult<WireRequest>;

    /// Lê a resposta para a língua interna.
    ///
    /// # Errors
    ///
    /// Corpo fora do formato.
    fn parse(&self, settings: &AiDeciderSettings, body: Value) -> AppResult<DecideResponse>;
}

/// O formato do provedor configurado.
#[must_use]
pub fn wire_for(protocol: DecisionProtocol) -> &'static dyn WireFormat {
    match protocol {
        DecisionProtocol::Ollaya => &Native("/api/decide"),
        DecisionProtocol::Systemone => &Native("/v1/systemone"),
        DecisionProtocol::Cloudflare => &Cloudflare,
        DecisionProtocol::Openai => &OpenAiDecisions,
        DecisionProtocol::Openrouter => &Native("/alpha/decisions"),
    }
}

fn url(settings: &AiDeciderSettings, path: &str) -> String {
    format!("{}{path}", settings.base_url.trim_end_matches('/'))
}

fn invalid(error: impl std::fmt::Display) -> AppError {
    AppError::service_unavailable(format!("Resposta fora do formato esperado: {error}"))
}

/// O corpo da língua interna, que Ollaya, TypeSafe e Cloudflare aceitam.
fn native_body(
    settings: &AiDeciderSettings,
    state: &str,
    questions: &BTreeMap<String, Question>,
) -> AppResult<Value> {
    serde_json::to_value(DecideRequest {
        model: &settings.model,
        state,
        questions,
    })
    .map_err(|error| AppError::Internal(anyhow::Error::new(error)))
}

/// Ollaya e System One: o mesmo corpo, caminhos diferentes.
struct Native(&'static str);

impl WireFormat for Native {
    fn request(
        &self,
        settings: &AiDeciderSettings,
        state: &str,
        questions: &BTreeMap<String, Question>,
    ) -> AppResult<WireRequest> {
        Ok(WireRequest {
            url: url(settings, self.0),
            body: native_body(settings, state, questions)?,
        })
    }

    fn parse(&self, _settings: &AiDeciderSettings, body: Value) -> AppResult<DecideResponse> {
        serde_json::from_value(body).map_err(invalid)
    }
}

/// Workers AI: o modelo vai na URL e no corpo; a resposta vem em `result`.
struct Cloudflare;

impl WireFormat for Cloudflare {
    fn request(
        &self,
        settings: &AiDeciderSettings,
        state: &str,
        questions: &BTreeMap<String, Question>,
    ) -> AppResult<WireRequest> {
        let Some(account) = settings.account_id.as_deref() else {
            return Err(AppError::validation(
                "Informe o ID da conta da Cloudflare (painel da Cloudflare, Workers AI).",
            ));
        };
        Ok(WireRequest {
            url: url(
                settings,
                &format!(
                    "/accounts/{account}/ai/run/@cf/cloudflare/{}",
                    settings.model
                ),
            ),
            body: native_body(settings, state, questions)?,
        })
    }

    fn parse(&self, settings: &AiDeciderSettings, mut body: Value) -> AppResult<DecideResponse> {
        let inner = body.get_mut("result").map_or(Value::Null, Value::take);
        Native("").parse(settings, if inner.is_null() { body } else { inner })
    }
}

/// OpenAI Decisions: perguntas em lista, `predicate` no lugar de `noul`.
struct OpenAiDecisions;

#[derive(Deserialize)]
struct OpenAiResponse {
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    answers: Vec<OpenAiAnswer>,
}

#[derive(Deserialize)]
struct OpenAiAnswer {
    #[serde(rename = "type")]
    kind: String,
    name: String,
    #[serde(default)]
    probability: Option<f64>,
    #[serde(default)]
    choice: Option<Value>,
    #[serde(default)]
    confidence: Option<f64>,
    #[serde(default)]
    probabilities: Vec<OpenAiOption>,
}

#[derive(Deserialize)]
struct OpenAiOption {
    value: Value,
    probability: f64,
}

/// O valor de uma opção como rótulo (a OpenAI aceita texto ou booleano).
fn label(value: &Value) -> String {
    value
        .as_str()
        .map_or_else(|| value.to_string(), str::to_string)
}

impl WireFormat for OpenAiDecisions {
    fn request(
        &self,
        settings: &AiDeciderSettings,
        state: &str,
        questions: &BTreeMap<String, Question>,
    ) -> AppResult<WireRequest> {
        let questions: Vec<Value> = questions
            .iter()
            .map(|(name, question)| match question {
                Question::Noul { instructions } => json!({
                    "type": "predicate",
                    "name": name,
                    "instructions": instructions,
                }),
                Question::Choice {
                    instructions,
                    criteria,
                } => json!({
                    "type": "choice",
                    "name": name,
                    "instructions": instructions,
                    "choices": criteria
                        .iter()
                        .map(|(value, description)| json!({ "value": value, "description": description }))
                        .collect::<Vec<_>>(),
                }),
            })
            .collect();
        Ok(WireRequest {
            url: url(settings, "/v1/decisions"),
            body: json!({ "model": settings.model, "input": state, "questions": questions }),
        })
    }

    fn parse(&self, settings: &AiDeciderSettings, body: Value) -> AppResult<DecideResponse> {
        let response: OpenAiResponse = serde_json::from_value(body).map_err(invalid)?;
        let answers = response
            .answers
            .into_iter()
            .map(|answer| {
                let parsed = match answer.kind.as_str() {
                    "predicate" => answer
                        .probability
                        .map_or(Answer::Other, |noul| Answer::Noul { noul }),
                    "choice" => match answer.choice {
                        Some(choice) => Answer::Choice {
                            choice: label(&choice),
                            confidence: answer.confidence.unwrap_or(0.0),
                            probabilities: answer
                                .probabilities
                                .iter()
                                .map(|option| (label(&option.value), option.probability))
                                .collect(),
                        },
                        None => Answer::Other,
                    },
                    // Recusa e tipos que o NetMonitor não pergunta.
                    _ => Answer::Other,
                };
                (answer.name, parsed)
            })
            .collect();
        Ok(DecideResponse {
            model: response.model.unwrap_or_else(|| settings.model.clone()),
            answers,
        })
    }
}

/// A mensagem de erro de qualquer provedor: `{"error":"…"}` (Ollaya),
/// `{"error":{"message":"…"}}` (OpenAI), `{"message":"…"}` (TypeSafe,
/// Vercel) ou `{"errors":[{"message":"…"}]}` (Cloudflare). Sem nenhum
/// desses, o corpo como veio.
#[must_use]
pub fn error_message(body: &str) -> String {
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return body.trim().to_string();
    };
    let text = |value: Option<&Value>| value.and_then(Value::as_str).map(str::to_string);
    let message = text(value.get("error"))
        .or_else(|| text(value.pointer("/error/message")))
        .or_else(|| text(value.get("message")))
        .or_else(|| text(value.pointer("/errors/0/message")));
    let code = text(value.get("code")).or_else(|| text(value.pointer("/error/code")));
    match (message, code) {
        (Some(message), Some(code)) => format!("{message} ({code})"),
        (Some(message), None) => message,
        _ => body.trim().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::ai::decider::providers::DeciderProvider;

    fn questions() -> BTreeMap<String, Question> {
        BTreeMap::from([
            (
                "urgent".to_string(),
                Question::Noul {
                    instructions: "Is it urgent?".into(),
                },
            ),
            (
                "team".to_string(),
                Question::Choice {
                    instructions: "Which team?".into(),
                    criteria: BTreeMap::from([
                        ("billing".to_string(), "Payments".to_string()),
                        ("network".to_string(), "Links".to_string()),
                    ]),
                },
            ),
        ])
    }

    fn settings(provider: DeciderProvider) -> AiDeciderSettings {
        AiDeciderSettings {
            provider,
            base_url: String::new(),
            model: String::new(),
            account_id: Some("abc123".into()),
            ..AiDeciderSettings::default()
        }
        .normalized()
    }

    #[test]
    fn system_one_e_o_corpo_do_ollaya_em_outro_caminho() {
        let typesafe = settings(DeciderProvider::Typesafe);
        let request = wire_for(typesafe.protocol())
            .request(&typesafe, "o link caiu", &questions())
            .unwrap();
        assert_eq!(request.url, "https://api.typesafe.ai/v1/systemone");
        assert_eq!(request.body["model"], "jev-latest");
        assert_eq!(request.body["questions"]["urgent"]["type"], "noul");
        assert_eq!(
            request.body["questions"]["team"]["criteria"]["billing"],
            "Payments"
        );

        let resposta = wire_for(typesafe.protocol())
            .parse(
                &typesafe,
                json!({
                    "model": "jev-1.13.0",
                    "answers": { "urgent": { "type": "noul", "noul": 0.95 } },
                    "usage": { "input_tokens": 296, "output_tokens": 20 }
                }),
            )
            .unwrap();
        assert_eq!(resposta.model, "jev-1.13.0");
        assert_eq!(resposta.answers["urgent"].yes(), Some(0.95));
    }

    #[test]
    fn openrouter_usa_o_caminho_de_decisoes_com_o_corpo_do_system_one() {
        let openrouter = settings(DeciderProvider::Openrouter);
        let request = wire_for(openrouter.protocol())
            .request(&openrouter, "s", &questions())
            .unwrap();
        assert_eq!(request.url, "https://openrouter.ai/api/alpha/decisions");
        assert_eq!(request.body["model"], "microsoft/microsoft-decision-1");
        assert_eq!(request.body["questions"]["urgent"]["type"], "noul");
    }

    #[test]
    fn cloudflare_poe_conta_e_modelo_na_url_e_tira_o_envelope() {
        let cloudflare = settings(DeciderProvider::Cloudflare);
        let request = wire_for(cloudflare.protocol())
            .request(&cloudflare, "s", &questions())
            .unwrap();
        assert_eq!(
            request.url,
            "https://api.cloudflare.com/client/v4/accounts/abc123/ai/run/@cf/cloudflare/clef-flash"
        );
        assert_eq!(request.body["model"], "clef-flash");

        let resposta = wire_for(cloudflare.protocol())
            .parse(
                &cloudflare,
                json!({
                    "result": { "model": "clef-flash", "answers": { "urgent": { "type": "noul", "noul": 0.3 } } },
                    "success": true, "errors": [], "messages": []
                }),
            )
            .unwrap();
        assert_eq!(resposta.answers["urgent"].yes(), Some(0.3));
    }

    #[test]
    fn cloudflare_sem_conta_diz_o_que_falta() {
        let sem_conta = AiDeciderSettings {
            account_id: None,
            ..settings(DeciderProvider::Cloudflare)
        };
        let erro = wire_for(sem_conta.protocol())
            .request(&sem_conta, "s", &questions())
            .unwrap_err();
        assert!(erro.to_string().contains("ID da conta"));
    }

    #[test]
    fn openai_traduz_perguntas_e_respostas() {
        let openai = settings(DeciderProvider::Openai);
        let request = wire_for(openai.protocol())
            .request(&openai, "o link caiu", &questions())
            .unwrap();
        assert_eq!(request.url, "https://api.openai.com/v1/decisions");
        assert_eq!(request.body["input"], "o link caiu");
        assert_eq!(request.body["model"], "gpt-6-luna");
        let perguntas = request.body["questions"].as_array().unwrap();
        assert_eq!(perguntas[0]["name"], "team");
        assert_eq!(perguntas[0]["choices"][1]["value"], "network");
        assert_eq!(perguntas[1]["type"], "predicate");

        let resposta = wire_for(openai.protocol())
            .parse(
                &openai,
                json!({
                    "answers": [
                        { "type": "predicate", "name": "urgent", "probability": 0.9 },
                        { "type": "choice", "name": "team", "choice": "network", "confidence": 0.7,
                          "probabilities": [ { "value": "billing", "probability": 0.2 }, { "value": "network", "probability": 0.8 } ] },
                        { "type": "refusal", "name": "outra" }
                    ],
                    "usage": { "input_tokens": 96 }
                }),
            )
            .unwrap();
        assert_eq!(resposta.model, "gpt-6-luna");
        assert_eq!(resposta.answers["urgent"].yes(), Some(0.9));
        assert_eq!(resposta.answers["team"].choice(), Some(("network", 0.7)));
        assert_eq!(
            resposta.answers["team"].probability_of("billing"),
            Some(0.2)
        );
        assert_eq!(resposta.answers["outra"], Answer::Other);
    }

    #[test]
    fn mensagem_de_erro_de_cada_provedor() {
        assert_eq!(
            error_message(r#"{"error":"model not found","code":"NOT_FOUND"}"#),
            "model not found (NOT_FOUND)"
        );
        assert_eq!(
            error_message(r#"{"error":{"message":"Incorrect API key","code":"invalid_api_key"}}"#),
            "Incorrect API key (invalid_api_key)"
        );
        assert_eq!(
            error_message(r#"{"message":"bad","error_type":"invalid_request"}"#),
            "bad"
        );
        assert_eq!(
            error_message(
                r#"{"errors":[{"code":10000,"message":"Authentication error"}],"success":false}"#
            ),
            "Authentication error"
        );
        assert_eq!(error_message("  gateway timeout "), "gateway timeout");
    }
}
