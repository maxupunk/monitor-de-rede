//! A camada de `tracing` que transforma evento da aplicação em log do
//! **dispositivo do sistema**.
//!
//! ```text
//! tracing::info!(…) → esta camada → LogQueue::try_enqueue → writer em lote
//!                                                                │
//!                                                          device_logs
//! ```
//!
//! # O que esta camada **não** faz
//!
//! Não cria fila, não cria escritor, não cria barramento e não abre banco. A
//! fila limitada com descarte contado (`queue`), a escrita em lote com gatilho
//! de 500 linhas / 200 ms (`writer`) e o barramento do live tail (`bus`) já
//! existem e são exatamente os que esta camada usa. Daí saem de graça: live
//! tail, SSE, busca, FTS, paginação e retenção.
//!
//! Também não passa pelo [`super::Ingestor`]: não há origem para resolver,
//! rate limit por fonte a aplicar nem registro em "origens vistas". A origem é
//! local e conhecida.
//!
//! # Por que a escrita não acontece dentro do callback
//!
//! `try_enqueue` nunca faz `await` e nunca toca no banco. Se a gravação
//! acontecesse aqui, cada `tracing::info!` de um handler HTTP esperaria um
//! `INSERT` — e o `INSERT` emitiria seus próprios eventos do SQLx, que
//! emitiriam outro `INSERT`. A fila é o que desacopla o request do banco e o
//! que corta a recursão.

use std::sync::{Arc, RwLock};

use chrono::Utc;
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::{layer::Context, registry::LookupSpan, Layer};

use super::{
    parser::ParsedLog,
    queue::{LogQueue, LogSource, PendingLog},
};
use crate::services::devices::system_device::resolver;

/// Origem das linhas emitidas por este processo.
///
/// `device_logs.source_ip` é `NOT NULL`, e `127.0.0.1` é o valor honesto: o
/// processo escreve de dentro de si mesmo. Inventar o IP da interface seria
/// afirmar algo que não foi observado.
pub const LOCAL_SOURCE_IP: &str = "127.0.0.1";

/// A fila global usada pela camada.
///
/// Global porque o `Hooks::init_logger` do Loco monta o subscriber **antes**
/// dos initializers e recebe o `AppContext` depois do `after_context`: quem
/// instala a camada não é quem constrói a fila.
///
/// `RwLock`, e não `OnceLock`: um `set` único faria a fila do primeiro
/// `syslog::build` do processo valer para sempre, e a suíte de testes monta um
/// pipeline por caso. Um `OnceLock` ali não é "mais simples" — é um caso em que
/// o segundo `build` grava numa fila cujo escritor já morreu, e a linha some
/// sem erro. A leitura é sob `read()`, sem contenção no caminho comum.
static QUEUE: RwLock<Option<LogQueue>> = RwLock::new(None);

/// Publica a fila para a camada. Chamado quando o pipeline monta.
pub fn install_queue(queue: LogQueue) {
    if let Ok(mut atual) = QUEUE.write() {
        *atual = Some(queue);
    }
}

/// Desliga a gravação em banco. O evento continua indo para o stdout.
///
/// Existe para o desligamento e para a suíte: sem isto, a fila de um teste
/// sobreviveria ao escritor dele e o teste seguinte gravaria num canal órfão.
pub fn clear_queue() {
    if let Ok(mut atual) = QUEUE.write() {
        *atual = None;
    }
}

/// Executa `acao` com a fila instalada, se houver.
///
/// O `LogQueue` é clonado para fora do bloqueio? Não: a ação roda **sob** o
/// `read()`, o que mantém a fila viva durante o `try_enqueue` sem custar dois
/// incrementos de `Arc` por evento de log.
fn com_a_fila<T>(acao: impl FnOnce(&LogQueue) -> T) -> Option<T> {
    let guarda = QUEUE.read().ok()?;
    guarda.as_ref().map(acao)
}

/// Alvos cujo log **não** vai para o banco.
///
/// A política é testável de propósito: é ela que impede a realimentação
/// `log → INSERT → log` e a poluição por consulta bem-sucedida.
///
/// - `backend::services::syslog::writer` é o próprio escritor: registrar que
///   gravou um lote geraria a linha seguinte, que geraria a próxima.
/// - `sqlx::query` emite **uma linha por consulta**, em `DEBUG`, incluindo os
///   `INSERT` deste mesmo caminho. Só o que é `WARN`/`ERROR` do SQLx interessa
///   — e esse continua passando, porque o corte é por nível, não por alvo.
/// - `sea_orm::driver` é o mesmo caso, um nível acima.
const ALVOS_SILENCIADOS: [&str; 4] = [
    "backend::services::syslog::writer",
    "backend::services::syslog::queue",
    "backend::services::syslog::app_layer",
    "backend::models::logs",
];

/// Alvos ruidosos por natureza, cortados **abaixo** de `WARN`.
///
/// Não é o mesmo que silenciar: um erro de driver precisa aparecer, e é
/// justamente ele que o operador procura quando o banco trava.
const ALVOS_RUIDOSOS: [&str; 2] = ["sqlx::query", "sea_orm::driver"];

/// Decide se um evento vira linha de log.
#[must_use]
pub fn deve_gravar(target: &str, level: Level) -> bool {
    if ALVOS_SILENCIADOS
        .iter()
        .any(|alvo| target.starts_with(alvo))
    {
        return false;
    }
    if ALVOS_RUIDOSOS.iter().any(|alvo| target.starts_with(alvo)) {
        // `Level` ordena do mais grave para o menos: `ERROR < WARN < INFO`.
        return level <= Level::WARN;
    }
    true
}

/// Severidade syslog equivalente ao nível do `tracing`.
///
/// É o que faz o filtro de severidade da tela funcionar igual para roteador e
/// para servidor: sem esta tradução, o log da aplicação ficaria fora de
/// qualquer filtro de gravidade e o operador teria de saber que existem duas
/// escalas.
#[must_use]
pub const fn severidade_syslog(level: Level) -> i16 {
    match level {
        Level::ERROR => 3,
        Level::WARN => 4,
        Level::INFO => 6,
        // DEBUG e TRACE viram `debug` (7): o syslog não tem nível abaixo dele.
        _ => 7,
    }
}

/// Achata os campos do evento numa única mensagem.
///
/// Os campos estruturados viram `chave=valor` no texto, como o próprio
/// formatador do `tracing` faz. Não há coluna JSON: o FTS indexa `message`, e
/// um campo dentro de JSON seria invisível à busca — o operador procuraria por
/// `monitor_id=7` e não acharia.
#[derive(Default)]
struct Achatador {
    mensagem: String,
    campos: Vec<String>,
}

impl tracing::field::Visit for Achatador {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        let texto = format!("{value:?}");
        // O campo `message` é o corpo do evento; os demais o acompanham.
        if field.name() == "message" {
            self.mensagem = texto.trim_matches('"').to_string();
        } else {
            self.campos
                .push(format!("{}={}", field.name(), texto.trim_matches('"')));
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.mensagem = value.to_string();
        } else {
            self.campos.push(format!("{}={value}", field.name()));
        }
    }
}

impl Achatador {
    fn finalize(self) -> String {
        if self.campos.is_empty() {
            return self.mensagem;
        }
        if self.mensagem.is_empty() {
            return self.campos.join(" ");
        }
        format!("{} {}", self.mensagem, self.campos.join(" "))
    }
}

/// A camada.
pub struct AppLogLayer;

impl<S> Layer<S> for AppLogLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();
        if !deve_gravar(metadata.target(), *metadata.level()) {
            return;
        }
        let mut achatador = Achatador::default();
        event.record(&mut achatador);
        let mensagem = achatador.finalize();
        if mensagem.is_empty() {
            return;
        }

        let parsed = ParsedLog {
            facility: None,
            severity: Some(severidade_syslog(*metadata.level())),
            device_time: None,
            hostname: None,
            // O `target` do evento é o `app_name`: é o que o operador filtra
            // quando quer "só o que veio do scheduler".
            app_name: Some(metadata.target().to_string()),
            pid: std::process::id().try_into().ok(),
            topics: None,
            message: mensagem,
        };

        // Antes de o pipeline montar (boot, migrations) não há fila: a linha
        // vai para o stdout e só. Perder log de boot no banco é aceitável;
        // travar o boot esperando por ele não é.
        //
        // Linhas emitidas antes de o **dispositivo** existir vão com
        // `device_id` nulo e aparecem em `/logs` sem filtro — comportamento
        // explícito e coberto por teste, não acidente.
        com_a_fila(|queue| {
            queue.try_enqueue(PendingLog {
                device_id: resolver::current(),
                source_ip: LOCAL_SOURCE_IP.to_string(),
                received_at: Utc::now(),
                parsed,
                source: LogSource::Application,
            })
        });
    }
}

/// A camada pronta para compor com as demais do `init_logger`.
#[must_use]
pub fn layer<S>() -> Arc<dyn Layer<S> + Send + Sync>
where
    S: Subscriber + for<'a> LookupSpan<'a> + Send + Sync,
{
    Arc::new(AppLogLayer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_proprio_escritor_nunca_se_registra() {
        // É esta linha que corta a realimentação `log → INSERT → log`.
        assert!(!deve_gravar(
            "backend::services::syslog::writer",
            Level::INFO
        ));
        assert!(!deve_gravar(
            "backend::services::syslog::writer::descarrega",
            Level::ERROR
        ));
    }

    #[test]
    fn consulta_sql_bem_sucedida_nao_entra_mas_o_erro_do_sqlx_entra() {
        assert!(!deve_gravar("sqlx::query", Level::DEBUG));
        assert!(!deve_gravar("sqlx::query", Level::INFO));
        assert!(
            deve_gravar("sqlx::query", Level::WARN),
            "esconder WARN/ERROR do SQLx tiraria justamente o que o operador procura"
        );
        assert!(deve_gravar("sqlx::query", Level::ERROR));
    }

    #[test]
    fn alvo_comum_da_aplicacao_passa_em_todos_os_niveis() {
        for nivel in [Level::ERROR, Level::WARN, Level::INFO, Level::DEBUG] {
            assert!(deve_gravar("backend::services::monitoring::runner", nivel));
        }
    }

    #[test]
    fn os_niveis_viram_a_severidade_syslog_equivalente() {
        assert_eq!(severidade_syslog(Level::ERROR), 3);
        assert_eq!(severidade_syslog(Level::WARN), 4);
        assert_eq!(severidade_syslog(Level::INFO), 6);
        assert_eq!(severidade_syslog(Level::DEBUG), 7);
        assert_eq!(severidade_syslog(Level::TRACE), 7);
    }

    #[test]
    fn os_campos_do_evento_sao_achatados_na_mensagem_e_nao_em_json() {
        let mut achatador = Achatador {
            mensagem: "monitor executado".into(),
            campos: vec!["monitor_id=7".into(), "status=up".into()],
        };
        let texto = std::mem::take(&mut achatador).finalize();
        assert_eq!(texto, "monitor executado monitor_id=7 status=up");
        // É isto que faz o FTS encontrar `monitor_id=7` de graça.
        assert!(texto.contains("monitor_id=7"));
    }

    #[test]
    fn evento_so_com_campos_ainda_produz_mensagem_util() {
        let achatador = Achatador {
            mensagem: String::new(),
            campos: vec!["erro=timeout".into()],
        };
        assert_eq!(achatador.finalize(), "erro=timeout");
    }
}
