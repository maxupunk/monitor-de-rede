//! O sandbox Rhai que executa uma ação de plugin.
//!
//! # O que o script enxerga
//!
//! Só o que é registrado aqui: o objeto `device` (passado como primeiro
//! argumento de cada ação) e um punhado de utilitários de texto. Não há
//! `import` (feature `no_module`), nem `eval`, nem acesso a arquivo, processo
//! ou rede. A única saída do sandbox é `device.ssh/run/telnet/http`, e cada
//! uma passa por três portas, nesta ordem:
//!
//! 1. o transporte precisa estar declarado no manifesto;
//! 2. o efeito da chamada ([`effect::classify_command`]/[`effect::classify_http`])
//!    não pode exceder o da ação — ação de leitura que tenta escrever é
//!    **recusada**, não perguntada;
//! 3. o [`AccessGate`] da execução aprova (ou pergunta ao operador).
//!
//! # Segredos
//!
//! O script nunca recebe senha. Ele escreve `{{username}}`/`{{password}}` onde
//! precisar (corpo de login, `sudo -S`), e a troca acontece aqui, no último
//! instante antes do transporte. O transcript guarda o texto com o marcador,
//! e toda saída passa pela máscara antes de ser registrada.
//!
//! # Limites
//!
//! Orçamento de operações, profundidade, tamanho de string/lista/mapa e um
//! prazo de relógio. O tempo que uma chamada passa esperando aprovação é
//! devolvido ao prazo: o operador pensando não pode derrubar a execução.

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use regex::Regex;
use rhai::{Dynamic, Engine, EvalAltResult, Map, Scope, AST};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use super::{
    effect::{self, Effect},
    gate::{AccessGate, AccessRequest},
    manifest::TransportKind,
    transport::{DeviceIoCall, DeviceIoReply, DeviceTransport, Endpoint, HttpRequest, Login},
};
use crate::services::shared::text::truncate_chars;

/// Quanto de cada saída entra no transcript. O script recebe a saída inteira.
const TRANSCRIPT_OUTPUT_CHARS: usize = 8_000;
const DEFAULT_CALL_TIMEOUT_MS: u64 = 30_000;
const PASSWORD_PLACEHOLDER: &str = "{{password}}";
const USERNAME_PLACEHOLDER: &str = "{{username}}";

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub max_operations: u64,
    pub deadline: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_operations: 5_000_000,
            deadline: Duration::from_secs(5 * 60),
        }
    }
}

/// O que o script sabe do equipamento (`device.info`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub id: i64,
    pub name: String,
    pub ip: String,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub platform: String,
    pub firmware: Option<String>,
}

/// Como alcançar o equipamento por um transporte.
#[derive(Debug, Clone)]
pub struct AccessProfile {
    pub login: Login,
    pub port: u16,
    pub https: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Credentials {
    pub ssh: Option<AccessProfile>,
    pub http: Option<AccessProfile>,
    pub telnet: Option<AccessProfile>,
}

impl Credentials {
    fn profile(&self, transport: TransportKind) -> Option<&AccessProfile> {
        match transport {
            TransportKind::Ssh => self.ssh.as_ref(),
            TransportKind::Http => self.http.as_ref(),
            TransportKind::Telnet => self.telnet.as_ref(),
        }
    }

    fn secrets(&self) -> Vec<String> {
        [&self.ssh, &self.http, &self.telnet]
            .iter()
            .filter_map(|profile| profile.as_ref())
            .filter_map(|profile| profile.login.password.clone())
            .filter(|password| !password.is_empty())
            .collect()
    }
}

/// Uma linha do registro da execução.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct TranscriptEntry {
    pub seq: u32,
    /// `ssh`, `http`, `telnet`, `log`, `denied` ou `error`.
    pub kind: String,
    pub request: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub effect: Option<Effect>,
    /// Código de saída (SSH) ou status HTTP.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "number")]
    pub status: Option<i64>,
    pub output: String,
    #[ts(type = "number")]
    pub duration_ms: u64,
    pub origin: String,
    pub at: String,
}

pub type Observer = Arc<dyn Fn(&TranscriptEntry) + Send + Sync>;

pub struct ExecutionContext {
    pub device: DeviceInfo,
    pub transports: Vec<TransportKind>,
    pub action_effect: Effect,
    /// Motivo registrado em cada pedido de aprovação.
    pub reason: Option<String>,
    pub credentials: Credentials,
    pub transport: Arc<dyn DeviceTransport>,
    pub gate: Arc<dyn AccessGate>,
    pub cancel: CancellationToken,
    pub observer: Option<Observer>,
    pub limits: Limits,
}

#[derive(Debug, Clone)]
pub struct ExecutionOutcome {
    pub output: Result<Value, String>,
    pub transcript: Vec<TranscriptEntry>,
}

/// As funções do script com a aridade — a validação do pacote confere que
/// cada ação tem a sua `fn <id>(device, params)`.
///
/// # Errors
///
/// Erro de sintaxe, com linha e coluna.
pub fn entry_points(script: &str) -> Result<Vec<(String, usize)>, String> {
    let ast = sandboxed_engine(Limits::default(), None)
        .compile(script)
        .map_err(|error| error.to_string())?;
    Ok(ast
        .iter_functions()
        .map(|function| (function.name.to_string(), function.params.len()))
        .collect())
}

/// Executa `action` do `script` com os `params` já validados.
pub async fn execute(
    script: &str,
    action: &str,
    params: Value,
    context: ExecutionContext,
) -> ExecutionOutcome {
    let limits = context.limits;
    let inner = Arc::new(Inner::new(context));
    let handle = DeviceHandle(inner.clone());
    let script = script.to_owned();
    let action = action.to_owned();

    let worker = {
        let inner = inner.clone();
        tokio::task::spawn_blocking(move || {
            let engine = sandboxed_engine(limits, Some(inner.clone()));
            let ast: AST = engine.compile(&script).map_err(|error| error.to_string())?;
            let params = rhai::serde::to_dynamic(&params).map_err(|error| error.to_string())?;
            let mut scope = Scope::new();
            let result: Dynamic = engine
                .call_fn(&mut scope, &ast, &action, (handle, params))
                .map_err(|error| describe_error(&error))?;
            rhai::serde::from_dynamic::<Value>(&result).map_err(|error| error.to_string())
        })
    };
    let output = match worker.await {
        Ok(result) => result,
        Err(error) => Err(format!("a execução do script falhou: {error}")),
    };
    let output = output.map_err(|message| inner.mask(&message));
    let transcript = inner.take_transcript();
    ExecutionOutcome { output, transcript }
}

fn describe_error(error: &EvalAltResult) -> String {
    match error {
        EvalAltResult::ErrorTerminated(reason, _) => reason
            .clone()
            .try_cast::<String>()
            .unwrap_or_else(|| "execução interrompida".into()),
        EvalAltResult::ErrorTooManyOperations(_) => {
            "o script passou do limite de operações (laço infinito?)".into()
        }
        // `throw` do próprio plugin é mensagem para o operador (muitas vezes a
        // resposta do equipamento): vai como está, sem posição no script.
        EvalAltResult::ErrorRuntime(value, _) => value
            .clone()
            .try_cast::<String>()
            .unwrap_or_else(|| value.to_string()),
        EvalAltResult::ErrorInFunctionCall(_, _, inner, _)
            if matches!(**inner, EvalAltResult::ErrorRuntime(..)) =>
        {
            describe_error(inner)
        }
        EvalAltResult::ErrorInFunctionCall(name, _, inner, position) => {
            format!("em `{name}` ({position}): {}", describe_error(inner))
        }
        other => other.to_string(),
    }
}

fn sandboxed_engine(limits: Limits, inner: Option<Arc<Inner>>) -> Engine {
    let mut engine = Engine::new();
    engine.set_max_operations(limits.max_operations);
    engine.set_max_call_levels(48);
    engine.set_max_expr_depths(64, 32);
    engine.set_max_string_size(4 * 1024 * 1024);
    engine.set_max_array_size(100_000);
    engine.set_max_map_size(20_000);
    engine.disable_symbol("eval");
    register_utils(&mut engine);

    if let Some(inner) = inner {
        let progress = inner.clone();
        engine.on_progress(move |_| progress.should_stop().map(Dynamic::from));
        let printer = inner.clone();
        engine.on_print(move |text| printer.log(text));
        let debugger = inner;
        engine.on_debug(move |text, _, _| debugger.log(text));
    } else {
        engine.on_print(|_| {});
        engine.on_debug(|_, _, _| {});
    }

    engine
        .register_type_with_name::<DeviceHandle>("Device")
        .register_get("info", DeviceHandle::info)
        .register_fn("ssh", DeviceHandle::ssh)
        .register_fn("run", DeviceHandle::run)
        .register_fn("telnet", DeviceHandle::telnet)
        .register_fn("http", DeviceHandle::http)
        .register_fn("get", DeviceHandle::get)
        .register_fn("post_form", DeviceHandle::post_form)
        .register_fn("post_json", DeviceHandle::post_json)
        .register_fn("log", DeviceHandle::log);
    engine
}

type RhaiResult<T> = Result<T, Box<EvalAltResult>>;

fn fail<T>(message: impl Into<String>) -> RhaiResult<T> {
    Err(message.into().into())
}

fn compile_regex(pattern: &str) -> RhaiResult<Regex> {
    if pattern.len() > 2_000 {
        return fail("regex longa demais");
    }
    Regex::new(pattern).or_else(|error| fail(format!("regex inválida: {error}")))
}

fn register_utils(engine: &mut Engine) {
    engine.register_fn(
        "regex_match",
        |text: &str, pattern: &str| -> RhaiResult<bool> {
            Ok(compile_regex(pattern)?.is_match(text))
        },
    );
    engine.register_fn(
        "regex_capture",
        |text: &str, pattern: &str| -> RhaiResult<Dynamic> {
            let re = compile_regex(pattern)?;
            Ok(re
                .captures(text)
                .and_then(|captures| captures.get(1).or_else(|| captures.get(0)))
                .map_or(Dynamic::UNIT, |m| Dynamic::from(m.as_str().to_owned())))
        },
    );
    engine.register_fn(
        "regex_captures",
        |text: &str, pattern: &str| -> RhaiResult<rhai::Array> {
            let re = compile_regex(pattern)?;
            Ok(re
                .captures_iter(text)
                .filter_map(|captures| captures.get(1).or_else(|| captures.get(0)))
                .map(|m| Dynamic::from(m.as_str().to_owned()))
                .collect())
        },
    );
    // O `trim()` nativo do Rhai altera a string no lugar e devolve `()` —
    // `x.trim() == ""` compara `()` com texto. `trimmed` devolve o valor.
    engine.register_fn("trimmed", |text: &str| text.trim().to_owned());
    engine.register_fn("words", |text: &str| -> rhai::Array {
        text.split_whitespace()
            .map(|word| Dynamic::from(word.to_owned()))
            .collect()
    });
    engine.register_fn("lines", |text: &str| -> rhai::Array {
        text.lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(|line| Dynamic::from(line.to_owned()))
            .collect()
    });
    engine.register_fn("parse_kv", |text: &str, separator: &str| -> Map {
        parse_kv(text, separator)
            .into_iter()
            .map(|(key, value)| (key.into(), Dynamic::from(value)))
            .collect()
    });
    engine.register_fn("json_parse", |text: &str| -> RhaiResult<Dynamic> {
        let value: Value =
            serde_json::from_str(text).or_else(|error| fail(format!("JSON inválido: {error}")))?;
        rhai::serde::to_dynamic(&value)
    });
    engine.register_fn("json_encode", |value: Dynamic| -> RhaiResult<String> {
        let value: Value = rhai::serde::from_dynamic(&value)?;
        Ok(value.to_string())
    });
    engine.register_fn("shell_quote", |text: &str| shell_quote(text));
    engine.register_fn("url_encode", |text: &str| url_encode(text));
}

/// `CHAVE='valor'`/`chave: valor` por linha → mapa. Aspas simples e duplas
/// em volta do valor saem.
#[must_use]
pub fn parse_kv(text: &str, separator: &str) -> BTreeMap<String, String> {
    let separator = if separator.is_empty() { "=" } else { separator };
    text.lines()
        .filter_map(|line| {
            let (key, value) = line.split_once(separator)?;
            let key = key.trim();
            if key.is_empty() {
                return None;
            }
            let value = value.trim();
            let value = value
                .strip_prefix('\'')
                .and_then(|v| v.strip_suffix('\''))
                .or_else(|| value.strip_prefix('"').and_then(|v| v.strip_suffix('"')))
                .unwrap_or(value);
            Some((key.to_owned(), value.to_owned()))
        })
        .collect()
}

/// Aspas simples de shell POSIX: o valor vira um único argumento literal.
#[must_use]
pub fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

#[must_use]
pub fn url_encode(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

struct Inner {
    context: ExecutionContext,
    runtime: tokio::runtime::Handle,
    secrets: Vec<String>,
    transcript: Mutex<Vec<TranscriptEntry>>,
    cookies: Mutex<BTreeMap<String, String>>,
    deadline: Mutex<Instant>,
}

impl Inner {
    fn new(context: ExecutionContext) -> Self {
        let secrets = context.credentials.secrets();
        let deadline = Instant::now() + context.limits.deadline;
        Self {
            context,
            runtime: tokio::runtime::Handle::current(),
            secrets,
            transcript: Mutex::default(),
            cookies: Mutex::default(),
            deadline: Mutex::new(deadline),
        }
    }

    fn mask(&self, text: &str) -> String {
        self.secrets.iter().fold(text.to_owned(), |text, secret| {
            text.replace(secret, "********")
        })
    }

    fn should_stop(&self) -> Option<String> {
        if self.context.cancel.is_cancelled() {
            return Some("execução cancelada pelo operador".into());
        }
        let deadline = *self
            .deadline
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (Instant::now() > deadline).then(|| "a execução passou do tempo limite".into())
    }

    fn extend_deadline(&self, by: Duration) {
        let mut deadline = self
            .deadline
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *deadline += by;
    }

    fn record(&self, mut entry: TranscriptEntry) {
        entry.request = self.mask(&entry.request);
        entry.output = truncate_chars(&self.mask(&entry.output), TRANSCRIPT_OUTPUT_CHARS);
        let mut transcript = self
            .transcript
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        entry.seq = u32::try_from(transcript.len() + 1).unwrap_or(u32::MAX);
        if let Some(observer) = &self.context.observer {
            observer(&entry);
        }
        transcript.push(entry);
    }

    fn log(&self, text: &str) {
        self.record(TranscriptEntry {
            seq: 0,
            kind: "log".into(),
            request: String::new(),
            effect: None,
            status: None,
            output: text.to_owned(),
            duration_ms: 0,
            origin: "script".into(),
            at: chrono::Utc::now().to_rfc3339(),
        });
    }

    fn take_transcript(&self) -> Vec<TranscriptEntry> {
        std::mem::take(
            &mut self
                .transcript
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }

    fn denied(&self, transport: TransportKind, summary: &str, effect: Effect, why: &str) {
        self.record(TranscriptEntry {
            seq: 0,
            kind: "denied".into(),
            request: format!("{}: {summary}", transport.as_str()),
            effect: Some(effect),
            status: None,
            output: why.to_owned(),
            duration_ms: 0,
            origin: self.context.transport.origin(),
            at: chrono::Utc::now().to_rfc3339(),
        });
    }

    fn substitute(&self, text: &str, login: &Login) -> String {
        text.replace(USERNAME_PLACEHOLDER, &login.username).replace(
            PASSWORD_PLACEHOLDER,
            login.password.as_deref().unwrap_or_default(),
        )
    }

    /// As três portas da nota do módulo, e depois o transporte.
    fn call(
        &self,
        transport: TransportKind,
        summary: &str,
        effect: Effect,
        build: impl FnOnce(&AccessProfile) -> DeviceIoCall,
    ) -> RhaiResult<DeviceIoReply> {
        if let Some(reason) = self.should_stop() {
            return fail(reason);
        }
        if !self.context.transports.contains(&transport) {
            let why = format!(
                "o plugin não declara o transporte `{}` no manifesto",
                transport.as_str()
            );
            self.denied(transport, summary, effect, &why);
            return fail(why);
        }
        if effect > self.context.action_effect {
            let why = "a ação é de leitura e esta chamada altera o equipamento; declare a ação \
                       como `write` se a alteração é intencional"
                .to_string();
            self.denied(transport, summary, effect, &why);
            return fail(why);
        }
        let Some(profile) = self.context.credentials.profile(transport) else {
            let why = format!(
                "não há credencial {} cadastrada para este equipamento",
                transport.as_str()
            );
            self.denied(transport, summary, effect, &why);
            return fail(why);
        };

        let request = AccessRequest {
            transport,
            summary: self.mask(summary),
            effect,
            reason: self.context.reason.clone(),
        };
        let waiting = Instant::now();
        let approval = self.runtime.block_on(self.context.gate.authorize(&request));
        self.extend_deadline(waiting.elapsed());
        if let Err(error) = approval {
            self.denied(transport, summary, effect, &error.to_string());
            return fail(error.to_string());
        }

        let call = build(profile);
        let started = Instant::now();
        let reply = self.runtime.block_on(self.context.transport.execute(&call));
        let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let at = chrono::Utc::now().to_rfc3339();
        let origin = self.context.transport.origin();
        match reply {
            Ok(reply) => {
                let (status, output) = match &reply {
                    DeviceIoReply::Exec {
                        stdout,
                        stderr,
                        exit_code,
                    } => (
                        exit_code.map(i64::from),
                        if stderr.is_empty() {
                            stdout.clone()
                        } else {
                            format!("{stdout}\n[stderr]\n{stderr}")
                        },
                    ),
                    DeviceIoReply::Http { status, body, .. } => {
                        (Some(i64::from(*status)), body.clone())
                    }
                };
                self.record(TranscriptEntry {
                    seq: 0,
                    kind: transport.as_str().into(),
                    request: summary.to_owned(),
                    effect: Some(effect),
                    status,
                    output,
                    duration_ms,
                    origin,
                    at,
                });
                Ok(reply)
            }
            Err(error) => {
                self.record(TranscriptEntry {
                    seq: 0,
                    kind: "error".into(),
                    request: format!("{}: {summary}", transport.as_str()),
                    effect: Some(effect),
                    status: None,
                    output: error.to_string(),
                    duration_ms,
                    origin,
                    at,
                });
                fail(self.mask(&error.to_string()))
            }
        }
    }

    fn endpoint(&self, profile: &AccessProfile) -> Endpoint {
        Endpoint {
            host: self.context.device.ip.clone(),
            port: profile.port,
        }
    }

    fn exec(
        &self,
        transport: TransportKind,
        command: &str,
    ) -> RhaiResult<(String, String, Option<i32>)> {
        let effect = effect::classify_command(command);
        let reply = self.call(transport, command, effect, |profile| {
            let endpoint = self.endpoint(profile);
            let command = self.substitute(command, &profile.login);
            let login = profile.login.clone();
            if transport == TransportKind::Telnet {
                DeviceIoCall::TelnetExec {
                    endpoint,
                    login,
                    command,
                    timeout_ms: DEFAULT_CALL_TIMEOUT_MS,
                }
            } else {
                DeviceIoCall::SshExec {
                    endpoint,
                    login,
                    command,
                    timeout_ms: DEFAULT_CALL_TIMEOUT_MS,
                }
            }
        })?;
        match reply {
            DeviceIoReply::Exec {
                stdout,
                stderr,
                exit_code,
            } => Ok((stdout, stderr, exit_code)),
            DeviceIoReply::Http { .. } => fail("resposta inesperada do transporte"),
        }
    }

    fn http(&self, spec: &HttpSpec) -> RhaiResult<Map> {
        if !spec.path.starts_with('/') || spec.path.starts_with("//") || spec.path.contains("://") {
            return fail("`path` precisa ser um caminho relativo (\"/cgi-bin/luci\"): o plugin só alcança o próprio equipamento");
        }
        let effect = if spec.login {
            Effect::Read
        } else {
            effect::classify_http(&spec.method)
        };
        let summary = format!(
            "{} {}{}",
            spec.method.to_ascii_uppercase(),
            spec.path,
            if spec.login { " (login)" } else { "" }
        );
        let cookie_header = {
            let cookies = self
                .cookies
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            (!cookies.is_empty()).then(|| {
                cookies
                    .iter()
                    .map(|(name, value)| format!("{name}={value}"))
                    .collect::<Vec<_>>()
                    .join("; ")
            })
        };
        let reply = self.call(TransportKind::Http, &summary, effect, |profile| {
            let mut headers: Vec<(String, String)> = spec
                .headers
                .iter()
                .map(|(name, value)| (name.clone(), self.substitute(value, &profile.login)))
                .collect();
            if let Some(cookie) = &cookie_header {
                if !headers
                    .iter()
                    .any(|(name, _)| name.eq_ignore_ascii_case("cookie"))
                {
                    headers.push(("Cookie".into(), cookie.clone()));
                }
            }
            DeviceIoCall::HttpRequest {
                endpoint: self.endpoint(profile),
                https: spec.https.unwrap_or(profile.https),
                request: HttpRequest {
                    method: spec.method.to_ascii_uppercase(),
                    path: spec.path.clone(),
                    headers,
                    body: spec
                        .body
                        .as_ref()
                        .map(|body| self.substitute(body, &profile.login)),
                    basic_auth: spec.basic_auth.then(|| profile.login.clone()),
                },
                timeout_ms: spec.timeout_ms.unwrap_or(DEFAULT_CALL_TIMEOUT_MS),
            }
        })?;
        let DeviceIoReply::Http {
            status,
            headers,
            body,
        } = reply
        else {
            return fail("resposta inesperada do transporte");
        };
        self.remember_cookies(&headers);
        let mut header_map = Map::new();
        for (name, value) in &headers {
            header_map.insert(name.as_str().into(), Dynamic::from(self.mask(value)));
        }
        let mut result = Map::new();
        result.insert("status".into(), Dynamic::from(i64::from(status)));
        result.insert("headers".into(), Dynamic::from_map(header_map));
        result.insert("body".into(), Dynamic::from(body));
        Ok(result)
    }

    fn remember_cookies(&self, headers: &[(String, String)]) {
        let mut cookies = self
            .cookies
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for (name, value) in headers {
            if !name.eq_ignore_ascii_case("set-cookie") {
                continue;
            }
            let pair = value.split(';').next().unwrap_or_default();
            if let Some((key, value)) = pair.split_once('=') {
                cookies.insert(key.trim().to_owned(), value.trim().to_owned());
            }
        }
    }
}

/// Um pedido HTTP vindo do script (`device.http(#{...})`).
#[derive(Debug, Clone, Default)]
struct HttpSpec {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Option<String>,
    https: Option<bool>,
    basic_auth: bool,
    login: bool,
    timeout_ms: Option<u64>,
}

impl HttpSpec {
    fn from_map(map: &Map) -> RhaiResult<Self> {
        let text = |key: &str| {
            map.get(key)
                .filter(|value| !value.is_unit())
                .map(|value| {
                    value
                        .clone()
                        .into_string()
                        .or_else(|_| fail(format!("`{key}` precisa ser texto")))
                })
                .transpose()
        };
        let flag = |key: &str| map.get(key).and_then(|value| value.as_bool().ok());
        let mut headers = Vec::new();
        if let Some(value) = map.get("headers") {
            let Some(values) = value.read_lock::<Map>() else {
                return fail("`headers` precisa ser um mapa");
            };
            for (name, value) in values.iter() {
                headers.push((name.to_string(), value.to_string()));
            }
        }
        let mut body = text("body")?;
        if let Some(form) = map.get("form") {
            let Some(form) = form.read_lock::<Map>() else {
                return fail("`form` precisa ser um mapa");
            };
            body = Some(form_encode(&form));
            headers.push((
                "Content-Type".into(),
                "application/x-www-form-urlencoded".into(),
            ));
        } else if let Some(json) = map.get("json") {
            let value: Value = rhai::serde::from_dynamic(json)?;
            body = Some(value.to_string());
            headers.push(("Content-Type".into(), "application/json".into()));
        }
        Ok(Self {
            method: text("method")?.unwrap_or_else(|| "GET".into()),
            path: text("path")?
                .ok_or_else(|| Box::<EvalAltResult>::from("`path` é obrigatório"))?,
            headers,
            body,
            https: flag("https"),
            basic_auth: flag("basic_auth").unwrap_or(false),
            login: flag("login").unwrap_or(false),
            timeout_ms: map
                .get("timeout_ms")
                .and_then(|value| value.as_int().ok())
                .and_then(|value| u64::try_from(value).ok()),
        })
    }
}

fn form_encode(form: &Map) -> String {
    form.iter()
        .map(|(key, value)| format!("{}={}", url_encode(key), url_encode(&value.to_string())))
        .collect::<Vec<_>>()
        .join("&")
}

/// O `device` que o script recebe.
#[derive(Clone)]
pub struct DeviceHandle(Arc<Inner>);

impl DeviceHandle {
    fn info(&mut self) -> Map {
        let device = &self.0.context.device;
        let mut map = Map::new();
        map.insert("id".into(), Dynamic::from(device.id));
        map.insert("name".into(), Dynamic::from(device.name.clone()));
        map.insert("ip".into(), Dynamic::from(device.ip.clone()));
        map.insert("platform".into(), Dynamic::from(device.platform.clone()));
        for (key, value) in [
            ("vendor", &device.vendor),
            ("model", &device.model),
            ("firmware", &device.firmware),
        ] {
            map.insert(
                key.into(),
                value.clone().map_or(Dynamic::UNIT, Dynamic::from),
            );
        }
        map
    }

    fn ssh(&mut self, command: &str) -> RhaiResult<Map> {
        let (stdout, stderr, exit_code) = self.0.exec(TransportKind::Ssh, command)?;
        let mut map = Map::new();
        map.insert("stdout".into(), Dynamic::from(stdout));
        map.insert("stderr".into(), Dynamic::from(stderr));
        map.insert(
            "exit".into(),
            exit_code.map_or(Dynamic::UNIT, |code| Dynamic::from(i64::from(code))),
        );
        Ok(map)
    }

    fn run(&mut self, command: &str) -> RhaiResult<String> {
        let (stdout, stderr, exit_code) = self.0.exec(TransportKind::Ssh, command)?;
        match exit_code {
            Some(0) | None => Ok(stdout),
            Some(code) => fail(format!(
                "`{}` saiu com código {code}: {}",
                self.0.mask(command),
                self.0.mask(stderr.trim())
            )),
        }
    }

    fn telnet(&mut self, command: &str) -> RhaiResult<String> {
        Ok(self.0.exec(TransportKind::Telnet, command)?.0)
    }

    fn http(&mut self, request: Map) -> RhaiResult<Map> {
        self.0.http(&HttpSpec::from_map(&request)?)
    }

    fn get(&mut self, path: &str) -> RhaiResult<Map> {
        self.0.http(&HttpSpec {
            method: "GET".into(),
            path: path.into(),
            ..HttpSpec::default()
        })
    }

    fn post_form(&mut self, path: &str, form: Map) -> RhaiResult<Map> {
        self.0.http(&HttpSpec {
            method: "POST".into(),
            path: path.into(),
            body: Some(form_encode(&form)),
            headers: vec![(
                "Content-Type".into(),
                "application/x-www-form-urlencoded".into(),
            )],
            ..HttpSpec::default()
        })
    }

    fn post_json(&mut self, path: &str, value: Dynamic) -> RhaiResult<Map> {
        let value: Value = rhai::serde::from_dynamic(&value)?;
        self.0.http(&HttpSpec {
            method: "POST".into(),
            path: path.into(),
            body: Some(value.to_string()),
            headers: vec![("Content-Type".into(), "application/json".into())],
            ..HttpSpec::default()
        })
    }

    fn log(&mut self, text: &str) {
        self.0.log(text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::plugins::{
        gate::AllowAll, package::Fixture, transport::fake::FakeTransport,
    };
    use serde_json::json;

    fn context(fixtures: Vec<Fixture>, effect: Effect) -> ExecutionContext {
        ExecutionContext {
            device: DeviceInfo {
                id: 1,
                name: "Roteador".into(),
                ip: "192.0.2.1".into(),
                platform: "openwrt".into(),
                ..DeviceInfo::default()
            },
            transports: vec![TransportKind::Ssh, TransportKind::Http],
            action_effect: effect,
            reason: None,
            credentials: Credentials {
                ssh: Some(AccessProfile {
                    login: Login {
                        username: "root".into(),
                        password: Some("s3nh4".into()),
                    },
                    port: 22,
                    https: false,
                }),
                http: Some(AccessProfile {
                    login: Login {
                        username: "admin".into(),
                        password: Some("s3nh4".into()),
                    },
                    port: 80,
                    https: false,
                }),
                telnet: None,
            },
            transport: Arc::new(FakeTransport::new(fixtures)),
            gate: Arc::new(AllowAll),
            cancel: CancellationToken::new(),
            observer: None,
            limits: Limits::default(),
        }
    }

    fn ssh(command: &str, stdout: &str) -> Fixture {
        Fixture {
            ssh: Some(command.into()),
            stdout: stdout.into(),
            ..Fixture::default()
        }
    }

    #[tokio::test]
    async fn acao_le_o_equipamento_e_devolve_json() {
        let script = r#"
            fn detect(device, params) {
                let release = parse_kv(device.run("cat /etc/openwrt_release"), "=");
                #{ firmware: release.DISTRIB_RELEASE, name: device.info.name }
            }
        "#;
        let outcome = execute(
            script,
            "detect",
            json!({}),
            context(
                vec![ssh(
                    "cat /etc/openwrt_release",
                    "DISTRIB_RELEASE='23.05.2'\n",
                )],
                Effect::Read,
            ),
        )
        .await;
        assert_eq!(
            outcome.output.unwrap(),
            json!({ "firmware": "23.05.2", "name": "Roteador" })
        );
        assert_eq!(outcome.transcript.len(), 1);
        assert_eq!(outcome.transcript[0].kind, "ssh");
    }

    #[tokio::test]
    async fn acao_de_leitura_nao_escreve() {
        let script = r#"fn detect(device, params) { device.run("reboot") }"#;
        let outcome = execute(
            script,
            "detect",
            json!({}),
            context(vec![ssh("reboot", "")], Effect::Read),
        )
        .await;
        assert!(outcome.output.unwrap_err().contains("leitura"));
        assert_eq!(outcome.transcript[0].kind, "denied");
    }

    #[tokio::test]
    async fn senha_entra_pelo_marcador_e_nao_aparece_no_registro() {
        let script = r#"
            fn login(device, params) {
                let r = device.http(#{ method: "POST", path: "/login", login: true,
                                       form: #{ user: "{{username}}", pass: "{{password}}" } });
                r.body
            }
        "#;
        let fixture = Fixture {
            http: Some("POST /login".into()),
            body: "ok s3nh4".into(),
            ..Fixture::default()
        };
        let outcome = execute(
            script,
            "login",
            json!({}),
            context(vec![fixture], Effect::Read),
        )
        .await;
        assert_eq!(outcome.output.unwrap(), json!("ok s3nh4"));
        assert!(outcome.transcript[0].output.contains("********"));
        assert!(!outcome.transcript[0].output.contains("s3nh4"));
    }

    #[tokio::test]
    async fn throw_do_plugin_chega_limpo_ao_operador() {
        let script = r#"
            fn falha(device) { throw "Unknown package 'x'"; }
            fn detect(device, params) { falha(device) }
        "#;
        let outcome = execute(script, "detect", json!({}), context(vec![], Effect::Read)).await;
        assert_eq!(outcome.output.unwrap_err(), "Unknown package 'x'");
    }

    #[tokio::test]
    async fn laco_infinito_e_interrompido() {
        let script = "fn detect(device, params) { loop { } }";
        let mut ctx = context(vec![], Effect::Read);
        ctx.limits.max_operations = 10_000;
        let outcome = execute(script, "detect", json!({}), ctx).await;
        assert!(outcome.output.unwrap_err().contains("limite de operações"));
    }

    #[tokio::test]
    async fn eval_e_import_nao_existem() {
        let eval = "fn detect(device, params) { eval(\"1\") }";
        assert!(
            execute(eval, "detect", json!({}), context(vec![], Effect::Read))
                .await
                .output
                .is_err()
        );
        assert!(entry_points("import \"x\" as y; fn detect(device, params) { 1 }").is_err());
    }

    #[tokio::test]
    async fn url_absoluta_e_recusada() {
        let script = r#"fn detect(device, params) { device.get("http://8.8.8.8/") }"#;
        let outcome = execute(script, "detect", json!({}), context(vec![], Effect::Read)).await;
        assert!(outcome.output.unwrap_err().contains("caminho relativo"));
    }

    #[tokio::test]
    async fn transporte_nao_declarado_e_recusado() {
        let script = r#"fn detect(device, params) { device.telnet("uptime") }"#;
        let outcome = execute(script, "detect", json!({}), context(vec![], Effect::Read)).await;
        assert!(outcome.output.unwrap_err().contains("não declara"));
    }

    #[tokio::test]
    async fn cookie_do_login_volta_na_proxima_chamada() {
        struct Echo;
        #[async_trait::async_trait]
        impl DeviceTransport for Echo {
            async fn execute(
                &self,
                call: &DeviceIoCall,
            ) -> crate::services::shared::errors::AppResult<DeviceIoReply> {
                let DeviceIoCall::HttpRequest { request, .. } = call else {
                    unreachable!()
                };
                let cookie = request
                    .headers
                    .iter()
                    .find(|(name, _)| name == "Cookie")
                    .map(|(_, value)| value.clone())
                    .unwrap_or_default();
                Ok(DeviceIoReply::Http {
                    status: 200,
                    headers: vec![("set-cookie".into(), "sysauth=abc; path=/".into())],
                    body: cookie,
                })
            }
            fn origin(&self) -> String {
                "eco".into()
            }
        }
        let script = r#"
            fn detect(device, params) {
                device.get("/a");
                device.get("/b").body
            }
        "#;
        let mut ctx = context(vec![], Effect::Read);
        ctx.transport = Arc::new(Echo);
        let outcome = execute(script, "detect", json!({}), ctx).await;
        assert_eq!(outcome.output.unwrap(), json!("sysauth=abc"));
    }

    #[test]
    fn utilitarios_de_texto() {
        let kv = parse_kv("A='1'\nB=\"dois\"\nsem separador\n", "=");
        assert_eq!(kv.get("A").map(String::as_str), Some("1"));
        assert_eq!(kv.get("B").map(String::as_str), Some("dois"));
        assert_eq!(kv.len(), 2);
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
        assert_eq!(url_encode("a b&c"), "a%20b%26c");
    }

    #[test]
    fn funcoes_do_script_com_aridade() {
        let functions = entry_points("fn detect(device, params) { 1 } fn aux(x) { x }").unwrap();
        assert!(functions.contains(&("detect".to_string(), 2)));
        assert!(functions.contains(&("aux".to_string(), 1)));
    }
}
