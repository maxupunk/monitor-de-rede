//! Máquina de estados pura do ciclo de vida do alerta (§2.1 da análise de
//! monitoramento inteligente, Fase 1 do roadmap).
//!
//! Toda decisão temporal — entrar em `recovering`, contar recaída, fechar de
//! vez — vive aqui, sem I/O: o chamador passa o instante atual (Clock
//! injetável) e recebe uma [`Transition`] para persistir e orquestrar. Em
//! produção `now` é `Utc::now()`; nos testes, um tempo fixo — é o que torna
//! cada ramo da máquina testável em tabela, sem banco e sem `sleep`.
//!
//! A janela de estabilidade conta do **último** problema, não do primeiro:
//! cada recaída reinicia o relógio (`data.lastProblemAt = now`).

use chrono::{DateTime, Utc};
use serde_json::{Map, Value};

use super::contracts::AlertStatus;

/// Chaves de `alert_events.data` que a máquina mantém (camelCase, como
/// `silencedUntil`).
pub const LAST_PROBLEM_AT: &str = "lastProblemAt";
pub const RECURRENCE_COUNT: &str = "recurrenceCount";

/// O que o episódio acumulou até fechar — vai no resumo da notificação de
/// resolução ("oscilou 7 vezes; estável há 5 min").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EpisodeSummary {
    /// Quantas recaídas (problema de volta dentro da janela) desde a abertura.
    pub recurrence: u64,
    /// Segundos estáveis entre o último problema e a resolução.
    pub stable_for_seconds: i64,
}

/// Decisão da máquina para um evento aberto diante da avaliação atual.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    /// Nada muda.
    None,
    /// Condição batendo num evento já aberto (`active`/`acknowledged`/
    /// `silenced`): o problema apenas continua — e é esse carimbo
    /// (`lastProblemAt = now`) que ancora a janela no último down.
    ProblemOngoing,
    /// Condição limpa com janela a respeitar: o evento passa a observar a
    /// estabilidade em vez de fechar na primeira checagem ok.
    EnterRecovering,
    /// Problema de volta dentro da janela: reabre (ou permanece silenciado,
    /// se o silêncio ainda vigora) e a contagem de estabilidade reinicia —
    /// sem evento novo e sem notificação.
    Relapse { recurrence: u64, silenced: bool },
    /// Janela esgotada sem recaída (ou janela 0): fecha o evento.
    Resolve { summary: EpisodeSummary },
}

/// Tudo que a máquina precisa saber; nada que ela precise buscar.
#[derive(Debug)]
pub struct EpisodeInput<'a> {
    /// Status atual do evento aberto.
    pub status: AlertStatus,
    /// `alert_events.data` — de onde saem `lastProblemAt` e `recurrenceCount`.
    pub data: Option<&'a Map<String, Value>>,
    /// Abertura do evento: âncora da janela quando `lastProblemAt` falta
    /// (eventos abertos antes da Fase 1).
    pub started_at: DateTime<Utc>,
    /// `recovery_window_seconds` da regra do evento; 0 quando a regra sumiu —
    /// sem regra não há janela a respeitar.
    pub recovery_window_seconds: i64,
    /// A condição da regra bateu nesta avaliação.
    pub condition_matched: bool,
    /// O alvo voltou ao normal nesta avaliação.
    pub recovered: bool,
    /// Há silêncio vigente (`data.silencedUntil` no futuro): a recaída devolve
    /// o evento a `silenced`, não a `active` — a recaída não pode furar o
    /// silêncio que o operador pediu.
    pub silenced_now: bool,
    /// O relógio, injetado pelo chamador.
    pub now: DateTime<Utc>,
}

/// Decide a transição de um evento aberto. Puro: mesma entrada, mesma saída.
#[must_use]
pub fn decide(input: &EpisodeInput) -> Transition {
    if input.condition_matched {
        // Recaída só existe sobre `recovering`: o alerta estava observando a
        // estabilidade e o problema voltou. Num evento ainda em falha o
        // problema apenas continua — sem contador, sem mudança de status.
        return if input.status == AlertStatus::Recovering {
            Transition::Relapse {
                recurrence: recurrence_count(input.data) + 1,
                silenced: input.silenced_now,
            }
        } else {
            Transition::ProblemOngoing
        };
    }

    if !input.recovered || !input.status.is_open() {
        return Transition::None;
    }

    let window = input.recovery_window_seconds.max(0);
    // Janela 0 é o comportamento original: a primeira checagem ok resolve.
    if window == 0 {
        return Transition::Resolve {
            summary: summary_of(input),
        };
    }

    match input.status {
        // A saída de `recovering` exige uma **nova** checagem ok depois de a
        // janela vencer: quem acabou de entrar no estado ainda está sob
        // observação, mesmo que o último problema seja antigo.
        AlertStatus::Active | AlertStatus::Acknowledged | AlertStatus::Silenced => {
            Transition::EnterRecovering
        }
        AlertStatus::Recovering if stable_seconds(input) >= window => Transition::Resolve {
            summary: summary_of(input),
        },
        _ => Transition::None,
    }
}

/// Recaídas acumuladas; dado ausente ou ilegível conta como zero.
#[must_use]
pub fn recurrence_count(data: Option<&Map<String, Value>>) -> u64 {
    data.and_then(|map| map.get(RECURRENCE_COUNT))
        .and_then(Value::as_u64)
        .unwrap_or(0)
}

/// Último problema registrado; na falta, a abertura do evento — um episódio
/// sem recaída estava "em problema" até a primeira checagem ok.
#[must_use]
pub fn last_problem_at(
    data: Option<&Map<String, Value>>,
    started_at: DateTime<Utc>,
) -> DateTime<Utc> {
    data.and_then(|map| map.get(LAST_PROBLEM_AT))
        .and_then(Value::as_str)
        .and_then(|raw| DateTime::parse_from_rfc3339(raw).ok())
        .map(|value| value.with_timezone(&Utc))
        .unwrap_or(started_at)
}

fn stable_seconds(input: &EpisodeInput) -> i64 {
    (input.now - last_problem_at(input.data, input.started_at))
        .num_seconds()
        .max(0)
}

fn summary_of(input: &EpisodeInput) -> EpisodeSummary {
    EpisodeSummary {
        recurrence: recurrence_count(input.data),
        stable_for_seconds: stable_seconds(input),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use serde_json::json;

    /// Tempo fixo: a máquina inteira é exercitada contra este relógio.
    fn t0() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-08-15T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn dados(pairs: &[(&str, Value)]) -> Map<String, Value> {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), value.clone()))
            .collect()
    }

    fn entrada<'a>(
        status: AlertStatus,
        data: Option<&'a Map<String, Value>>,
        window: i64,
    ) -> EpisodeInput<'a> {
        EpisodeInput {
            status,
            data,
            started_at: t0(),
            recovery_window_seconds: window,
            condition_matched: false,
            recovered: true,
            silenced_now: false,
            now: t0(),
        }
    }

    // --- Condição batendo ---------------------------------------------------

    #[test]
    fn condicao_batendo_em_active_so_carimba_o_ultimo_problema() {
        for status in [
            AlertStatus::Active,
            AlertStatus::Acknowledged,
            AlertStatus::Silenced,
        ] {
            let mut input = entrada(status, None, 300);
            input.condition_matched = true;
            assert_eq!(decide(&input), Transition::ProblemOngoing);
        }
    }

    #[test]
    fn condicao_batendo_em_recovering_e_recaida_e_soma_o_contador() {
        let data = dados(&[
            (RECURRENCE_COUNT, json!(6)),
            (LAST_PROBLEM_AT, json!("2026-08-15T11:59:00Z")),
        ]);
        let mut input = entrada(AlertStatus::Recovering, Some(&data), 300);
        input.condition_matched = true;
        assert_eq!(
            decide(&input),
            Transition::Relapse {
                recurrence: 7,
                silenced: false
            }
        );
    }

    #[test]
    fn recaida_com_silencio_vigente_permanece_silenciada() {
        let mut input = entrada(AlertStatus::Recovering, None, 300);
        input.condition_matched = true;
        input.silenced_now = true;
        assert_eq!(
            decide(&input),
            Transition::Relapse {
                recurrence: 1,
                silenced: true
            }
        );
    }

    // --- Condição limpa, janela 0 -------------------------------------------

    #[test]
    fn janela_zero_resolve_na_primeira_checagem_ok() {
        for status in AlertStatus::OPEN {
            let input = entrada(status, None, 0);
            assert!(matches!(decide(&input), Transition::Resolve { .. }));
        }
    }

    #[test]
    fn janela_zero_resume_o_episodio_com_as_recaidas_acumuladas() {
        let data = dados(&[
            (RECURRENCE_COUNT, json!(3)),
            (LAST_PROBLEM_AT, json!("2026-08-15T11:58:40Z")),
        ]);
        let input = entrada(AlertStatus::Active, Some(&data), 0);
        let Transition::Resolve { summary } = decide(&input) else {
            panic!("janela 0 resolve");
        };
        assert_eq!(summary.recurrence, 3);
        assert_eq!(summary.stable_for_seconds, 80);
    }

    // --- Condição limpa, janela aberta --------------------------------------

    #[test]
    fn primeira_checagem_ok_com_janela_entra_em_recovering() {
        for status in [
            AlertStatus::Active,
            AlertStatus::Acknowledged,
            AlertStatus::Silenced,
        ] {
            let input = entrada(status, None, 300);
            assert_eq!(decide(&input), Transition::EnterRecovering);
        }
    }

    #[test]
    fn recovering_dentro_da_janela_permanece_observando() {
        // Último problema há 60 s, janela de 300 s: ainda não fechou.
        let data = dados(&[(LAST_PROBLEM_AT, json!("2026-08-15T11:59:00Z"))]);
        let input = entrada(AlertStatus::Recovering, Some(&data), 300);
        assert_eq!(decide(&input), Transition::None);
    }

    #[test]
    fn recovering_com_a_janela_vencida_resolve() {
        // Último problema há exatamente 300 s: a janela se esgotou.
        let data = dados(&[(LAST_PROBLEM_AT, json!("2026-08-15T11:55:00Z"))]);
        let input = entrada(AlertStatus::Recovering, Some(&data), 300);
        let Transition::Resolve { summary } = decide(&input) else {
            panic!("janela vencida resolve");
        };
        assert_eq!(summary.stable_for_seconds, 300);
    }

    #[test]
    fn recaida_reinicia_a_janela() {
        // Estável por 280 s → recaída → o relógio zera: 280 s depois da
        // recaída ainda não resolve, apesar de 560 s desde o problema original.
        let antes = dados(&[(LAST_PROBLEM_AT, json!("2026-08-15T11:55:20Z"))]);
        let mut input = entrada(AlertStatus::Recovering, Some(&antes), 300);
        input.condition_matched = true;
        assert_eq!(
            decide(&input),
            Transition::Relapse {
                recurrence: 1,
                silenced: false
            }
        );

        // A recaída carimbou lastProblemAt = t0; 280 s depois ainda não fecha.
        let depois = dados(&[
            (RECURRENCE_COUNT, json!(1)),
            (LAST_PROBLEM_AT, json!("2026-08-15T12:00:00Z")),
        ]);
        let mut input = entrada(AlertStatus::Recovering, Some(&depois), 300);
        input.now = t0() + Duration::seconds(280);
        assert_eq!(decide(&input), Transition::None);

        // Só fecha quando a nova janela inteira se esgota.
        input.now = t0() + Duration::seconds(300);
        assert!(matches!(decide(&input), Transition::Resolve { .. }));
    }

    // --- Ramos neutros ------------------------------------------------------

    #[test]
    fn sem_recuperacao_ou_evento_fechado_nada_acontece() {
        let mut input = entrada(AlertStatus::Active, None, 300);
        input.recovered = false;
        assert_eq!(decide(&input), Transition::None);

        let input = entrada(AlertStatus::Resolved, None, 0);
        assert_eq!(decide(&input), Transition::None);
    }

    #[test]
    fn sem_last_problem_at_a_janela_conta_da_abertura() {
        // Evento aberto antes da Fase 1 não tem o carimbo: a abertura ancora.
        let mut input = entrada(AlertStatus::Recovering, None, 300);
        input.now = t0() + Duration::seconds(299);
        assert_eq!(decide(&input), Transition::None);
        input.now = t0() + Duration::seconds(300);
        assert!(matches!(decide(&input), Transition::Resolve { .. }));
    }

    #[test]
    fn leituras_de_data_toleram_ausencia_e_lixo() {
        assert_eq!(recurrence_count(None), 0);
        let lixo = dados(&[
            (RECURRENCE_COUNT, json!("muitas")),
            (LAST_PROBLEM_AT, json!("ontem")),
        ]);
        assert_eq!(recurrence_count(Some(&lixo)), 0);
        assert_eq!(last_problem_at(Some(&lixo), t0()), t0());
        assert_eq!(last_problem_at(None, t0()), t0());
    }
}
