//! Janela de contexto do modelo: quantos tokens a conversa pode ocupar.
//!
//! A fonte preferida é o próprio provedor ([`sources`]), consultada pelo
//! driver e guardada em cache. Sem resposta dele — sem rede, catálogo fora do
//! ar, modelo desconhecido —, vale a [`estimate`] pelo nome. A tela sabe qual
//! das duas chegou: janela estimada é mostrada como estimada.

pub mod estimate;
pub mod sources;

use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

pub use estimate::{estimate, DEFAULT_CONTEXT_WINDOW};
use sources::{LearnedWindow, WindowLookup};

use super::drivers::traits::AiDriver;

/// Por quanto tempo um "o provedor não conhece este modelo" vale.
const MISS_TTL: Duration = Duration::from_secs(10 * 60);

/// Janela resolvida para um modelo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextWindow {
    pub tokens: u64,
    /// Informada pelo provedor (`true`) ou estimada (`false`).
    pub reported: bool,
}

struct Entry {
    window: Option<LearnedWindow>,
    expires: Instant,
}

fn cache() -> &'static Mutex<HashMap<String, Entry>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Entry>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cache_key(source: &dyn WindowLookup, model: &str) -> String {
    format!("{}|{model}", source.cache_key())
}

/// O id pedido e, para ids com prefixo de provedor (`opencode/x`), o nome
/// sem ele — o catálogo pode listar de qualquer um dos dois jeitos.
fn candidates(model: &str) -> Vec<&str> {
    let model = model.trim();
    let mut list = vec![model];
    if let Some((_, bare)) = model.rsplit_once('/') {
        if !bare.is_empty() {
            list.push(bare);
        }
    }
    list
}

/// Consulta a fonte passando pelo cache.
///
/// Uma consulta pode ensinar o catálogo inteiro; tudo o que for certo fica
/// guardado pelo `ttl` da fonte. O que é presumido (Ollama sem modelo
/// carregado) vale só para esta resposta.
pub async fn lookup(source: &dyn WindowLookup, model: &str) -> Option<LearnedWindow> {
    let now = Instant::now();
    {
        let guard = cache().lock().ok()?;
        for candidate in candidates(model) {
            if let Some(entry) = guard.get(&cache_key(source, candidate)) {
                if entry.expires > now {
                    if let Some(window) = entry.window {
                        return Some(window);
                    }
                    if candidate == model.trim() {
                        return None;
                    }
                }
            }
        }
    }

    let learned = source.fetch(model).await;
    let found = candidates(model)
        .into_iter()
        .find_map(|candidate| learned.get(candidate).copied());

    if let Ok(mut guard) = cache().lock() {
        let expires = Instant::now() + source.ttl();
        for (id, window) in &learned {
            if window.confident {
                guard.insert(
                    cache_key(source, id),
                    Entry {
                        window: Some(*window),
                        expires,
                    },
                );
            }
        }
        if found.is_none() {
            guard.insert(
                cache_key(source, model.trim()),
                Entry {
                    window: None,
                    expires: Instant::now() + MISS_TTL,
                },
            );
        }
    }
    found
}

/// A janela de `model`: a do provedor quando ele informa, senão a estimada.
pub async fn resolve(driver: &dyn AiDriver, model: &str) -> ContextWindow {
    match driver.context_window(model).await {
        Some(window) => ContextWindow {
            tokens: window.tokens,
            reported: window.confident,
        },
        None => ContextWindow {
            tokens: estimate(model),
            reported: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    use async_trait::async_trait;

    use super::*;

    struct Contador {
        chave: String,
        consultas: Arc<AtomicUsize>,
        catalogo: HashMap<String, LearnedWindow>,
    }

    #[async_trait]
    impl WindowLookup for Contador {
        fn cache_key(&self) -> String {
            self.chave.clone()
        }

        async fn fetch(&self, _model: &str) -> HashMap<String, LearnedWindow> {
            self.consultas.fetch_add(1, Ordering::SeqCst);
            self.catalogo.clone()
        }
    }

    fn fonte(chave: &str, catalogo: &[(&str, LearnedWindow)]) -> (Contador, Arc<AtomicUsize>) {
        let consultas = Arc::new(AtomicUsize::new(0));
        (
            Contador {
                chave: chave.into(),
                consultas: consultas.clone(),
                catalogo: catalogo
                    .iter()
                    .map(|(id, janela)| ((*id).to_string(), *janela))
                    .collect(),
            },
            consultas,
        )
    }

    #[tokio::test]
    async fn uma_consulta_ensina_o_catalogo_inteiro() {
        let (fonte, consultas) = fonte(
            "teste-catalogo",
            &[
                ("a", LearnedWindow::confident(1000)),
                ("b", LearnedWindow::confident(2000)),
            ],
        );
        assert_eq!(lookup(&fonte, "a").await.map(|j| j.tokens), Some(1000));
        assert_eq!(lookup(&fonte, "b").await.map(|j| j.tokens), Some(2000));
        assert_eq!(lookup(&fonte, "a").await.map(|j| j.tokens), Some(1000));
        assert_eq!(consultas.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn modelo_desconhecido_tambem_fica_em_cache() {
        let (fonte, consultas) = fonte("teste-ausente", &[("a", LearnedWindow::confident(1))]);
        assert_eq!(lookup(&fonte, "zzz").await, None);
        assert_eq!(lookup(&fonte, "zzz").await, None);
        assert_eq!(consultas.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn janela_presumida_nao_fica_em_cache() {
        let presumida = LearnedWindow {
            tokens: 4096,
            confident: false,
        };
        let (fonte, consultas) = fonte("teste-presumida", &[("llama3.2", presumida)]);
        assert_eq!(lookup(&fonte, "llama3.2").await, Some(presumida));
        assert_eq!(lookup(&fonte, "llama3.2").await, Some(presumida));
        assert_eq!(consultas.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn id_com_prefixo_de_provedor_acha_o_nome_simples() {
        let (fonte, _) = fonte(
            "teste-prefixo",
            &[("muse-spark-1.3", LearnedWindow::confident(1_048_576))],
        );
        assert_eq!(
            lookup(&fonte, "opencode/muse-spark-1.3")
                .await
                .map(|j| j.tokens),
            Some(1_048_576)
        );
    }
}
