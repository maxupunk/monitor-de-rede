//! Onde o resto do sistema consulta o Laya.
//!
//! Três cuidados que cada chamador teria de repetir ficam aqui:
//!
//! - **Configuração em cache** (30 s, invalidada ao salvar): a classificação
//!   de logs consultaria o banco a cada padrão novo.
//! - **Duas filas.** O que alguém espera na tela (chat, identificar, sugerir
//!   interfaces) passa direto; o que roda sozinho (descoberta, logs, triagem)
//!   entra um de cada vez — o Ollaya em CPU faz uma inferência por vez, e um
//!   lote de logs não pode empurrar o chat para além do tempo limite.
//! - **Modelo frio.** Tirar o modelo do disco leva de 5 a 20 s; o tempo
//!   máximo por pergunta vale só para a inferência. Quem está na tela
//!   esperando uma resposta pedida (`OnDemand`) e as rotinas (`Background`)
//!   aguardam o carregamento — a tela recebe `laya:model_state` pelo SSE e
//!   mostra "carregando". O chat (`Interactive`) não espera: dispara o
//!   carregamento e segue com as heurísticas naquela pergunta.
//! - **Disjuntor** para o segundo plano: 3 falhas seguidas o abrem por 60 s.
//!   Os logs da própria aplicação viram linhas de log (`syslog::app_layer`);
//!   avisar a cada falha encheria o banco com o aviso. Avisa ao abrir e ao
//!   fechar, e só.
//!
//! Nunca falha: sem Laya, a resposta é `None` e quem chamou segue com a
//! heurística de sempre.

use std::{
    sync::{Arc, Mutex, RwLock},
    time::{Duration, Instant},
};

use loco_rs::prelude::AppContext;
use sea_orm::ConnectionTrait;
use serde_json::json;
use tokio::sync::Semaphore;

use super::{
    client::LayaClient,
    config::{AiLayaSettings, LayaFeature},
    decision::{self, Decision, Outcome},
};
use crate::services::{
    ai::{local_models, settings},
    events::EventBus,
    shared::errors::{AppError, AppResult},
};

/// Evento SSE com o estado do modelo: `loading`, `ready` ou `failed`.
pub const MODEL_STATE_EVENT: &str = "laya:model_state";
/// Por quanto tempo "está na memória" vale sem perguntar de novo ao Ollaya.
/// Curto: o `OLLAYA_KEEP_ALIVE` pode descarregar o modelo a qualquer momento.
const LOADED_TTL: Duration = Duration::from_secs(20);

const SETTINGS_TTL: Duration = Duration::from_secs(30);
const BREAKER_THRESHOLD: u32 = 3;
const BREAKER_COOLDOWN: Duration = Duration::from_secs(60);

/// Quem espera pela resposta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lane {
    /// Alguém pediu esta resposta na tela (testar, sugerir interfaces):
    /// espera o modelo carregar, com a tela avisando pelo SSE.
    OnDemand,
    /// Alguém na tela: passa direto, sem fila nem disjuntor.
    Interactive,
    /// Rotina: um por vez e respeitando o disjuntor.
    Background,
}

#[derive(Debug, Default)]
struct Breaker {
    failures: u32,
    open_until: Option<Instant>,
}

impl Breaker {
    fn is_open(&self, now: Instant) -> bool {
        self.open_until.is_some_and(|until| now < until)
    }

    /// Registra o resultado. Devolve a transição, para avisar uma vez só.
    fn record(&mut self, ok: bool, now: Instant) -> Option<bool> {
        if ok {
            self.failures = 0;
            return self.open_until.take().map(|_| false);
        }
        self.failures += 1;
        if self.failures >= BREAKER_THRESHOLD && !self.is_open(now) {
            let reopened = self.open_until.is_none();
            self.open_until = Some(now + BREAKER_COOLDOWN);
            return reopened.then_some(true);
        }
        None
    }
}

struct Inner {
    settings: RwLock<Option<(AiLayaSettings, Instant)>>,
    background: Semaphore,
    breaker: Mutex<Breaker>,
    /// Um carregamento por vez: quem chega depois espera e encontra quente.
    loading: tokio::sync::Mutex<()>,
    /// `servidor|modelo` visto na memória, e quando.
    warm: Mutex<Option<(String, Instant)>>,
    events: Option<EventBus>,
}

#[derive(Clone)]
pub struct LayaRuntime {
    inner: Arc<Inner>,
}

impl Default for LayaRuntime {
    fn default() -> Self {
        Self {
            inner: Arc::new(Inner {
                settings: RwLock::new(None),
                background: Semaphore::new(1),
                breaker: Mutex::new(Breaker::default()),
                loading: tokio::sync::Mutex::new(()),
                warm: Mutex::new(None),
                events: None,
            }),
        }
    }
}

impl LayaRuntime {
    /// O runtime do processo, criado na primeira consulta.
    #[must_use]
    pub fn from_context(ctx: &AppContext) -> Self {
        if let Some(runtime) = ctx.shared_store.get::<Self>() {
            return runtime;
        }
        let mut runtime = Self::default();
        if let Some(inner) = Arc::get_mut(&mut runtime.inner) {
            inner.events = EventBus::from_context(ctx).ok();
        }
        ctx.shared_store.insert(runtime.clone());
        runtime
    }

    /// A configuração mudou: a próxima consulta relê do banco.
    pub fn invalidate(&self) {
        if let Ok(mut cached) = self.inner.settings.write() {
            *cached = None;
        }
    }

    fn cached_settings(&self) -> Option<AiLayaSettings> {
        let cached = self.inner.settings.read().ok()?;
        cached
            .as_ref()
            .filter(|(_, loaded)| loaded.elapsed() < SETTINGS_TTL)
            .map(|(settings, _)| settings.clone())
    }

    /// A configuração, se o Laya está ligado e a frente liberada.
    pub async fn settings_for<C: ConnectionTrait>(
        &self,
        db: &C,
        feature: LayaFeature,
    ) -> Option<AiLayaSettings> {
        let settings = match self.cached_settings() {
            Some(settings) => settings,
            None => {
                let settings = settings::load(db).await.ok()?.laya.normalized();
                if let Ok(mut cached) = self.inner.settings.write() {
                    *cached = Some((settings.clone(), Instant::now()));
                }
                settings
            }
        };
        settings.allows(feature).then_some(settings)
    }

    /// Consulta com a configuração gravada.
    pub async fn decide<D: Decision + Sync, C: ConnectionTrait>(
        &self,
        db: &C,
        feature: LayaFeature,
        lane: Lane,
        state: &str,
        decision: &D,
    ) -> Option<Outcome<D::Output>> {
        let settings = self.settings_for(db, feature).await?;
        self.decide_with(&settings, lane, state, decision).await
    }

    /// Consulta com uma configuração dada (a do formulário, ou a de um teste).
    pub async fn decide_with<D: Decision + Sync>(
        &self,
        settings: &AiLayaSettings,
        lane: Lane,
        state: &str,
        decision: &D,
    ) -> Option<Outcome<D::Output>> {
        self.try_decide_with(settings, lane, state, decision)
            .await
            .ok()
    }

    /// O mesmo, com o motivo da falha — para a tela explicar o que houve.
    ///
    /// # Errors
    ///
    /// Disjuntor aberto, modelo frio no chat, falha ao carregar ou ao decidir.
    pub async fn try_decide_with<D: Decision + Sync>(
        &self,
        settings: &AiLayaSettings,
        lane: Lane,
        state: &str,
        decision: &D,
    ) -> AppResult<Outcome<D::Output>> {
        let _permit = match lane {
            Lane::Interactive | Lane::OnDemand => None,
            Lane::Background => {
                if self.breaker_open() {
                    return Err(AppError::service_unavailable(
                        "Laya em pausa depois de falhas seguidas",
                    ));
                }
                self.inner.background.acquire().await.ok()
            }
        };
        match lane {
            Lane::Interactive => {
                if self.is_cold(settings).await {
                    self.warm_in_background(settings);
                    return Err(AppError::service_unavailable(
                        "O modelo do Laya está sendo carregado na memória",
                    ));
                }
            }
            Lane::OnDemand | Lane::Background => self.ensure_loaded(settings).await?,
        }
        let result = decision::run(&LayaClient::new(settings), state, decision).await;
        if result.is_ok() {
            self.mark_warm(settings);
        }
        self.record(
            result.is_ok(),
            result.as_ref().err().map(ToString::to_string),
        );
        result
    }

    fn warm_key(settings: &AiLayaSettings) -> String {
        format!("{}|{}", settings.base_url, settings.model)
    }

    fn mark_warm(&self, settings: &AiLayaSettings) {
        if let Ok(mut warm) = self.inner.warm.lock() {
            *warm = Some((Self::warm_key(settings), Instant::now()));
        }
    }

    fn recently_warm(&self, settings: &AiLayaSettings) -> bool {
        let key = Self::warm_key(settings);
        self.inner.warm.lock().is_ok_and(|warm| {
            warm.as_ref()
                .is_some_and(|(seen, at)| *seen == key && at.elapsed() < LOADED_TTL)
        })
    }

    /// O modelo está fora da memória? Na dúvida (Ollaya antigo sem
    /// `/api/ps`, erro), responde que não — e a consulta segue como antes.
    async fn is_cold(&self, settings: &AiLayaSettings) -> bool {
        if self.recently_warm(settings) {
            return false;
        }
        match LayaClient::new(settings).loaded_models().await {
            Ok(loaded) => {
                let warm = local_models::is_installed(&loaded, &settings.model);
                if warm {
                    self.mark_warm(settings);
                }
                !warm
            }
            Err(_) => false,
        }
    }

    fn publish_state(&self, settings: &AiLayaSettings, state: &str, extra: serde_json::Value) {
        if let Some(events) = &self.inner.events {
            let mut payload = json!({
                "model": settings.model,
                "baseUrl": settings.base_url,
                "state": state,
            });
            if let (Some(payload), Some(extra)) = (payload.as_object_mut(), extra.as_object()) {
                payload.extend(extra.clone());
            }
            events.publish_ephemeral(MODEL_STATE_EVENT, payload);
        }
    }

    /// Garante o modelo na memória antes de perguntar. Um carregamento por
    /// vez; quem espera na fila reconfere e, em geral, já encontra quente.
    ///
    /// # Errors
    ///
    /// O Ollaya não conseguiu carregar o modelo no prazo de carregamento.
    pub async fn ensure_loaded(&self, settings: &AiLayaSettings) -> AppResult<()> {
        if !self.is_cold(settings).await {
            return Ok(());
        }
        let _loading = self.inner.loading.lock().await;
        if !self.is_cold(settings).await {
            return Ok(());
        }
        self.publish_state(settings, "loading", json!({}));
        tracing::info!(model = %settings.model, "laya: carregando o modelo na memória");
        match LayaClient::new(settings).load().await {
            Ok(load_ms) => {
                self.mark_warm(settings);
                self.publish_state(settings, "ready", json!({ "loadMs": load_ms }));
                tracing::info!(model = %settings.model, load_ms, "laya: modelo carregado");
                Ok(())
            }
            Err(error) => {
                self.publish_state(settings, "failed", json!({ "message": error.to_string() }));
                Err(error)
            }
        }
    }

    /// Dispara o carregamento sem esperar. Se já há um em curso, não faz nada.
    pub fn warm_in_background(&self, settings: &AiLayaSettings) {
        if self.inner.loading.try_lock().is_err() {
            return;
        }
        let runtime = self.clone();
        let settings = settings.clone();
        tokio::spawn(async move {
            if let Err(error) = runtime.ensure_loaded(&settings).await {
                tracing::debug!(%error, "laya: aquecimento do modelo falhou");
            }
        });
    }

    fn breaker_open(&self) -> bool {
        self.inner
            .breaker
            .lock()
            .is_ok_and(|breaker| breaker.is_open(Instant::now()))
    }

    fn record(&self, ok: bool, error: Option<String>) {
        let Ok(mut breaker) = self.inner.breaker.lock() else {
            return;
        };
        match breaker.record(ok, Instant::now()) {
            Some(true) => tracing::warn!(
                error = error.as_deref().unwrap_or_default(),
                "laya: {BREAKER_THRESHOLD} falhas seguidas; rotinas em segundo plano pausadas por {}s",
                BREAKER_COOLDOWN.as_secs()
            ),
            Some(false) => tracing::info!("laya: respondendo de novo; rotinas retomadas"),
            None => {
                if let Some(error) = error {
                    tracing::debug!(%error, "laya: consulta falhou");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::services::ai::laya::schema::{Answer, Question};

    struct Nothing;

    impl Decision for Nothing {
        type Output = ();

        fn questions(&self) -> BTreeMap<String, Question> {
            BTreeMap::from([(
                "x".to_string(),
                Question::Noul {
                    instructions: "x".into(),
                },
            )])
        }

        fn interpret(&self, _answers: &BTreeMap<String, Answer>) {}
    }

    fn unreachable_settings() -> AiLayaSettings {
        AiLayaSettings {
            enabled: true,
            base_url: "http://127.0.0.1:9".into(),
            timeout_ms: 300,
            ..AiLayaSettings::default()
        }
    }

    #[test]
    fn disjuntor_abre_na_terceira_falha_e_fecha_no_sucesso() {
        let now = Instant::now();
        let mut breaker = Breaker::default();
        assert_eq!(breaker.record(false, now), None);
        assert_eq!(breaker.record(false, now), None);
        assert_eq!(breaker.record(false, now), Some(true), "avisa ao abrir");
        assert!(breaker.is_open(now));
        assert_eq!(breaker.record(false, now), None, "não repete o aviso");
        assert!(!breaker.is_open(now + BREAKER_COOLDOWN));
        assert_eq!(breaker.record(true, now), Some(false), "avisa ao fechar");
        assert!(!breaker.is_open(now));
    }

    #[tokio::test]
    async fn segundo_plano_para_de_consultar_com_o_disjuntor_aberto() {
        let runtime = LayaRuntime::default();
        let settings = unreachable_settings();
        for _ in 0..BREAKER_THRESHOLD {
            assert!(runtime
                .decide_with(&settings, Lane::Background, "s", &Nothing)
                .await
                .is_none());
        }
        assert!(runtime.breaker_open());
        let started = Instant::now();
        assert!(runtime
            .decide_with(&settings, Lane::Background, "s", &Nothing)
            .await
            .is_none());
        assert!(
            started.elapsed() < Duration::from_millis(50),
            "com o disjuntor aberto nem tenta a rede"
        );
    }

    #[tokio::test]
    async fn fila_interativa_nao_espera_o_segundo_plano() {
        let runtime = LayaRuntime::default();
        let _held = runtime.inner.background.acquire().await.unwrap();
        let result = tokio::time::timeout(
            Duration::from_secs(3),
            runtime.decide_with(&unreachable_settings(), Lane::Interactive, "s", &Nothing),
        )
        .await;
        assert!(
            result.is_ok(),
            "não ficou preso no semáforo do segundo plano"
        );
    }
}
