//! Casamento de padrões de log contra as regras de alerta.
//!
//! **A regex casa na ingestão; a regra é avaliada no ciclo do scheduler.**
//! A divisão não é arbitrária:
//!
//! - *Casar por consulta* seria uma varredura com regex sobre a janela a cada
//!   ciclo — predicado que nenhum índice cobre. Na ingestão são ~120 regex/s
//!   com 10 regras a 12 msg/s: ruído.
//! - *Avaliar na ingestão* faria cada linha recebida virar uma ida ao banco
//!   pelo motor de alertas, no caminho quente do listener.
//!
//! Assim a parte cara fica onde é barata, e a avaliação acontece onde todo o
//! resto do motor já acontece — herdando de graça a histerese, a detecção de
//! flapping e a higiene de notificação. Uma tempestade de log não vira
//! tempestade de notificação porque o `manager` já sabe disso.
//!
//! **Os contadores vivem em memória e zeram no restart.** É o mesmo desenho da
//! histerese temporal (`alerts::hysteresis`) e pelo mesmo motivo: são janelas
//! de minutos, e persistir cada casamento custaria uma escrita por linha para
//! sobreviver a um reinício que reinicia a janela de qualquer forma.

use std::{
    collections::HashMap,
    sync::{Mutex, PoisonError},
};

use chrono::{DateTime, Duration, Utc};
use loco_rs::app::AppContext;
use regex::Regex;
use serde_json::{Map, Value};

use crate::{
    models::alert_rules,
    services::{
        alerts::{
            contracts::{AlertEvaluationContext, AlertEvaluationScope},
            datasets::log_pattern::{self, LogPatternFacts},
            manager, repository,
        },
        shared::errors::AppResult,
        syslog::parser::ParsedLog,
    },
};

/// Tipo da regra no catálogo. É o que separa as regras deste motor das demais.
pub const RULE_TYPE: &str = "log_pattern";

/// Janela padrão quando a regra não define uma.
pub const DEFAULT_WINDOW_SECONDS: i64 = 300;

/// Teto de casamentos guardados por par (regra, dispositivo).
///
/// Uma tempestade de log poderia guardar milhões de instantes para responder
/// "quantos na janela". Passado o teto, a contagem satura — e saturada ela já
/// disparou qualquer limiar plausível.
const MAX_HITS_PER_KEY: usize = 1_000;

/// A configuração de casamento, lida da `condition` da regra.
///
/// Mora na mesma `condition` que o avaliador lê: o `AlertRuleCondition::from_json`
/// ignora chaves que não conhece, então `pattern` e `windowSeconds` convivem
/// com `field`/`operator`/`value` sem coluna nova no banco.
#[derive(Debug, Clone)]
pub struct PatternRule {
    pub rule_id: i64,
    pub pattern_key: String,
    pub regex: Regex,
    /// Severidade numérica **máxima** que a linha precisa ter para contar.
    /// `None` aceita qualquer uma.
    pub min_severity: Option<i16>,
    pub window_seconds: i64,
    /// Escopo da regra, para não contar log de dispositivo que ela não cobre.
    pub device_id: Option<i64>,
}

impl PatternRule {
    /// Lê a configuração da regra. `None` quando ela não é de log ou está
    /// malformada — regra corrompida no banco nunca derruba a ingestão.
    #[must_use]
    pub fn from_rule(rule: &alert_rules::Model) -> Option<Self> {
        if rule.r#type != RULE_TYPE {
            return None;
        }
        let condition = rule.condition.as_object()?;
        let pattern = condition.get("pattern")?.as_str()?;
        let regex = Regex::new(pattern).ok()?;

        Some(Self {
            rule_id: rule.id,
            pattern_key: rule
                .template_key
                .clone()
                .unwrap_or_else(|| format!("rule_{}", rule.id)),
            regex,
            min_severity: condition
                .get("minSeverity")
                .and_then(Value::as_i64)
                .and_then(|valor| i16::try_from(valor).ok()),
            window_seconds: condition
                .get("windowSeconds")
                .and_then(Value::as_i64)
                .filter(|valor| *valor > 0)
                .unwrap_or(DEFAULT_WINDOW_SECONDS),
            device_id: rule.device_id,
        })
    }

    /// A linha conta para esta regra?
    #[must_use]
    pub fn matches(&self, device_id: Option<i64>, parsed: &ParsedLog) -> bool {
        if let Some(alvo) = self.device_id {
            if device_id != Some(alvo) {
                return false;
            }
        }
        if let Some(minima) = self.min_severity {
            // Linha sem severidade não é descartada por um limiar de
            // severidade: no RouterOS cru ela pode ser justamente a que
            // interessa.
            if parsed.severity.is_some_and(|atual| atual > minima) {
                return false;
            }
        }
        self.regex.is_match(&parsed.message)
    }
}

/// Um casamento observado.
#[derive(Debug, Clone)]
struct Hit {
    at: DateTime<Utc>,
    severity: Option<i16>,
    message: String,
}

/// Chave da janela: a regra e o dispositivo de onde o log veio.
///
/// O dispositivo entra na chave porque o **episódio** é por aparelho: uma regra
/// global vale para todos, mas a contagem de um roteador não pode silenciar o
/// alerta do vizinho.
type WindowKey = (i64, Option<i64>);

/// Janelas deslizantes por `(rule_id, device_id)`.
#[derive(Default)]
pub struct PatternMatcher {
    /// As regras compiladas. Recarregadas pelo ciclo do scheduler.
    rules: Mutex<Vec<PatternRule>>,
    hits: Mutex<HashMap<WindowKey, Vec<Hit>>>,
}

impl PatternMatcher {
    #[must_use]
    pub fn create() -> Self {
        Self::default()
    }

    /// Substitui o conjunto de regras compiladas.
    ///
    /// Compilar a regex uma vez por recarga, e não por linha, é o que torna o
    /// casamento na ingestão barato.
    pub fn set_rules(&self, rules: Vec<PatternRule>) {
        *self.rules.lock().unwrap_or_else(PoisonError::into_inner) = rules;
    }

    #[must_use]
    pub fn rule_count(&self) -> usize {
        self.rules
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    /// Confere uma linha recebida contra as regras. Chamado da ingestão.
    ///
    /// Sai imediatamente quando não há regra de log cadastrada — que é o estado
    /// de quem não usa o recurso, e não pode pagar nada por ele.
    pub fn record(&self, device_id: Option<i64>, parsed: &ParsedLog, agora: DateTime<Utc>) {
        let rules = self.rules.lock().unwrap_or_else(PoisonError::into_inner);
        if rules.is_empty() {
            return;
        }
        let casadas: Vec<i64> = rules
            .iter()
            .filter(|rule| rule.matches(device_id, parsed))
            .map(|rule| rule.rule_id)
            .collect();
        drop(rules);

        if casadas.is_empty() {
            return;
        }
        let mut hits = self.hits.lock().unwrap_or_else(PoisonError::into_inner);
        for rule_id in casadas {
            let lista = hits.entry((rule_id, device_id)).or_default();
            if lista.len() < MAX_HITS_PER_KEY {
                lista.push(Hit {
                    at: agora,
                    severity: parsed.severity,
                    message: parsed.message.clone(),
                });
            }
        }
    }

    /// Descarta o que saiu da janela e devolve o que sobrou por chave.
    fn drain_window(&self, rules: &[PatternRule], agora: DateTime<Utc>) -> Vec<Observation> {
        let mut hits = self.hits.lock().unwrap_or_else(PoisonError::into_inner);
        let mut observacoes = Vec::new();

        for rule in rules {
            let corte = agora - Duration::seconds(rule.window_seconds);
            for ((rule_id, device_id), lista) in hits.iter_mut() {
                if *rule_id != rule.rule_id {
                    continue;
                }
                lista.retain(|hit| hit.at >= corte);
                // Chave sem casamento na janela também vira observação: é ela
                // que leva `recovered` ao motor e permite o alerta resolver.
                observacoes.push(Observation {
                    rule_id: *rule_id,
                    device_id: *device_id,
                    count: i64::try_from(lista.len()).unwrap_or(i64::MAX),
                    severity: lista.iter().filter_map(|hit| hit.severity).min(),
                    last_message: lista
                        .last()
                        .map_or_else(String::new, |hit| hit.message.clone()),
                });
            }
        }

        // Chave vazia de regra que já não existe não precisa ocupar memória.
        hits.retain(|_, lista| !lista.is_empty());
        observacoes
    }
}

/// O que a janela observou para um par (regra, dispositivo).
#[derive(Debug, Clone)]
struct Observation {
    rule_id: i64,
    device_id: Option<i64>,
    count: i64,
    severity: Option<i16>,
    last_message: String,
}

/// Recarrega as regras e avalia as janelas. Chamado do ciclo do scheduler.
///
/// # Errors
///
/// Propaga erro do banco ao ler as regras.
pub async fn evaluate(ctx: &AppContext, matcher: &PatternMatcher) -> AppResult<usize> {
    let compiladas: Vec<PatternRule> = repository::find_all(&ctx.db)
        .await?
        .iter()
        .filter(|rule| rule.enabled)
        .filter_map(PatternRule::from_rule)
        .collect();
    matcher.set_rules(compiladas.clone());

    if compiladas.is_empty() {
        return Ok(0);
    }

    let agora = Utc::now();
    let observacoes = matcher.drain_window(&compiladas, agora);
    let mut avaliadas = 0;

    for observacao in observacoes {
        let Some(rule) = compiladas
            .iter()
            .find(|rule| rule.rule_id == observacao.rule_id)
        else {
            continue;
        };

        let dataset = log_pattern::build(&LogPatternFacts {
            pattern_key: &rule.pattern_key,
            match_count: observacao.count,
            window_seconds: rule.window_seconds,
            severity: observacao.severity,
            last_message: &observacao.last_message,
        });

        // O escopo é o dispositivo da linha, não o da regra: uma regra global
        // vale para todos, mas o **episódio** é por aparelho — senão a queda de
        // um roteador silenciaria o alerta do vizinho.
        let scope = AlertEvaluationScope {
            device_id: observacao.device_id,
            ..AlertEvaluationScope::default()
        };
        let scope_key = format!(
            "log:{}:{}",
            rule.pattern_key,
            observacao
                .device_id
                .map_or_else(|| "global".to_owned(), |id| id.to_string())
        );

        let mut data = Map::new();
        data.insert(
            "logPatternKey".into(),
            Value::String(rule.pattern_key.clone()),
        );
        data.insert("logMatchCount".into(), Value::from(observacao.count));

        manager::evaluate(
            ctx,
            &AlertEvaluationContext {
                scope,
                scope_key,
                target_label: rotulo(observacao.device_id, &rule.pattern_key),
                dataset,
                message: (!observacao.last_message.is_empty())
                    .then(|| observacao.last_message.clone()),
                data,
                // Zero casamentos na janela é a recuperação: o padrão parou de
                // acontecer. Sem isto o alerta nunca resolveria.
                recovered: observacao.count == 0,
                degraded: false,
            },
        )
        .await?;
        avaliadas += 1;
    }

    Ok(avaliadas)
}

fn rotulo(device_id: Option<i64>, pattern_key: &str) -> String {
    device_id.map_or_else(
        || format!("Log ({pattern_key})"),
        |id| format!("Log do dispositivo {id}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn regra(condition: Value, device_id: Option<i64>) -> alert_rules::Model {
        alert_rules::Model {
            id: 1,
            site_id: None,
            device_id,
            monitor_id: None,
            name: "Falha de login".into(),
            r#type: RULE_TYPE.into(),
            template_key: Some("log_login_failure".into()),
            condition,
            severity: "warning".into(),
            duration_seconds: 0,
            recovery_window_seconds: 0,
            flap_threshold: 0,
            flap_window_seconds: 0,
            notification_cooldown_seconds: 0,
            inhibit_when_parent_down: false,
            enabled: true,
            created_at: Utc::now().into(),
            updated_at: Utc::now().into(),
        }
    }

    fn linha(mensagem: &str, severity: Option<i16>) -> ParsedLog {
        ParsedLog {
            severity,
            message: mensagem.into(),
            ..ParsedLog::default()
        }
    }

    #[test]
    fn a_configuracao_convive_com_a_condicao_do_avaliador() {
        // `pattern` e `windowSeconds` moram na mesma `condition` que
        // `field`/`operator`/`value` — sem coluna nova no banco.
        let rule = regra(
            json!({
                "field": "logMatchCount", "operator": "gte", "value": 3,
                "pattern": "login failure", "minSeverity": 4, "windowSeconds": 600
            }),
            None,
        );
        let compilada = PatternRule::from_rule(&rule).expect("compila");
        assert_eq!(compilada.pattern_key, "log_login_failure");
        assert_eq!(compilada.min_severity, Some(4));
        assert_eq!(compilada.window_seconds, 600);
    }

    #[test]
    fn regra_malformada_e_ignorada_em_vez_de_derrubar_a_ingestao() {
        // Regex inválida, padrão ausente, tipo errado: nenhum deles pode
        // impedir a ingestão de seguir.
        assert!(PatternRule::from_rule(&regra(json!({ "pattern": "[" }), None)).is_none());
        assert!(PatternRule::from_rule(&regra(json!({ "field": "x" }), None)).is_none());
        let mut outra = regra(json!({ "pattern": "ok" }), None);
        outra.r#type = "custom".into();
        assert!(PatternRule::from_rule(&outra).is_none());
    }

    #[test]
    fn a_janela_padrao_vale_quando_a_regra_nao_define() {
        let compilada =
            PatternRule::from_rule(&regra(json!({ "pattern": "erro" }), None)).expect("compila");
        assert_eq!(compilada.window_seconds, DEFAULT_WINDOW_SECONDS);
        // Zero e negativo não desligam a janela — cairiam em contagem infinita.
        for invalido in [json!(0), json!(-5)] {
            let compilada = PatternRule::from_rule(&regra(
                json!({ "pattern": "erro", "windowSeconds": invalido }),
                None,
            ))
            .expect("compila");
            assert_eq!(compilada.window_seconds, DEFAULT_WINDOW_SECONDS);
        }
    }

    #[test]
    fn a_regra_com_dispositivo_ignora_log_dos_outros() {
        let compilada =
            PatternRule::from_rule(&regra(json!({ "pattern": "erro" }), Some(7))).expect("compila");
        assert!(compilada.matches(Some(7), &linha("erro grave", None)));
        assert!(!compilada.matches(Some(8), &linha("erro grave", None)));
        assert!(!compilada.matches(None, &linha("erro grave", None)));
    }

    #[test]
    fn a_severidade_minima_corta_o_que_e_menos_grave() {
        let compilada = PatternRule::from_rule(&regra(
            json!({ "pattern": "login", "minSeverity": 4 }),
            None,
        ))
        .expect("compila");
        assert!(
            compilada.matches(None, &linha("login failure", Some(3))),
            "erro"
        );
        assert!(
            compilada.matches(None, &linha("login failure", Some(4))),
            "aviso"
        );
        assert!(
            !compilada.matches(None, &linha("login failure", Some(6))),
            "info"
        );
        // Linha sem severidade não é cortada: no RouterOS cru pode ser
        // justamente a que interessa.
        assert!(compilada.matches(None, &linha("login failure", None)));
    }

    #[test]
    fn a_janela_deslizante_conta_e_esquece() {
        let matcher = PatternMatcher::create();
        let compilada = PatternRule::from_rule(&regra(
            json!({ "pattern": "login failure", "windowSeconds": 300 }),
            None,
        ))
        .expect("compila");
        matcher.set_rules(vec![compilada.clone()]);

        let base = Utc::now();
        for offset in [0, 10, 20] {
            matcher.record(
                Some(7),
                &linha("login failure for admin", Some(3)),
                base + Duration::seconds(offset),
            );
        }
        // Linha que não casa não conta.
        matcher.record(Some(7), &linha("usuário conectado", Some(6)), base);

        let observacoes = matcher.drain_window(
            std::slice::from_ref(&compilada),
            base + Duration::seconds(30),
        );
        let observacao = observacoes
            .iter()
            .find(|o| o.device_id == Some(7))
            .expect("observação");
        assert_eq!(observacao.count, 3);
        assert_eq!(observacao.severity, Some(3));

        // Passada a janela, a contagem zera — e é isso que resolve o alerta.
        let depois = matcher.drain_window(&[compilada], base + Duration::seconds(600));
        let observacao = depois
            .iter()
            .find(|o| o.device_id == Some(7))
            .expect("observação");
        assert_eq!(observacao.count, 0);
    }

    #[test]
    fn sem_regra_cadastrada_a_ingestao_nao_paga_nada() {
        let matcher = PatternMatcher::create();
        assert_eq!(matcher.rule_count(), 0);
        matcher.record(Some(1), &linha("qualquer coisa", Some(3)), Utc::now());
        assert!(matcher
            .hits
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_empty());
    }

    #[test]
    fn a_tempestade_de_log_nao_estoura_a_memoria() {
        let matcher = PatternMatcher::create();
        let compilada =
            PatternRule::from_rule(&regra(json!({ "pattern": "erro" }), None)).expect("compila");
        matcher.set_rules(vec![compilada]);

        let agora = Utc::now();
        for _ in 0..(MAX_HITS_PER_KEY + 5_000) {
            matcher.record(Some(1), &linha("erro repetido", Some(3)), agora);
        }
        let guardados = matcher
            .hits
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .values()
            .map(Vec::len)
            .sum::<usize>();
        assert_eq!(guardados, MAX_HITS_PER_KEY, "a contagem satura no teto");
    }
}
