use std::{
    collections::BTreeMap,
    sync::{Arc, OnceLock},
    time::Instant,
};

use async_trait::async_trait;
use futures::StreamExt;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde_json::json;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::codec::{BytesCodec, FramedRead};

use super::traits::{
    AiChatChunk, AiChatOptions, AiChunkStream, AiDriver, AiMessage, AiTool, AiToolCall, AiUsage,
};
use crate::{
    context_window::{
        self,
        sources::{LearnedWindow, WindowLookup},
    },
    dtos::TestConnectionResponse,
};
use netmonitor_core::shared::errors::{AppError, AppResult};

pub struct OpenAiCompatibleDriver {
    pub driver_id: &'static str,
    pub display_name: &'static str,
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: String,
    pub extra_headers: Vec<(String, String)>,
    client: reqwest::Client,
    /// Onde o provedor informa a janela de contexto dos modelos.
    window_lookup: Option<Arc<dyn WindowLookup>>,
    num_ctx: Option<u64>,
    /// `reasoning_effort` do protocolo OpenAI (`none` desliga o raciocínio).
    reasoning_effort: Option<&'static str>,
    /// Piso do `max_tokens` pedido (o raciocínio conta no teto).
    min_output_tokens: Option<u32>,
}

/// Até conectar: provedor fora do ar responde logo.
const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

/// Sem nenhum byte por tanto tempo, o provedor travou. Não há teto para a
/// resposta inteira: modelo local em CPU leva minutos para ler um prompt grande
/// e para escrever, e quem decide parar é a pessoa (fechar a tela ou "Parar"
/// interrompe na hora).
const SILENCE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(600);

/// Pool HTTP compartilhado entre chamadas de chat para reaproveitar conexões
/// TLS/keep-alive com o provedor.
fn shared_client() -> reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .connect_timeout(CONNECT_TIMEOUT)
                .read_timeout(SILENCE_TIMEOUT)
                .build()
                .unwrap_or_default()
        })
        .clone()
}

/// Erro de prazo explicado para quem usa, com o que fazer.
fn silence_message(provider: &str) -> String {
    format!(
        "{provider} ficou {} minutos sem responder. Em CPU, modelo grande ou janela de contexto grande demoram a ler a pergunta: escolha um modelo menor ou reduza a janela nas Configurações de IA.",
        SILENCE_TIMEOUT.as_secs() / 60
    )
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
        let client = shared_client();

        let trimmed_url = base_url.trim_end_matches('/').to_string();

        Self {
            driver_id,
            display_name,
            base_url: trimmed_url,
            api_key,
            model,
            extra_headers,
            client,
            window_lookup: None,
            num_ctx: None,
            reasoning_effort: None,
            min_output_tokens: None,
        }
    }

    /// Liga a consulta da janela de contexto ao provedor.
    #[must_use]
    pub fn with_window_lookup(mut self, lookup: Arc<dyn WindowLookup>) -> Self {
        self.window_lookup = Some(lookup);
        self
    }

    /// Configura a janela de contexto / num_ctx enviada ao servidor (ex: Ollama options.num_ctx).
    #[must_use]
    pub fn with_num_ctx(mut self, num_ctx: u64) -> Self {
        self.num_ctx = Some(num_ctx);
        self
    }

    /// Quanto o modelo raciocina antes de responder; `None` não envia nada.
    #[must_use]
    pub fn with_reasoning_effort(mut self, effort: Option<&'static str>) -> Self {
        self.reasoning_effort = effort;
        self
    }

    /// Nenhum `max_tokens` abaixo deste valor; `None` mantém o pedido.
    #[must_use]
    pub fn with_min_output_tokens(mut self, floor: Option<u32>) -> Self {
        self.min_output_tokens = floor;
        self
    }

    fn endpoint_url(&self) -> String {
        if self.base_url.ends_with("/chat/completions") {
            self.base_url.clone()
        } else {
            format!("{}/chat/completions", self.base_url)
        }
    }

    /// O pedido não chegou a ter resposta: prazo esgotado ou provedor fora.
    fn send_error(&self, err: &reqwest::Error) -> AppError {
        AppError::service_unavailable(if err.is_timeout() {
            silence_message(&format!("O provedor {}", self.display_name))
        } else {
            format!("Falha ao conectar ao provedor {}: {err}", self.display_name)
        })
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

/// Lê o stream SSE do provedor numa task e devolve os pedaços já interpretados.
/// Modelos que só guardam o prefixo em cache com marcação explícita
/// (`cache_control`). OpenAI, DeepSeek e afins fazem sozinhos.
fn needs_cache_breakpoints(driver_id: &str, model: &str) -> bool {
    let model = model.to_ascii_lowercase();
    driver_id == "openrouter"
        && (model.starts_with("anthropic/") || model.contains("claude") || model.contains("gemini"))
}

/// Marca o system prompt e a última pergunta como pontos de cache.
///
/// O system prompt (com os contratos das ferramentas, que vêm antes dele)
/// é o mesmo a conversa inteira; a última pergunta fecha o prefixo que a
/// próxima rodada — e a próxima pergunta — reaproveita.
fn with_cache_breakpoints(messages: &mut serde_json::Value) {
    let Some(list) = messages.as_array_mut() else {
        return;
    };
    let last_user = list.iter().rposition(|message| message["role"] == "user");
    for index in [Some(0), last_user].into_iter().flatten() {
        let message = &mut list[index];
        if index == 0 && message["role"] != "system" {
            continue;
        }
        if let Some(text) = message["content"].as_str().map(ToString::to_string) {
            message["content"] = json!([{
                "type": "text",
                "text": text,
                "cache_control": { "type": "ephemeral" }
            }]);
        }
    }
}

/// Buffer de bytes O(N) para decodificar linhas SSE sem corromper caracteres
/// UTF-8 divididos entre pacotes de rede nem copiar o restante a cada linha.
#[derive(Default)]
struct SseParser {
    buffer: Vec<u8>,
    offset: usize,
    accumulated_tools: BTreeMap<usize, (String, String, String)>,
    // O id do modelo se repete em todo pedaço; basta repassar uma vez.
    model_reported: bool,
    done: bool,
}

impl SseParser {
    fn push(&mut self, chunk: &[u8]) -> Vec<AiChatChunk> {
        let mut out = Vec::new();
        if self.done {
            return out;
        }
        self.buffer.extend_from_slice(chunk);

        while !self.done {
            let Some(rel) = self.buffer[self.offset..].iter().position(|&b| b == b'\n') else {
                break;
            };
            let end = self.offset + rel;
            let raw = self.buffer[self.offset..end]
                .strip_suffix(b"\r")
                .unwrap_or(&self.buffer[self.offset..end]);
            let line = String::from_utf8_lossy(raw);
            Self::parse_line(
                &mut self.accumulated_tools,
                &mut self.model_reported,
                &mut self.done,
                line.trim(),
                &mut out,
            );
            self.offset = end + 1;
        }

        if self.offset == self.buffer.len() {
            self.buffer.clear();
            self.offset = 0;
        } else if self.offset >= 4096 && self.offset * 2 >= self.buffer.len() {
            self.buffer.drain(..self.offset);
            self.offset = 0;
        }

        out
    }

    fn finish(&mut self) -> Vec<AiChatChunk> {
        let mut out = Vec::new();
        if !self.done && self.offset < self.buffer.len() {
            let raw = self.buffer[self.offset..]
                .strip_suffix(b"\r")
                .unwrap_or(&self.buffer[self.offset..]);
            let line = String::from_utf8_lossy(raw);
            Self::parse_line(
                &mut self.accumulated_tools,
                &mut self.model_reported,
                &mut self.done,
                line.trim(),
                &mut out,
            );
        }
        self.buffer.clear();
        self.offset = 0;

        if !self.done && !self.accumulated_tools.is_empty() {
            let calls = drain_calls(&mut self.accumulated_tools);
            out.push(AiChatChunk {
                text_delta: None,
                tool_calls: calls,
                finish_reason: Some("tool_calls".into()),
                ..AiChatChunk::default()
            });
        }
        out
    }

    fn parse_line(
        accumulated_tools: &mut BTreeMap<usize, (String, String, String)>,
        model_reported: &mut bool,
        done: &mut bool,
        line: &str,
        out: &mut Vec<AiChatChunk>,
    ) {
        if line.is_empty() || line.starts_with(':') {
            return;
        }
        let Some(data_str) = line.strip_prefix("data:").map(str::trim) else {
            return;
        };
        if data_str == "[DONE]" {
            *done = true;
            if !accumulated_tools.is_empty() {
                let calls = drain_calls(accumulated_tools);
                out.push(AiChatChunk {
                    text_delta: None,
                    tool_calls: calls,
                    finish_reason: Some("tool_calls".into()),
                    ..AiChatChunk::default()
                });
            }
            out.push(AiChatChunk {
                text_delta: None,
                tool_calls: Vec::new(),
                finish_reason: Some("stop".into()),
                ..AiChatChunk::default()
            });
            return;
        }

        let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data_str) else {
            return;
        };

        if !*model_reported {
            if let Some(model) = parsed
                .get("model")
                .and_then(|m| m.as_str())
                .filter(|m| !m.is_empty())
            {
                *model_reported = true;
                out.push(AiChatChunk {
                    model: Some(model.to_string()),
                    ..AiChatChunk::default()
                });
            }
        }
        if let Some(usage) = parsed.get("usage").and_then(AiUsage::from_openai) {
            out.push(AiChatChunk {
                usage: Some(usage),
                ..AiChatChunk::default()
            });
        }
        let Some(choice) = parsed
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|choices| choices.first())
        else {
            return;
        };

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
                let index = tc.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
                let id = tc.get("id").and_then(|s| s.as_str()).unwrap_or("");
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
                    .or_insert_with(|| (String::new(), String::new(), String::new()));
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
        if finish_reason.as_deref() == Some("tool_calls") && !accumulated_tools.is_empty() {
            calls.extend(drain_calls(accumulated_tools));
        }

        if text_delta.is_some() || !calls.is_empty() || finish_reason.is_some() {
            out.push(AiChatChunk {
                text_delta,
                tool_calls: calls,
                finish_reason,
                ..AiChatChunk::default()
            });
        }
    }
}

fn spawn_sse_reader(res: reqwest::Response, notice: Option<String>) -> AiChunkStream {
    let (sender, receiver) = mpsc::channel(64);

    tokio::spawn(async move {
        if let Some(notice_text) = notice {
            if sender
                .send(Ok(AiChatChunk {
                    notice: Some(notice_text),
                    ..AiChatChunk::default()
                }))
                .await
                .is_err()
            {
                return;
            }
        }

        let byte_stream = res
            .bytes_stream()
            .map(|item| item.map_err(std::io::Error::other));
        let reader = tokio_util::io::StreamReader::new(byte_stream);
        let mut framed = FramedRead::new(reader, BytesCodec::new());
        let mut parser = SseParser::default();

        while let Some(chunk_res) = framed.next().await {
            let bytes = match chunk_res {
                Ok(b) => b,
                Err(e) => {
                    let err_str = e.to_string();
                    let friendly = if err_str.contains("timed out") {
                        silence_message("O provedor de IA")
                    } else if err_str.contains("error decoding response body")
                        || err_str.contains("connection closed")
                        || err_str.contains("broken pipe")
                    {
                        "A conexão com o provedor de IA foi encerrada inesperadamente (o modelo local pode ter excedido a capacidade de memória ou limite de contexto). Recomendação: aumente o num_ctx nas Configurações de IA ou compacte a conversa.".to_string()
                    } else {
                        format!("Erro no stream de dados: {e}")
                    };
                    let _ = sender
                        .send(Err(AppError::service_unavailable(friendly)))
                        .await;
                    return;
                }
            };

            for item in parser.push(&bytes) {
                if sender.send(Ok(item)).await.is_err() {
                    return;
                }
            }
            if parser.done {
                return;
            }
        }

        for item in parser.finish() {
            if sender.send(Ok(item)).await.is_err() {
                return;
            }
        }
    });

    Box::pin(ReceiverStream::new(receiver))
}

#[async_trait]
impl AiDriver for OpenAiCompatibleDriver {
    fn id(&self) -> &'static str {
        self.driver_id
    }

    fn display_name(&self) -> &'static str {
        self.display_name
    }

    fn model(&self) -> Option<&str> {
        Some(&self.model)
    }

    async fn context_window(&self, model: &str) -> Option<LearnedWindow> {
        context_window::lookup(self.window_lookup.as_deref()?, model).await
    }

    async fn chat_stream(
        &self,
        messages: &[AiMessage],
        tools: &[AiTool],
        options: AiChatOptions,
    ) -> AppResult<AiChunkStream> {
        let url = self.endpoint_url();
        let headers = self.build_headers()?;

        let mut body = json!({
            "model": self.model,
            "messages": messages,
            "stream": true,
            "stream_options": { "include_usage": true },
        });

        if !tools.is_empty() {
            body["tools"] = json!(tools);
        }
        if let Some(num_ctx) = self.num_ctx {
            body["num_ctx"] = json!(num_ctx);
            body["options"] = json!({ "num_ctx": num_ctx });
        }
        if needs_cache_breakpoints(self.driver_id, &self.model) {
            with_cache_breakpoints(&mut body["messages"]);
        }
        if let Some(max_tokens) = options.max_tokens {
            body["max_tokens"] = json!(max_tokens.max(self.min_output_tokens.unwrap_or(0)));
        }
        if let Some(effort) = self.reasoning_effort {
            body["reasoning_effort"] = json!(effort);
        }

        let mut res = self
            .client
            .post(&url)
            .headers(headers.clone())
            .json(&body)
            .send()
            .await
            .map_err(|err| self.send_error(&err))?;

        let mut status = res.status();
        let mut fallback_notice = None;

        // Se o OpenRouter retornar 429 (rate-limited upstream) ou 404 para um modelo gratuito específico,
        // tenta o fallback automático para "openrouter/free" para não deixar o usuário sem resposta.
        if self.driver_id == "openrouter"
            && (status == reqwest::StatusCode::TOO_MANY_REQUESTS
                || status == reqwest::StatusCode::NOT_FOUND)
            && self.model != "openrouter/free"
        {
            let original_status = status;
            tracing::warn!(
                model = %self.model,
                status = %original_status,
                "Modelo OpenRouter retornou erro; tentando fallback automático para 'openrouter/free'"
            );
            body["model"] = json!("openrouter/free");
            if let Ok(fallback_res) = self
                .client
                .post(&url)
                .headers(headers.clone())
                .json(&body)
                .send()
                .await
            {
                if fallback_res.status().is_success() {
                    res = fallback_res;
                    status = res.status();
                    let motivo = if original_status == reqwest::StatusCode::NOT_FOUND {
                        "não estava disponível (404)"
                    } else {
                        "atingiu o limite de taxa (429)"
                    };
                    fallback_notice = Some(format!(
                        "O modelo '{}' {motivo} no OpenRouter. A resposta foi gerada usando o balanceador dinâmico 'openrouter/free'.",
                        self.model
                    ));
                }
            }
        }

        if !status.is_success() {
            let mut err_text = res
                .text()
                .await
                .unwrap_or_else(|_| "Erro desconhecido".into());

            // Servidor compatível mais antigo que não conhece `stream_options`:
            // perde-se só a contagem de tokens, não a resposta.
            if status == reqwest::StatusCode::BAD_REQUEST && err_text.contains("stream_options") {
                if let Some(fields) = body.as_object_mut() {
                    fields.remove("stream_options");
                }
                let retry = self
                    .client
                    .post(&url)
                    .headers(headers)
                    .json(&body)
                    .send()
                    .await
                    .map_err(|err| self.send_error(&err))?;
                status = retry.status();
                if status.is_success() {
                    return Ok(spawn_sse_reader(retry, None));
                }
                err_text = retry
                    .text()
                    .await
                    .unwrap_or_else(|_| "Erro desconhecido".into());
            }

            let friendly =
                clean_provider_error_message(status, &err_text, &self.model, self.display_name);
            return Err(AppError::service_unavailable(friendly));
        }

        Ok(spawn_sse_reader(res, fallback_notice))
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

/// Esvazia as chamadas acumuladas na ordem do `index` do provedor — a ordem
/// em que o modelo as pediu, e a que a tela mostra. Id vazio ganha um único.
fn drain_calls(accumulated: &mut BTreeMap<usize, (String, String, String)>) -> Vec<AiToolCall> {
    std::mem::take(accumulated)
        .into_values()
        .map(|(id, name, arguments)| AiToolCall {
            id: if id.is_empty() {
                format!("call_{}", &uuid::Uuid::new_v4().simple().to_string()[..12])
            } else {
                id
            },
            name,
            arguments,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_explicito_so_para_quem_precisa_de_marcacao() {
        assert!(needs_cache_breakpoints(
            "openrouter",
            "anthropic/claude-sonnet-5"
        ));
        assert!(needs_cache_breakpoints(
            "openrouter",
            "google/gemini-2.5-pro"
        ));
        assert!(!needs_cache_breakpoints("openrouter", "openai/gpt-5"));
        assert!(!needs_cache_breakpoints("ollama", "claude-imitacao"));
    }

    #[test]
    fn marca_o_system_prompt_e_a_ultima_pergunta() {
        let mut mensagens = json!([
            { "role": "system", "content": "papel" },
            { "role": "user", "content": "primeira" },
            { "role": "assistant", "content": "resposta" },
            { "role": "user", "content": "segunda" },
            { "role": "tool", "content": "{}", "tool_call_id": "1" }
        ]);
        with_cache_breakpoints(&mut mensagens);
        assert_eq!(mensagens[0]["content"][0]["text"], "papel");
        assert_eq!(
            mensagens[0]["content"][0]["cache_control"]["type"],
            "ephemeral"
        );
        assert_eq!(mensagens[1]["content"], "primeira");
        assert_eq!(mensagens[3]["content"][0]["text"], "segunda");
        assert_eq!(
            mensagens[4]["content"], "{}",
            "resultado de ferramenta fica como texto"
        );
    }

    #[test]
    fn sse_preserva_utf8_partido_aceita_data_sem_espaco_e_descarrega_no_eof() {
        let mut parser = SseParser::default();
        // Caractere 'é' (0xC3 0xA9) partido no meio de dois chunks TCP,
        // linha `data:{...}` sem espaço após `:` e ferramenta sem `[DONE]` no fim.
        let line1 = "data:{\"choices\":[{\"delta\":{\"content\":\"olá café\"}}]}\n".as_bytes();
        let split_at = line1.iter().position(|&b| b == 0xC3).unwrap() + 1;

        let mut chunks = parser.push(&line1[..split_at]);
        assert!(chunks.is_empty(), "linha incompleta aguarda o restante");
        chunks.extend(parser.push(&line1[split_at..]));
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text_delta.as_deref(), Some("olá café"));

        let tool_line = b"data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"c1\",\"function\":{\"name\":\"ping\",\"arguments\":\"{}\"}}]}}]}";
        assert!(parser.push(tool_line).is_empty());
        let flushed = parser.finish();
        assert_eq!(flushed.len(), 1);
        assert_eq!(flushed[0].tool_calls.len(), 1);
        assert_eq!(flushed[0].tool_calls[0].name, "ping");
    }
}
