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
//! cada recaída reinicia o relógio (`data.lastProblemAt = now`). Desde a Fase
//! 2, "problema" inclui a degradação (`warning`): perda parcial de pacotes ou
//! DNS parcial não disparam regra por si, mas reiniciam a janela como qualquer
//! recaída — um link intermitente não resolve como se estivesse saudável.
//!
//! A Fase 3 acrescenta o estado `flapping`. A janela da Fase 1 já mantém o
//! episódio aberto durante a oscilação; o que faltava era **nomear** o alvo
//! cronicamente instável. Cada recaída deixa um carimbo em
//! `data.problemTimeline`, e quando a contagem dentro de
//! `flap_window_seconds` alcança `flap_threshold` o episódio vira `flapping`:
//! um único aviso "alvo oscilando" e nada mais até a contagem deslizante
//! decair **e** a estabilidade voltar. Como o flapping é detectado sobre o
//! episódio, ele pressupõe `recovery_window_seconds > 0` — sem janela o evento
//! fecha na primeira checagem ok e nunca chega a recair.

use chrono::{DateTime, Duration, Utc};
use serde_json::{Map, Value};

use super::contracts::AlertStatus;

/// Chaves de `alert_events.data` que a máquina mantém (camelCase, como
/// `silencedUntil`).
pub const LAST_PROBLEM_AT: &str = "lastProblemAt";
pub const RECURRENCE_COUNT: &str = "recurrenceCount";
/// Carimbos RFC 3339 das recaídas, do mais velho para o mais novo — a janela
/// deslizante da detecção de flapping.
pub const PROBLEM_TIMELINE: &str = "problemTimeline";
/// Quando o episódio foi declarado oscilante; sobrevive à resolução para que a
/// notificação final saiba contar a história.
pub const FLAPPING_SINCE: &str = "flappingSince";

/// Teto de carimbos guardados por episódio. Um alvo muito instável poderia
/// crescer o JSON sem limite; o que passa disso não muda nenhuma decisão,
/// porque a janela deslizante já descarta o que envelheceu.
const MAX_TIMELINE: usize = 64;

/// Os parâmetros de episódio que vêm da regra (§3.4 do roadmap: comportamento
/// novo é sempre parâmetro de regra, nunca constante global).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EpisodePolicy {
    /// Estabilidade exigida antes de resolver; 0 = resolve na primeira ok.
    pub recovery_window_seconds: i64,
    /// Recaídas dentro da janela que declaram o alvo oscilando; 0 = desligado.
    pub flap_threshold: u32,
    /// Largura da janela deslizante de flap.
    pub flap_window_seconds: i64,
}

impl EpisodePolicy {
    /// Só há detecção de flapping com limiar **e** janela configurados: um dos
    /// dois zerado é "desligado", não "dispara sempre".
    #[must_use]
    pub const fn flap_enabled(self) -> bool {
        self.flap_threshold > 0 && self.flap_window_seconds > 0
    }
}

/// A regra é a única fonte destes parâmetros (§3.4). A conversão vive aqui, ao
/// lado da struct, para que manager e recovery leiam a mesma política — e
/// continua pura: nada de banco, só a leitura da linha já carregada.
impl From<&crate::models::alert_rules::Model> for EpisodePolicy {
    fn from(rule: &crate::models::alert_rules::Model) -> Self {
        Self {
            recovery_window_seconds: i64::from(rule.recovery_window_seconds),
            flap_threshold: u32::try_from(rule.flap_threshold).unwrap_or(0),
            flap_window_seconds: i64::from(rule.flap_window_seconds),
        }
    }
}

/// O que o episódio acumulou até fechar — vai no resumo da notificação de
/// resolução ("oscilou 7 vezes; estável há 5 min").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EpisodeSummary {
    /// Quantas recaídas (problema de volta dentro da janela) desde a abertura.
    pub recurrence: u64,
    /// Segundos estáveis entre o último problema e a resolução.
    pub stable_for_seconds: i64,
    /// O episódio chegou a ser declarado oscilante (Fase 3).
    pub flapped: bool,
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
    /// Problema de volta dentro da janela: o evento volta a `status` (`Active`,
    /// `Silenced` se o silêncio do operador ainda vigora, ou `Flapping` se já
    /// estava oscilando) e a contagem de estabilidade reinicia — sem evento
    /// novo e sem notificação.
    Relapse {
        recurrence: u64,
        status: AlertStatus,
    },
    /// A recaída fez a contagem deslizante alcançar `flap_threshold`: o alvo é
    /// declarado cronicamente instável. É a **única** transição de flapping que
    /// notifica — daí em diante as recaídas voltam a ser silenciosas.
    StartFlapping { recurrence: u64, transitions: u32 },
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
    /// Parâmetros da regra do evento; tudo zerado quando a regra sumiu — sem
    /// regra não há janela nem limiar a respeitar.
    pub policy: EpisodePolicy,
    /// A condição da regra bateu nesta avaliação.
    pub condition_matched: bool,
    /// O alvo respondeu, mas degradado (`warning`: perda parcial de pacotes,
    /// DNS parcial). Não dispara regra nenhuma — o evaluator manda no disparo —
    /// mas conta como problema para a janela: carimba `lastProblemAt` e, se o
    /// evento estava `recovering`, vira recaída (Fase 2 do roadmap).
    pub degraded: bool,
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
    // Um problema é a condição da regra batendo **ou** a degradação da Fase 2
    // (`warning`); os dois juntos descrevem o mesmo fato e seguem o mesmo ramo.
    if input.condition_matched || input.degraded {
        if !input.status.is_open() {
            return Transition::None;
        }
        // Recaída só existe sobre um episódio que já observava estabilidade
        // (`recovering`) ou já foi declarado oscilante (`flapping`). Num evento
        // ainda em falha o problema apenas continua — sem contador, sem
        // mudança de status.
        return match input.status {
            AlertStatus::Recovering | AlertStatus::Flapping => relapse(input),
            _ => Transition::ProblemOngoing,
        };
    }

    if !input.recovered || !input.status.is_open() {
        return Transition::None;
    }

    let window = input.policy.recovery_window_seconds.max(0);
    match input.status {
        // Sair do flapping exige as duas coisas: silêncio de problemas por toda
        // a janela de estabilidade **e** a contagem deslizante já decaída
        // abaixo do limiar. Sem a segunda, um alvo cronicamente instável
        // fecharia e reabriria o aviso a cada respiro — exatamente a fadiga de
        // notificações que a Fase 3 existe para evitar.
        AlertStatus::Flapping => {
            if stable_seconds(input) >= window && !over_flap_threshold(input) {
                Transition::Resolve {
                    summary: summary_of(input),
                }
            } else {
                Transition::None
            }
        }
        // Janela 0 é o comportamento original: a primeira checagem ok resolve.
        _ if window == 0 => Transition::Resolve {
            summary: summary_of(input),
        },
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

/// O problema voltou sobre um episódio que já não estava em falha.
///
/// É aqui que a Fase 3 decide se esta recaída é só mais uma — silenciosa, como
/// desde a Fase 1 — ou a que transborda o limiar e declara o alvo oscilante.
fn relapse(input: &EpisodeInput) -> Transition {
    let recurrence = recurrence_count(input.data) + 1;

    // O silêncio pedido pelo operador vence tudo: nem a declaração de flapping
    // pode furar o prazo, porque ela notifica.
    if input.silenced_now {
        return Transition::Relapse {
            recurrence,
            status: AlertStatus::Silenced,
        };
    }

    // `+ 1` porque esta recaída ainda não foi carimbada na linha do tempo —
    // quem persiste chama `record_transition` depois de decidir.
    let transitions = transitions_in_window(input) + 1;
    if input.status != AlertStatus::Flapping
        && input.policy.flap_enabled()
        && transitions >= input.policy.flap_threshold
    {
        return Transition::StartFlapping {
            recurrence,
            transitions,
        };
    }

    Transition::Relapse {
        recurrence,
        status: if input.status == AlertStatus::Flapping {
            AlertStatus::Flapping
        } else {
            AlertStatus::Active
        },
    }
}

/// Acrescenta o carimbo desta recaída à linha do tempo do episódio.
///
/// Poda o que já saiu da janela de flap (o que envelheceu não muda decisão
/// nenhuma) e limita o tamanho a [`MAX_TIMELINE`]. Sem detecção configurada
/// não grava nada: `data` não carrega o que ninguém lê.
pub fn record_transition(data: &mut Map<String, Value>, now: DateTime<Utc>, policy: EpisodePolicy) {
    if !policy.flap_enabled() {
        return;
    }
    let cutoff = now - Duration::seconds(policy.flap_window_seconds);
    let mut stamps: Vec<DateTime<Utc>> = timeline(Some(&*data))
        .into_iter()
        .filter(|at| *at >= cutoff)
        .collect();
    stamps.push(now);
    if stamps.len() > MAX_TIMELINE {
        stamps.drain(..stamps.len() - MAX_TIMELINE);
    }
    data.insert(
        PROBLEM_TIMELINE.into(),
        Value::Array(
            stamps
                .into_iter()
                .map(|at| Value::String(at.to_rfc3339()))
                .collect(),
        ),
    );
}

/// Carimbos de recaída legíveis em `data.problemTimeline`; lixo é ignorado.
#[must_use]
pub fn timeline(data: Option<&Map<String, Value>>) -> Vec<DateTime<Utc>> {
    data.and_then(|map| map.get(PROBLEM_TIMELINE))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .filter_map(|raw| DateTime::parse_from_rfc3339(raw).ok())
                .map(|at| at.with_timezone(&Utc))
                .collect()
        })
        .unwrap_or_default()
}

/// Quantas recaídas carimbadas ainda estão dentro da janela de flap.
fn transitions_in_window(input: &EpisodeInput) -> u32 {
    if !input.policy.flap_enabled() {
        return 0;
    }
    let cutoff = input.now - Duration::seconds(input.policy.flap_window_seconds);
    u32::try_from(
        timeline(input.data)
            .into_iter()
            .filter(|at| *at >= cutoff)
            .count(),
    )
    .unwrap_or(u32::MAX)
}

fn over_flap_threshold(input: &EpisodeInput) -> bool {
    input.policy.flap_enabled() && transitions_in_window(input) >= input.policy.flap_threshold
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
        // `flappingSince` fica gravado depois de a declaração acontecer: mesmo
        // que o episódio saia do estado, a resolução ainda sabe que ele oscilou.
        flapped: input.status == AlertStatus::Flapping
            || input
                .data
                .is_some_and(|map| map.contains_key(FLAPPING_SINCE)),
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
            policy: EpisodePolicy {
                recovery_window_seconds: window,
                ..EpisodePolicy::default()
            },
            condition_matched: false,
            degraded: false,
            recovered: true,
            silenced_now: false,
            now: t0(),
        }
    }

    /// Entrada com detecção de flapping ligada: 3 recaídas em 15 min.
    fn entrada_com_flap<'a>(
        status: AlertStatus,
        data: Option<&'a Map<String, Value>>,
    ) -> EpisodeInput<'a> {
        let mut input = entrada(status, data, 300);
        input.policy.flap_threshold = 3;
        input.policy.flap_window_seconds = 900;
        input
    }

    /// Carimbos de recaída em `problemTimeline`, N segundos antes de `t0`.
    fn carimbos(segundos_atras: &[i64]) -> Value {
        json!(segundos_atras
            .iter()
            .map(|seconds| (t0() - Duration::seconds(*seconds)).to_rfc3339())
            .collect::<Vec<_>>())
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
                status: AlertStatus::Active
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
                status: AlertStatus::Silenced
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
                status: AlertStatus::Active
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

    // --- Warning (degradação sem a condição bater) ---------------------------

    #[test]
    fn warning_em_recovering_e_recaida_mesmo_com_a_condicao_limpa() {
        let data = dados(&[
            (RECURRENCE_COUNT, json!(2)),
            (LAST_PROBLEM_AT, json!("2026-08-15T11:59:00Z")),
        ]);
        let mut input = entrada(AlertStatus::Recovering, Some(&data), 300);
        input.recovered = false;
        input.degraded = true;
        assert_eq!(
            decide(&input),
            Transition::Relapse {
                recurrence: 3,
                status: AlertStatus::Active
            }
        );
    }

    #[test]
    fn warning_em_evento_aberto_so_carimba_o_ultimo_problema() {
        for status in [
            AlertStatus::Active,
            AlertStatus::Acknowledged,
            AlertStatus::Silenced,
        ] {
            let mut input = entrada(status, None, 300);
            input.recovered = false;
            input.degraded = true;
            assert_eq!(decide(&input), Transition::ProblemOngoing);
        }
    }

    #[test]
    fn warning_com_janela_vencida_impede_a_resolucao_e_conta_recaida() {
        // O último problema é antigo o bastante para resolver — mas esta
        // avaliação veio degradada: o relógio reinicia em vez de fechar.
        let data = dados(&[(LAST_PROBLEM_AT, json!("2026-08-15T11:50:00Z"))]);
        let mut input = entrada(AlertStatus::Recovering, Some(&data), 300);
        input.recovered = false;
        input.degraded = true;
        assert_eq!(
            decide(&input),
            Transition::Relapse {
                recurrence: 1,
                status: AlertStatus::Active
            }
        );
    }

    #[test]
    fn warning_respeita_o_silencio_vigente_e_nao_move_evento_fechado() {
        let mut input = entrada(AlertStatus::Recovering, None, 300);
        input.recovered = false;
        input.degraded = true;
        input.silenced_now = true;
        assert_eq!(
            decide(&input),
            Transition::Relapse {
                recurrence: 1,
                status: AlertStatus::Silenced
            }
        );

        let mut input = entrada(AlertStatus::Resolved, None, 300);
        input.recovered = false;
        input.degraded = true;
        assert_eq!(decide(&input), Transition::None);
    }

    #[test]
    fn condicao_batendo_prevalece_sobre_a_degradacao() {
        // Os dois sinais juntos descrevem o mesmo problema; a condição é a
        // leitura mais forte e define o ramo.
        let mut input = entrada(AlertStatus::Recovering, None, 300);
        input.condition_matched = true;
        input.degraded = true;
        input.recovered = false;
        assert_eq!(
            decide(&input),
            Transition::Relapse {
                recurrence: 1,
                status: AlertStatus::Active
            }
        );
    }

    // --- Flapping (Fase 3) ---------------------------------------------------

    #[test]
    fn a_recaida_que_alcanca_o_limiar_declara_o_alvo_oscilando() {
        // Duas recaídas carimbadas dentro da janela; esta é a terceira.
        let data = dados(&[
            (RECURRENCE_COUNT, json!(2)),
            (PROBLEM_TIMELINE, carimbos(&[600, 300])),
        ]);
        let mut input = entrada_com_flap(AlertStatus::Recovering, Some(&data));
        input.condition_matched = true;
        input.recovered = false;
        assert_eq!(
            decide(&input),
            Transition::StartFlapping {
                recurrence: 3,
                transitions: 3
            }
        );
    }

    #[test]
    fn abaixo_do_limiar_a_recaida_segue_silenciosa() {
        let data = dados(&[(PROBLEM_TIMELINE, carimbos(&[300]))]);
        let mut input = entrada_com_flap(AlertStatus::Recovering, Some(&data));
        input.condition_matched = true;
        input.recovered = false;
        assert_eq!(
            decide(&input),
            Transition::Relapse {
                recurrence: 1,
                status: AlertStatus::Active
            }
        );
    }

    #[test]
    fn carimbos_fora_da_janela_nao_contam() {
        // Três recaídas, mas duas com mais de 15 min: só a recente conta.
        let data = dados(&[(PROBLEM_TIMELINE, carimbos(&[3600, 1800, 300]))]);
        let mut input = entrada_com_flap(AlertStatus::Recovering, Some(&data));
        input.condition_matched = true;
        input.recovered = false;
        assert_eq!(
            decide(&input),
            Transition::Relapse {
                recurrence: 1,
                status: AlertStatus::Active
            }
        );
    }

    #[test]
    fn recaida_em_flapping_permanece_flapping_e_nao_redeclara() {
        // Já oscilando: a declaração é única, por mais que as recaídas somem.
        let data = dados(&[
            (RECURRENCE_COUNT, json!(9)),
            (PROBLEM_TIMELINE, carimbos(&[600, 400, 200])),
        ]);
        let mut input = entrada_com_flap(AlertStatus::Flapping, Some(&data));
        input.condition_matched = true;
        input.recovered = false;
        assert_eq!(
            decide(&input),
            Transition::Relapse {
                recurrence: 10,
                status: AlertStatus::Flapping
            }
        );
    }

    #[test]
    fn o_silencio_do_operador_impede_a_declaracao_de_flapping() {
        // A declaração notifica; o silêncio pedido vale mais que ela.
        let data = dados(&[(PROBLEM_TIMELINE, carimbos(&[600, 300]))]);
        let mut input = entrada_com_flap(AlertStatus::Recovering, Some(&data));
        input.condition_matched = true;
        input.recovered = false;
        input.silenced_now = true;
        assert_eq!(
            decide(&input),
            Transition::Relapse {
                recurrence: 1,
                status: AlertStatus::Silenced
            }
        );
    }

    #[test]
    fn sem_limiar_ou_sem_janela_nao_ha_deteccao() {
        let data = dados(&[(PROBLEM_TIMELINE, carimbos(&[600, 400, 200, 100]))]);
        for (threshold, window) in [(0, 900), (3, 0), (0, 0)] {
            let mut input = entrada(AlertStatus::Recovering, Some(&data), 300);
            input.policy.flap_threshold = threshold;
            input.policy.flap_window_seconds = window;
            input.condition_matched = true;
            input.recovered = false;
            assert_eq!(
                decide(&input),
                Transition::Relapse {
                    recurrence: 1,
                    status: AlertStatus::Active
                },
                "limiar {threshold} e janela {window} desligam a detecção"
            );
        }
    }

    #[test]
    fn flapping_com_a_contagem_ainda_alta_nao_resolve() {
        // Estável há tempo suficiente, mas a janela deslizante ainda acusa 3
        // recaídas: o alvo continua cronicamente instável.
        let data = dados(&[
            (
                LAST_PROBLEM_AT,
                json!((t0() - Duration::seconds(400)).to_rfc3339()),
            ),
            (FLAPPING_SINCE, json!("2026-08-15T11:40:00Z")),
            (PROBLEM_TIMELINE, carimbos(&[800, 600, 400])),
        ]);
        let input = entrada_com_flap(AlertStatus::Flapping, Some(&data));
        assert_eq!(decide(&input), Transition::None);
    }

    #[test]
    fn flapping_resolve_quando_a_contagem_decai_e_a_estabilidade_volta() {
        // Os carimbos envelheceram para fora da janela de 15 min e o último
        // problema é mais antigo que a janela de estabilidade.
        let data = dados(&[
            (RECURRENCE_COUNT, json!(8)),
            (
                LAST_PROBLEM_AT,
                json!((t0() - Duration::seconds(1000)).to_rfc3339()),
            ),
            (FLAPPING_SINCE, json!("2026-08-15T11:40:00Z")),
            (PROBLEM_TIMELINE, carimbos(&[3600, 2400, 1000])),
        ]);
        let input = entrada_com_flap(AlertStatus::Flapping, Some(&data));
        let Transition::Resolve { summary } = decide(&input) else {
            panic!("contagem decaída + estabilidade fecham o episódio");
        };
        assert_eq!(summary.recurrence, 8);
        assert_eq!(summary.stable_for_seconds, 1000);
        assert!(summary.flapped, "a resolução sabe que o episódio oscilou");
    }

    #[test]
    fn flapping_sem_estabilidade_suficiente_nao_resolve() {
        // Contagem já decaída, mas o último problema é recente demais.
        let data = dados(&[
            (
                LAST_PROBLEM_AT,
                json!((t0() - Duration::seconds(60)).to_rfc3339()),
            ),
            (PROBLEM_TIMELINE, carimbos(&[3600])),
        ]);
        let input = entrada_com_flap(AlertStatus::Flapping, Some(&data));
        assert_eq!(decide(&input), Transition::None);
    }

    #[test]
    fn episodio_que_oscilou_carrega_a_marca_ate_a_resolucao() {
        // Saiu do flapping por outro caminho, mas `flappingSince` permanece: a
        // notificação final ainda conta a história inteira.
        let data = dados(&[
            (LAST_PROBLEM_AT, json!("2026-08-15T11:50:00Z")),
            (FLAPPING_SINCE, json!("2026-08-15T11:40:00Z")),
        ]);
        let input = entrada(AlertStatus::Recovering, Some(&data), 300);
        let Transition::Resolve { summary } = decide(&input) else {
            panic!("janela vencida resolve");
        };
        assert!(summary.flapped);
    }

    #[test]
    fn a_linha_do_tempo_poda_o_que_envelheceu_e_respeita_o_teto() {
        let policy = EpisodePolicy {
            recovery_window_seconds: 300,
            flap_threshold: 3,
            flap_window_seconds: 900,
        };
        let mut data = dados(&[(PROBLEM_TIMELINE, carimbos(&[3600, 600]))]);
        record_transition(&mut data, t0(), policy);
        let stamps = timeline(Some(&data));
        assert_eq!(stamps.len(), 2, "o carimbo de 1 h atrás saiu da janela");
        assert_eq!(stamps[1], t0());

        // Teto: nada além de MAX_TIMELINE fica guardado.
        let mut cheia = Map::new();
        for step in 0..(MAX_TIMELINE + 10) {
            record_transition(&mut cheia, t0() + Duration::seconds(step as i64), policy);
        }
        assert_eq!(timeline(Some(&cheia)).len(), MAX_TIMELINE);
    }

    #[test]
    fn sem_deteccao_configurada_a_linha_do_tempo_nao_e_gravada() {
        let mut data = Map::new();
        record_transition(&mut data, t0(), EpisodePolicy::default());
        assert!(!data.contains_key(PROBLEM_TIMELINE));
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
