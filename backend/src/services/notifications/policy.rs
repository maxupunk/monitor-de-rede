//! Política de notificação (Fase 4 do roadmap de alertas inteligentes, §2.3 da
//! análise).
//!
//! Até a Fase 3 o motor **decidia e entregava** no mesmo gesto: quem abria o
//! evento chamava o `NotificationService` na linha seguinte. Isso já não serve
//! quando a pergunta deixa de ser "houve transição?" e passa a ser "esta
//! mensagem ainda merece o canal?".
//!
//! Aqui mora a resposta, e ela é **pura**: dado o tipo da mensagem, a política
//! da regra, o que já foi entregue para aquele par (regra, alvo) e o que já foi
//! entregue para o grupo correlato, sai uma [`Decision`]. Nada de banco, nada de
//! rede — o chamador injeta os fatos e o relógio, como na
//! [`super::super::alerts::state_machine`].
//!
//! As quatro perguntas que a política responde, na ordem em que importam:
//!
//! 1. **O operador pediu silêncio?** Então nem a boa notícia sai. É a correção
//!    do defeito antigo em que um alerta silenciado ainda notificava o ✅.
//! 2. **A volta ao normal foi de algo que ninguém soube que caiu?** Um ✅ sem
//!    🚨 correspondente é ruído puro — e é exatamente o que sobra quando o
//!    cooldown ou a inibição engolem o disparo.
//! 3. **Já falei disto agora há pouco?** O cooldown por (regra, alvo) cobre o
//!    caso que a janela de estabilização não cobre: o episódio que fecha e um
//!    novo que abre minutos depois.
//! 4. **Está tudo caindo junto?** Então uma mensagem só para o grupo, em vez de
//!    uma por alvo.
//!
//! A **inibição por dependência** não é decidida aqui de propósito: ela depende
//! de o pai já ter sido detectado, e o pai costuma cair *depois* do filho na
//! ordem de checagem. Por isso a mensagem inibível espera
//! [`INHIBITION_GRACE_SECONDS`] na fila e é julgada na entrega, por
//! [`super::outbox`].

use std::{fmt, str::FromStr};

use chrono::{DateTime, Duration, Utc};

use super::contracts::Severity;

/// Espera padrão antes da primeira mensagem de um grupo ocioso, em segundos.
///
/// É o `group_wait` do Alertmanager, com o mesmo valor: meio minuto é o
/// suficiente para uma rajada se formar e pouco o bastante para ninguém sentir.
/// Severidade crítica não paga esta espera (ver [`decide`]).
pub const DEFAULT_DIGEST_WAIT_SECONDS: i64 = 30;

/// Intervalo mínimo entre mensagens do mesmo grupo, em segundos.
///
/// É o `group_interval` do Alertmanager. Tudo que chega dentro dele é
/// consolidado na mensagem seguinte — "8 alertas no site Matriz" em vez de oito
/// mensagens. `0` desliga o agrupamento e devolve a entrega imediata.
pub const DEFAULT_DIGEST_WINDOW_SECONDS: i64 = 300;

/// Quanto uma mensagem inibível espera na fila antes de a inibição ser julgada.
///
/// O dispositivo filho quase sempre é detectado antes do pai — intervalos
/// diferentes, ordem de execução diferente. Julgar a inibição no enfileiramento
/// perderia essa corrida em quase todo caso real, e a enxurrada que a inibição
/// existe para conter sairia inteira.
pub const INHIBITION_GRACE_SECONDS: i64 = 120;

/// O que a mensagem diz. Persistido em `notification_outbox.kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationKind {
    /// 🚨 O alerta abriu.
    Problem,
    /// ⚠️ O alvo foi declarado cronicamente instável (Fase 3).
    Flapping,
    /// ✅ O episódio fechou.
    Resolved,
}

impl NotificationKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Problem => "problem",
            Self::Flapping => "flapping",
            Self::Resolved => "resolved",
        }
    }

    /// As duas mensagens que **abrem** conversa: depois de qualquer uma delas,
    /// há o que normalizar.
    #[must_use]
    pub const fn announces_problem(self) -> bool {
        matches!(self, Self::Problem | Self::Flapping)
    }
}

impl fmt::Display for NotificationKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for NotificationKind {
    type Err = ();

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        match raw {
            "problem" => Ok(Self::Problem),
            "flapping" => Ok(Self::Flapping),
            "resolved" => Ok(Self::Resolved),
            _ => Err(()),
        }
    }
}

/// Por que a mensagem não foi entregue. Vai para
/// `notification_outbox.suppress_reason` — é o que responde "por que não fui
/// avisado?" sem ninguém precisar adivinhar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuppressReason {
    /// O operador pediu silêncio e o prazo ainda vigora.
    Silenced,
    /// Ninguém foi avisado do problema: não há o que normalizar.
    Unannounced,
    /// Já houve notificação de problema deste par (regra, alvo) há pouco.
    Cooldown,
    /// O pai do dispositivo está em alerta e explica a queda do filho.
    Inhibited,
    /// Há uma janela de manutenção agendada para o site ou dispositivo.
    Maintenance,
}

impl SuppressReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Silenced => "silenced",
            Self::Unannounced => "unannounced",
            Self::Cooldown => "cooldown",
            Self::Inhibited => "inhibited",
            Self::Maintenance => "maintenance",
        }
    }
}

impl fmt::Display for SuppressReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// O que fazer com a mensagem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Entrega na próxima passagem do despachante.
    Deliver,
    /// Represada até `after`, para que a rajada do grupo saia numa mensagem só.
    Digest { after: DateTime<Utc> },
    /// Engolida, com o motivo registrado.
    Suppress(SuppressReason),
}

/// Os parâmetros de notificação que vêm da regra (§3.4 do roadmap: cada
/// comportamento novo é parâmetro de regra, ajustável na tela que o usuário já
/// conhece).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NotificationPolicy {
    /// Intervalo mínimo entre notificações de problema do par (regra, alvo).
    /// `0` desliga.
    pub cooldown_seconds: i64,
    /// Suprimir quando o pai declarado do dispositivo está em alerta.
    pub inhibit_when_parent_down: bool,
}

impl From<&crate::models::alert_rules::Model> for NotificationPolicy {
    fn from(rule: &crate::models::alert_rules::Model) -> Self {
        Self {
            cooldown_seconds: i64::from(rule.notification_cooldown_seconds),
            inhibit_when_parent_down: rule.inhibit_when_parent_down,
        }
    }
}

/// O agrupamento é **global**, e não parâmetro de regra, porque a pergunta que
/// ele responde atravessa as regras: "quantas mensagens o operador recebe nesta
/// janela?". Oito alertas num site vêm de regras diferentes — pendurar a janela
/// em uma delas seria arbitrário. Fica junto da retenção do `data_pruner`, que é
/// a outra configuração de infraestrutura do mesmo tipo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DigestPolicy {
    pub wait_seconds: i64,
    pub window_seconds: i64,
}

impl Default for DigestPolicy {
    fn default() -> Self {
        Self {
            wait_seconds: DEFAULT_DIGEST_WAIT_SECONDS,
            window_seconds: DEFAULT_DIGEST_WINDOW_SECONDS,
        }
    }
}

impl DigestPolicy {
    /// Lê `NOTIFICATION_DIGEST_WAIT_SECONDS` e
    /// `NOTIFICATION_DIGEST_WINDOW_SECONDS`.
    ///
    /// Valor ausente ou ilegível cai no padrão; negativo vira `0`. Janela `0` é
    /// legítima e desliga o agrupamento — ao contrário da retenção do
    /// `data_pruner`, aqui desligar não enche disco nenhum, só devolve o
    /// comportamento anterior.
    #[must_use]
    pub fn from_env() -> Self {
        let read = |variable: &str, default: i64| {
            std::env::var(variable)
                .ok()
                .and_then(|value| value.trim().parse::<i64>().ok())
                .map_or(default, |value| value.max(0))
        };
        Self {
            wait_seconds: read(
                "NOTIFICATION_DIGEST_WAIT_SECONDS",
                DEFAULT_DIGEST_WAIT_SECONDS,
            ),
            window_seconds: read(
                "NOTIFICATION_DIGEST_WINDOW_SECONDS",
                DEFAULT_DIGEST_WINDOW_SECONDS,
            ),
        }
    }

    #[must_use]
    pub const fn enabled(self) -> bool {
        self.window_seconds > 0
    }
}

/// O que já foi entregue para o par (regra, alvo).
///
/// Dois fatos bastam, e é de propósito: o cooldown mede do último **problema**
/// (a resolução não empurra o relógio, ela apenas fecha o par), e o ✅ só sai se
/// houve 🚨 antes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DeliveryLog {
    /// Quando saiu a última notificação de problema (disparo ou oscilação).
    pub last_problem_at: Option<DateTime<Utc>>,
    /// A última mensagem entregue foi de problema — logo, há o que normalizar.
    /// `false` quando nada saiu ou quando a última foi a própria resolução.
    pub announced: bool,
}

/// Tudo que a política precisa saber; nada que ela precise buscar.
#[derive(Debug, Clone, Copy)]
pub struct DeliveryInput {
    pub kind: NotificationKind,
    pub severity: Severity,
    pub policy: NotificationPolicy,
    pub digest: DigestPolicy,
    pub log: DeliveryLog,
    /// Última entrega de **qualquer** mensagem do grupo correlato.
    pub group_last_sent_at: Option<DateTime<Utc>>,
    /// Há silêncio vigente pedido pelo operador para este alerta.
    pub silenced: bool,
    /// Há janela de manutenção vigente para o site ou dispositivo.
    pub under_maintenance: bool,
    pub now: DateTime<Utc>,
}

/// Decide o destino de uma notificação. Pura: mesma entrada, mesma saída.
#[must_use]
pub fn decide(input: &DeliveryInput) -> Decision {
    // 1. Janela de manutenção vence tudo: o operador agendou o silêncio antes
    //    do incidente acontecer.
    if input.under_maintenance {
        return Decision::Suppress(SuppressReason::Maintenance);
    }

    // 2. Silêncio pedido pelo operador vence o resto, inclusive a boa notícia:
    //    quem silenciou não quer saber nem que voltou.
    if input.silenced {
        return Decision::Suppress(SuppressReason::Silenced);
    }

    // 2. Resolução de algo que nunca foi anunciado é ruído: o operador
    //    receberia um ✅ de um 🚨 que a política engoliu.
    if input.kind == NotificationKind::Resolved && !input.log.announced {
        return Decision::Suppress(SuppressReason::Unannounced);
    }

    // 3. Cooldown. Só o disparo paga: a declaração de oscilação é a **única**
    //    mensagem que explica por que o canal vai ficar quieto, e engoli-la
    //    deixaria o silêncio sem explicação. A resolução também não paga —
    //    ela fecha um par que já foi aberto.
    if input.kind == NotificationKind::Problem && within_cooldown(input) {
        return Decision::Suppress(SuppressReason::Cooldown);
    }

    // 4. Agrupamento.
    match digest_deadline(input) {
        Some(after) if after > input.now => Decision::Digest { after },
        _ => Decision::Deliver,
    }
}

fn within_cooldown(input: &DeliveryInput) -> bool {
    let cooldown = input.policy.cooldown_seconds;
    if cooldown <= 0 {
        return false;
    }
    input
        .log
        .last_problem_at
        .is_some_and(|at| input.now - at < Duration::seconds(cooldown))
}

/// Quando esta mensagem pode sair, considerando o que o grupo já recebeu.
///
/// `None` = agora. A severidade crítica dispensa a **espera** do grupo ocioso
/// (o primeiro crítico sai na hora), mas continua respeitando a **janela**: uma
/// cascata de 200 críticos vira uma mensagem imediata e uma consolidada, não
/// 200 mensagens.
fn digest_deadline(input: &DeliveryInput) -> Option<DateTime<Utc>> {
    if !input.digest.enabled() {
        return None;
    }
    let wait = if input.severity == Severity::Critical {
        0
    } else {
        input.digest.wait_seconds
    };
    let window = Duration::seconds(input.digest.window_seconds);
    match input.group_last_sent_at {
        Some(at) if input.now - at < window => Some(at + window),
        _ => Some(input.now + Duration::seconds(wait)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn t0() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-08-15T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    /// Entrada base: disparo comum, sem cooldown e sem agrupamento.
    fn entrada(kind: NotificationKind) -> DeliveryInput {
        DeliveryInput {
            kind,
            severity: Severity::Warning,
            policy: NotificationPolicy::default(),
            digest: DigestPolicy {
                wait_seconds: 0,
                window_seconds: 0,
            },
            log: DeliveryLog::default(),
            group_last_sent_at: None,
            silenced: false,
            under_maintenance: false,
            now: t0(),
        }
    }

    fn regra(cooldown: i32, inhibit: bool) -> crate::models::alert_rules::Model {
        let now = Utc::now().into();
        crate::models::alert_rules::Model {
            id: 1,
            site_id: None,
            device_id: None,
            monitor_id: None,
            name: "Regra".into(),
            r#type: "custom".into(),
            template_key: None,
            condition: json!({ "field": "status", "operator": "eq", "value": "down" }),
            severity: "warning".into(),
            duration_seconds: 0,
            recovery_window_seconds: 0,
            flap_threshold: 0,
            flap_window_seconds: 900,
            notification_cooldown_seconds: cooldown,
            inhibit_when_parent_down: inhibit,
            enabled: true,
            created_at: now,
            updated_at: now,
        }
    }

    // --- Silêncio ------------------------------------------------------------

    #[test]
    fn silencio_do_operador_engole_ate_a_resolucao() {
        for kind in [
            NotificationKind::Problem,
            NotificationKind::Flapping,
            NotificationKind::Resolved,
        ] {
            let mut input = entrada(kind);
            input.silenced = true;
            input.log.announced = true;
            assert_eq!(
                decide(&input),
                Decision::Suppress(SuppressReason::Silenced),
                "{kind} furou o silêncio"
            );
        }
    }

    #[test]
    fn janela_de_manutencao_suprime_todos_os_tipos() {
        for kind in [
            NotificationKind::Problem,
            NotificationKind::Flapping,
            NotificationKind::Resolved,
        ] {
            let mut input = entrada(kind);
            input.under_maintenance = true;
            input.log.announced = true;
            assert_eq!(
                decide(&input),
                Decision::Suppress(SuppressReason::Maintenance),
                "{kind} furou a manutenção"
            );
        }
    }

    // --- Pareamento 🚨/✅ -----------------------------------------------------

    #[test]
    fn resolucao_sem_disparo_anunciado_e_suprimida() {
        let input = entrada(NotificationKind::Resolved);
        assert_eq!(
            decide(&input),
            Decision::Suppress(SuppressReason::Unannounced)
        );
    }

    #[test]
    fn resolucao_de_problema_anunciado_sai_mesmo_dentro_do_cooldown() {
        // O cooldown vale para o 🚨; o ✅ fecha um par que já foi aberto.
        let mut input = entrada(NotificationKind::Resolved);
        input.policy.cooldown_seconds = 900;
        input.log = DeliveryLog {
            last_problem_at: Some(t0() - Duration::seconds(30)),
            announced: true,
        };
        assert_eq!(decide(&input), Decision::Deliver);
    }

    // --- Cooldown ------------------------------------------------------------

    #[test]
    fn disparo_dentro_do_cooldown_e_suprimido() {
        let mut input = entrada(NotificationKind::Problem);
        input.policy.cooldown_seconds = 900;
        input.log.last_problem_at = Some(t0() - Duration::seconds(899));
        assert_eq!(decide(&input), Decision::Suppress(SuppressReason::Cooldown));

        // Vencido o intervalo, o alvo volta a ter voz.
        input.log.last_problem_at = Some(t0() - Duration::seconds(900));
        assert_eq!(decide(&input), Decision::Deliver);
    }

    #[test]
    fn sem_cooldown_configurado_todo_disparo_passa() {
        let mut input = entrada(NotificationKind::Problem);
        input.log.last_problem_at = Some(t0() - Duration::seconds(1));
        assert_eq!(decide(&input), Decision::Deliver);
    }

    #[test]
    fn o_aviso_de_oscilacao_nao_paga_cooldown() {
        // É a única mensagem que explica por que o canal vai ficar quieto —
        // engoli-la deixaria o silêncio sem explicação (critério de aceite da
        // Fase 3: 1 problema + 1 oscilação + 1 resolução).
        let mut input = entrada(NotificationKind::Flapping);
        input.policy.cooldown_seconds = 900;
        input.log.last_problem_at = Some(t0() - Duration::seconds(10));
        assert_eq!(decide(&input), Decision::Deliver);
    }

    // --- Agrupamento ---------------------------------------------------------

    #[test]
    fn grupo_ocioso_espera_a_janela_curta_antes_da_primeira_mensagem() {
        let mut input = entrada(NotificationKind::Problem);
        input.digest = DigestPolicy::default();
        assert_eq!(
            decide(&input),
            Decision::Digest {
                after: t0() + Duration::seconds(DEFAULT_DIGEST_WAIT_SECONDS)
            }
        );
    }

    #[test]
    fn severidade_critica_nao_paga_a_espera_do_grupo_ocioso() {
        let mut input = entrada(NotificationKind::Problem);
        input.digest = DigestPolicy::default();
        input.severity = Severity::Critical;
        assert_eq!(decide(&input), Decision::Deliver);
    }

    #[test]
    fn mensagem_dentro_da_janela_do_grupo_espera_a_consolidacao() {
        let mut input = entrada(NotificationKind::Problem);
        input.digest = DigestPolicy::default();
        input.group_last_sent_at = Some(t0() - Duration::seconds(60));
        let esperado =
            t0() - Duration::seconds(60) + Duration::seconds(DEFAULT_DIGEST_WINDOW_SECONDS);
        assert_eq!(decide(&input), Decision::Digest { after: esperado });

        // O crítico também espera a janela: o que ele dispensa é só a espera do
        // grupo ocioso. Uma cascata vira 1 imediata + 1 consolidada.
        input.severity = Severity::Critical;
        assert_eq!(decide(&input), Decision::Digest { after: esperado });
    }

    #[test]
    fn grupo_com_a_janela_vencida_recomeca_do_zero() {
        let mut input = entrada(NotificationKind::Problem);
        input.digest = DigestPolicy::default();
        input.group_last_sent_at = Some(t0() - Duration::seconds(DEFAULT_DIGEST_WINDOW_SECONDS));
        assert_eq!(
            decide(&input),
            Decision::Digest {
                after: t0() + Duration::seconds(DEFAULT_DIGEST_WAIT_SECONDS)
            }
        );
    }

    #[test]
    fn janela_zerada_desliga_o_agrupamento() {
        let mut input = entrada(NotificationKind::Problem);
        input.digest = DigestPolicy {
            wait_seconds: 30,
            window_seconds: 0,
        };
        input.group_last_sent_at = Some(t0() - Duration::seconds(1));
        assert_eq!(decide(&input), Decision::Deliver);
    }

    // --- Leitura da regra e do ambiente --------------------------------------

    #[test]
    fn a_politica_sai_inteira_da_regra() {
        assert_eq!(
            NotificationPolicy::from(&regra(600, true)),
            NotificationPolicy {
                cooldown_seconds: 600,
                inhibit_when_parent_down: true,
            }
        );
        assert_eq!(
            NotificationPolicy::from(&regra(0, false)),
            NotificationPolicy::default()
        );
    }

    #[test]
    #[serial_test::serial]
    fn o_agrupamento_vem_do_ambiente_com_padrao() {
        for variable in [
            "NOTIFICATION_DIGEST_WAIT_SECONDS",
            "NOTIFICATION_DIGEST_WINDOW_SECONDS",
        ] {
            std::env::remove_var(variable);
        }
        assert_eq!(DigestPolicy::from_env(), DigestPolicy::default());

        std::env::set_var("NOTIFICATION_DIGEST_WINDOW_SECONDS", "0");
        assert!(!DigestPolicy::from_env().enabled(), "0 desliga o digest");

        // Negativo vira zero; texto ilegível cai no padrão.
        std::env::set_var("NOTIFICATION_DIGEST_WAIT_SECONDS", "-5");
        assert_eq!(DigestPolicy::from_env().wait_seconds, 0);
        std::env::set_var("NOTIFICATION_DIGEST_WAIT_SECONDS", "sempre");
        assert_eq!(
            DigestPolicy::from_env().wait_seconds,
            DEFAULT_DIGEST_WAIT_SECONDS
        );

        for variable in [
            "NOTIFICATION_DIGEST_WAIT_SECONDS",
            "NOTIFICATION_DIGEST_WINDOW_SECONDS",
        ] {
            std::env::remove_var(variable);
        }
    }

    #[test]
    fn vocabulario_persistido_faz_ida_e_volta() {
        for kind in [
            NotificationKind::Problem,
            NotificationKind::Flapping,
            NotificationKind::Resolved,
        ] {
            assert_eq!(kind.as_str().parse::<NotificationKind>(), Ok(kind));
        }
        assert!("outro".parse::<NotificationKind>().is_err());
        assert!(NotificationKind::Problem.announces_problem());
        assert!(NotificationKind::Flapping.announces_problem());
        assert!(!NotificationKind::Resolved.announces_problem());
        assert_eq!(SuppressReason::Cooldown.to_string(), "cooldown");
    }
}
