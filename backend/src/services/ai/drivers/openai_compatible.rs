use std::{collections::HashMap, time::Instant};

use async_trait::async_trait;
use futures::StreamExt;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde_json::json;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::codec::{BytesCodec, FramedRead};

use super::traits::{AiChatChunk, AiChunkStream, AiDriver, AiMessage, AiTool, AiToolCall};
use crate::{
    dtos::ai::TestConnectionResponse,
    services::shared::errors::{AppError, AppResult},
};

pub struct OpenAiCompatibleDriver {
    pub driver_id: &'static str,
    pub display_name: &'static str,
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: String,
    pub extra_headers: Vec<(String, String)>,
    client: reqwest::Client,
}

impl OpenAiCompatibleDriver {
    pub fn new(
        driver_id: &'static str,
        display_name: &'static str,
        base_url: String,
        api_key: Option<String>,
        model: String,
        extra_headers: Vec<(String, String)>,
    ) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .unwrap_or_default();

        let trimmed_url = base_url.trim_end_matches('/').to_string();

        Self {
            driver_id,
            display_name,
            base_url: trimmed_url,
            api_key,
            model,
            extra_headers,
            client,
        }
    }

    fn endpoint_url(&self) -> String {
        if self.base_url.ends_with("/chat/completions") {
            self.base_url.clone()
        } else {
            format!("{}/chat/completions", self.base_url)
        }
    }

    fn build_headers(&self) -> AppResult<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        if let Some(key) = &self.api_key {
            let key_trimmed = key.trim();
            if !key_trimmed.is_empty() {
                let auth_val = format!("Bearer {key_trimmed}");
                headers.insert(
                    AUTHORIZATION,
                    HeaderValue::from_str(&auth_val)
                        .map_err(|e| AppError::validation(format!("API Key inválida: {e}")))?,
                );
            }
        }

        for (k, v) in &self.extra_headers {
            if let (Ok(h_name), Ok(h_val)) = (
                HeaderName::from_bytes(k.as_bytes()),
                HeaderValue::from_str(v),
            ) {
                headers.insert(h_name, h_val);
            }
        }

        Ok(headers)
    }
}

fn clean_provider_error_message(
    status: reqwest::StatusCode,
    body: &str,
    model: &str,
    display_name: &str,
) -> String {
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(err_obj) = val.get("error") {
            let msg = err_obj
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("");
            let raw_meta = err_obj
                .get("metadata")
                .and_then(|m| m.get("raw"))
                .and_then(|r| r.as_str())
                .unwrap_or("");

            if status == reqwest::StatusCode::TOO_MANY_REQUESTS
                || raw_meta.contains("rate-limited upstream")
            {
                return format!(
                    "O modelo '{model}' atingiu temporariamente o limite de taxa no {display_name} (HTTP 429). Dica: use o modelo 'openrouter/free' (roteador automático gratuito) que balanceia a carga."
                );
            }
            if status == reqwest::StatusCode::NOT_FOUND || msg.contains("unavailable for free") {
                return format!(
                    "O modelo '{model}' não está mais disponível gratuitamente no {display_name} (HTTP 404). Use o modelo 'openrouter/free' ou selecione outro no catálogo."
                );
            }
            if status == reqwest::StatusCode::UNAUTHORIZED {
                return format!(
                    "Chave de API inválida ou expirada no {display_name} (HTTP 401). Verifique a chave nas Configurações de IA."
                );
            }
            if !raw_meta.is_empty() {
                return format!("Provedor {display_name} (HTTP {status}): {raw_meta}");
            }
            if !msg.is_empty() {
                return format!("Provedor {display_name} (HTTP {status}): {msg}");
            }
        }
    }
    format!("Provedor {display_name} retornou HTTP {status}: {body}")
}

#[async_trait]
impl AiDriver for OpenAiCompatibleDriver {
    fn id(&self) -> &'static str {
        self.driver_id
    }

    fn display_name(&self) -> &'static str {
        self.display_name
    }

    async fn chat_stream(
        &self,
        messages: &[AiMessage],
        tools: &[AiTool],
    ) -> AppResult<AiChunkStream> {
        let url = self.endpoint_url();
        let headers = self.build_headers()?;

        let mut body = json!({
            "model": self.model,
            "messages": messages,
            "stream": true,
        });

        if !tools.is_empty() {
            body["tools"] = json!(tools);
        }

        let mut res = self
            .client
            .post(&url)
            .headers(headers.clone())
            .json(&body)
            .send()
            .await
            .map_err(|err| {
                AppError::service_unavailable(format!(
                    "Falha ao conectar ao provedor {}: {}",
                    self.display_name, err
                ))
            })?;

        let mut status = res.status();

        // Se o OpenRouter retornar 429 (rate-limited upstream) ou 404 para um modelo gratuito específico,
        // tenta o fallback automático para "openrouter/free" para não deixar o usuário sem resposta.
        if self.driver_id == "openrouter"
            && (status == reqwest::StatusCode::TOO_MANY_REQUESTS
                || status == reqwest::StatusCode::NOT_FOUND)
            && self.model != "openrouter/free"
        {
            tracing::warn!(
                model = %self.model,
                status = %status,
                "Modelo OpenRouter retornou erro; tentando fallback automático para 'openrouter/free'"
            );
            body["model"] = json!("openrouter/free");
            if let Ok(fallback_res) = self
                .client
                .post(&url)
                .headers(headers)
                .json(&body)
                .send()
                .await
            {
                if fallback_res.status().is_success() {
                    res = fallback_res;
                    status = res.status();
                }
            }
        }

        if !status.is_success() {
            let err_text = res
                .text()
                .await
                .unwrap_or_else(|_| "Erro desconhecido".into());
            let friendly =
                clean_provider_error_message(status, &err_text, &self.model, self.display_name);
            return Err(AppError::service_unavailable(friendly));
        }

        let (sender, receiver) = mpsc::channel(64);

        tokio::spawn(async move {
            let byte_stream = res
                .bytes_stream()
                .map(|item| item.map_err(std::io::Error::other));
            let reader = tokio_util::io::StreamReader::new(byte_stream);
            let mut framed = FramedRead::new(reader, BytesCodec::new());

            let mut pending_buffer = String::new();
            let mut accumulated_tools: HashMap<usize, (String, String, String)> = HashMap::new();

            while let Some(chunk_res) = framed.next().await {
                let bytes = match chunk_res {
                    Ok(b) => b,
                    Err(e) => {
                        let _ = sender
                            .send(Err(AppError::service_unavailable(format!(
                                "Erro no stream de dados: {e}"
                            ))))
                            .await;
                        return;
                    }
                };

                let text = match String::from_utf8(bytes.to_vec()) {
                    Ok(s) => s,
                    Err(_) => continue,
                };

                pending_buffer.push_str(&text);

                while let Some(pos) = pending_buffer.find('\n') {
                    let line = pending_buffer[..pos].trim().to_string();
                    pending_buffer = pending_buffer[pos + 1..].to_string();

                    if line.is_empty() || line.starts_with(':') {
                        continue;
                    }

                    if let Some(data_str) = line.strip_prefix("data: ") {
                        let data_str = data_str.trim();
                        if data_str == "[DONE]" {
                            if !accumulated_tools.is_empty() {
                                let mut calls = Vec::new();
                                for (_, (mut id, name, args)) in accumulated_tools.drain() {
                                    if id.is_empty() {
                                        id = format!(
                                            "call_{}",
                                            &uuid::Uuid::new_v4().simple().to_string()[..12]
                                        );
                                    }
                                    calls.push(AiToolCall {
                                        id,
                                        name,
                                        arguments: args,
                                    });
                                }
                                let _ = sender
                                    .send(Ok(AiChatChunk {
                                        text_delta: None,
                                        tool_calls: calls,
                                        finish_reason: Some("tool_calls".into()),
                                    }))
                                    .await;
                            }
                            let _ = sender
                                .send(Ok(AiChatChunk {
                                    text_delta: None,
                                    tool_calls: Vec::new(),
                                    finish_reason: Some("stop".into()),
                                }))
                                .await;
                            return;
                        }

                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data_str) {
                            if let Some(choices) = parsed.get("choices").and_then(|c| c.as_array())
                            {
                                if let Some(choice) = choices.first() {
                                    let delta = choice.get("delta");
                                    let finish_reason = choice
                                        .get("finish_reason")
                                        .and_then(|f| f.as_str())
                                        .map(ToString::to_string);

                                    let text_delta = delta
                                        .and_then(|d| d.get("content"))
                                        .and_then(|c| c.as_str())
                                        .map(ToString::to_string);

                                    if let Some(tool_calls_json) = delta
                                        .and_then(|d| d.get("tool_calls"))
                                        .and_then(|t| t.as_array())
                                    {
                                        for tc in tool_calls_json {
                                            let index = tc
                                                .get("index")
                                                .and_then(|i| i.as_u64())
                                                .unwrap_or(0)
                                                as usize;
                                            let id =
                                                tc.get("id").and_then(|s| s.as_str()).unwrap_or("");
                                            let func = tc.get("function");
                                            let name = func
                                                .and_then(|f| f.get("name"))
                                                .and_then(|n| n.as_str())
                                                .unwrap_or("");
                                            let args = func
                                                .and_then(|f| f.get("arguments"))
                                                .and_then(|a| a.as_str())
                                                .unwrap_or("");

                                            let entry = accumulated_tools
                                                .entry(index)
                                                .or_insert_with(|| {
                                                    (String::new(), String::new(), String::new())
                                                });
                                            if !id.is_empty() {
                                                entry.0 = id.to_string();
                                            }
                                            if !name.is_empty() {
                                                entry.1.push_str(name);
                                            }
                                            if !args.is_empty() {
                                                entry.2.push_str(args);
                                            }
                                        }
                                    }

                                    let mut calls = Vec::new();
                                    if finish_reason.as_deref() == Some("tool_calls")
                                        && !accumulated_tools.is_empty()
                                    {
                                        for (_, (mut id, name, args)) in accumulated_tools.drain() {
                                            if id.is_empty() {
                                                id = format!(
                                                    "call_{}",
                                                    &uuid::Uuid::new_v4().simple().to_string()
                                                        [..12]
                                                );
                                            }
                                            calls.push(AiToolCall {
                                                id,
                                                name,
                                                arguments: args,
                                            });
                                        }
                                    }

                                    if (text_delta.is_some()
                                        || !calls.is_empty()
                                        || finish_reason.is_some())
                                        && sender
                                            .send(Ok(AiChatChunk {
                                                text_delta,
                                                tool_calls: calls,
                                                finish_reason,
                                            }))
                                            .await
                                            .is_err()
                                    {
                                        return;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        Ok(Box::pin(ReceiverStream::new(receiver)))
    }

    async fn test_connection(&self) -> AppResult<TestConnectionResponse> {
        let url = self.endpoint_url();
        let headers = self.build_headers()?;

        let body = json!({
            "model": self.model,
            "messages": [
                {
                    "role": "user",
                    "content": "ping"
                }
            ],
            "max_tokens": 5,
            "stream": false
        });

        let start = Instant::now();
        let res = self
            .client
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|err| {
                AppError::service_unavailable(format!(
                    "Falha na conexão com {}: {}",
                    self.display_name, err
                ))
            })?;

        let latency_ms = (start.elapsed().as_secs_f64() * 1000.0).round();
        let status = res.status();

        if status.is_success() {
            Ok(TestConnectionResponse {
                success: true,
                latency_ms,
                message: format!(
                    "Conectado com sucesso a {} em {:.1}ms",
                    self.display_name, latency_ms
                ),
                model: Some(self.model.clone()),
            })
        } else {
            let err_body = res
                .text()
                .await
                .unwrap_or_else(|_| "Erro desconhecido".into());
            let friendly =
                clean_provider_error_message(status, &err_body, &self.model, self.display_name);
            Ok(TestConnectionResponse {
                success: false,
                latency_ms,
                message: friendly,
                model: Some(self.model.clone()),
            })
        }
    }
}
