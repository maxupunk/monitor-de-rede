//! Quando uma tarefa roda: expressão cron de 5 campos
//! (`minuto hora dia-do-mês mês dia-da-semana`) na hora local do servidor
//! (`TZ`), como o resto das agendas do sistema.
//!
//! O cálculo das ocorrências é do `croner`; a frase em português
//! ([`Schedule::label`]) é daqui, porque é ela que a tela e a IA mostram ao
//! usuário ("de segunda a sexta às 08:00").

use chrono::{DateTime, Local, TimeZone, Utc};
use croner::{
    parser::{CronParser, Seconds},
    Cron,
};

use crate::services::shared::errors::{AppError, AppResult};

/// Maior expressão aceita — a coluna tem 120 caracteres.
const MAX_LENGTH: usize = 120;

/// Uma agenda válida.
#[derive(Debug, Clone)]
pub struct Schedule {
    expression: String,
    cron: Cron,
}

impl Schedule {
    /// Lê e confere a expressão.
    ///
    /// # Errors
    ///
    /// `Validation` com a explicação do formato quando a expressão não é um
    /// cron de 5 campos ou nunca acontece (31 de fevereiro).
    pub fn parse(expression: &str) -> AppResult<Self> {
        let expression = expression.split_whitespace().collect::<Vec<_>>().join(" ");
        if expression.is_empty() {
            return Err(AppError::validation(
                "Informe quando a tarefa roda (expressão cron)",
            ));
        }
        if expression.len() > MAX_LENGTH {
            return Err(AppError::validation(format!(
                "Expressão cron longa demais (até {MAX_LENGTH} caracteres)"
            )));
        }
        let cron = CronParser::builder()
            .seconds(Seconds::Disallowed)
            .build()
            .parse(&expression)
            .map_err(|_| {
                AppError::validation(format!(
                    "Expressão cron inválida: \"{expression}\". Use 5 campos — minuto hora dia mês dia-da-semana (ex.: \"0 3 * * *\" = todo dia às 03:00)"
                ))
            })?;
        let schedule = Self { expression, cron };
        if schedule.next_after_in(&Utc::now()).is_none() {
            return Err(AppError::validation(format!(
                "A expressão \"{}\" nunca acontece",
                schedule.expression
            )));
        }
        Ok(schedule)
    }

    /// A expressão normalizada (espaços simples), como é gravada.
    #[must_use]
    pub fn expression(&self) -> &str {
        &self.expression
    }

    /// A primeira ocorrência estritamente depois de `at`, no fuso de `at`.
    #[must_use]
    pub fn next_after_in<Tz: TimeZone>(&self, at: &DateTime<Tz>) -> Option<DateTime<Tz>> {
        self.cron.find_next_occurrence(at, false).ok()
    }

    /// A próxima execução depois de `now`, na hora local do servidor.
    #[must_use]
    pub fn next_after(&self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        self.next_after_in(&now.with_timezone(&Local))
            .map(|at| at.with_timezone(&Utc))
    }

    /// As próximas `count` execuções depois de `now`.
    #[must_use]
    pub fn upcoming(&self, now: DateTime<Utc>, count: usize) -> Vec<DateTime<Utc>> {
        let mut runs = Vec::with_capacity(count);
        let mut at = now;
        while runs.len() < count {
            let Some(next) = self.next_after(at) else {
                break;
            };
            runs.push(next);
            at = next;
        }
        runs
    }

    /// A agenda em português ("todo dia às 03:00").
    #[must_use]
    pub fn label(&self) -> String {
        describe(&self.expression)
    }
}

/// A expressão em português. O que não tem frase pronta (mês específico, dia
/// do mês e da semana juntos, nomes em inglês) sai como `cron <expressão>`.
#[must_use]
pub fn describe(expression: &str) -> String {
    let fallback = || format!("cron {expression}");
    if let Some(label) = nickname(expression) {
        return label.to_string();
    }
    let fields: Vec<&str> = expression.split_whitespace().collect();
    let [minute, hour, day_of_month, month, day_of_week] = fields[..] else {
        return fallback();
    };
    if month != "*" {
        return fallback();
    }
    let (Some(days), Some(time)) = (
        days_label(day_of_month, day_of_week),
        time_label(minute, hour),
    ) else {
        return fallback();
    };
    match (time, days) {
        (Time::At(time), None) => format!("todo dia {time}"),
        (Time::At(time), Some(days)) => format!("{days} {time}"),
        (Time::Every(every), None) => every,
        (Time::Every(every), Some(days)) => format!("{every}, {days}"),
    }
}

fn nickname(expression: &str) -> Option<&'static str> {
    Some(match expression.trim().to_ascii_lowercase().as_str() {
        "@hourly" => "a cada hora, em ponto",
        "@daily" | "@midnight" => "todo dia às 00:00",
        "@weekly" => "todo domingo às 00:00",
        "@monthly" => "todo dia 1 do mês às 00:00",
        "@yearly" | "@annually" => "todo 1º de janeiro às 00:00",
        _ => return None,
    })
}

/// A parte do horário: horários fixos ou uma repetição.
enum Time {
    /// "às 08:00 e 18:00".
    At(String),
    /// "a cada 15 minutos".
    Every(String),
}

fn number(field: &str) -> Option<u32> {
    field.parse().ok()
}

fn numbers(field: &str) -> Option<Vec<u32>> {
    let mut values: Vec<u32> = field.split(',').map(number).collect::<Option<_>>()?;
    values.sort_unstable();
    values.dedup();
    Some(values)
}

fn step(field: &str) -> Option<u32> {
    field.strip_prefix("*/").and_then(number)
}

fn range(field: &str) -> Option<(u32, u32)> {
    let (start, end) = field.split_once('-')?;
    Some((number(start)?, number(end)?))
}

/// "a, b e c".
fn join(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [only] => only.clone(),
        [rest @ .., last] => format!("{} e {last}", rest.join(", ")),
    }
}

fn plural(count: u32, one: &str, many: &str) -> String {
    if count == 1 {
        one.to_string()
    } else {
        format!("{count} {many}")
    }
}

fn past_the_hour(minute: u32) -> String {
    if minute == 0 {
        "em ponto".to_string()
    } else {
        format!("aos {minute} minutos")
    }
}

fn time_label(minute: &str, hour: &str) -> Option<Time> {
    let every_minutes = match (minute, step(minute)) {
        ("*", _) => Some(1),
        (_, Some(n)) => Some(n),
        _ => None,
    };
    if let Some(n) = every_minutes {
        let every = format!("a cada {}", plural(n, "minuto", "minutos"));
        return match (hour, range(hour)) {
            ("*", _) => Some(Time::Every(every)),
            (_, Some((start, end))) => Some(Time::Every(format!(
                "{every}, das {start:02}:00 às {end:02}:59"
            ))),
            _ => None,
        };
    }
    let minutes = numbers(minute)?;
    if hour == "*" || step(hour).is_some() {
        let [minute] = minutes[..] else {
            return None;
        };
        let hours = step(hour).unwrap_or(1);
        return Some(Time::Every(format!(
            "a cada {}, {}",
            plural(hours, "hora", "horas"),
            past_the_hour(minute)
        )));
    }
    if let Some((start, end)) = range(hour) {
        let [minute] = minutes[..] else {
            return None;
        };
        return Some(Time::Every(format!(
            "a cada hora, das {start:02}:{minute:02} às {end:02}:{minute:02}"
        )));
    }
    let hours = numbers(hour)?;
    let mut times: Vec<(u32, u32)> = hours
        .iter()
        .flat_map(|hour| minutes.iter().map(move |minute| (*hour, *minute)))
        .collect();
    if times.is_empty() || times.len() > 6 {
        return None;
    }
    times.sort_unstable();
    let times: Vec<String> = times
        .iter()
        .map(|(hour, minute)| format!("{hour:02}:{minute:02}"))
        .collect();
    Some(Time::At(format!("às {}", join(&times))))
}

/// Dias da semana do campo, de segunda (1) a domingo (0); `7` é domingo.
fn weekdays(field: &str) -> Option<Vec<u32>> {
    let mut days = Vec::new();
    for part in field.split(',') {
        if let Some((start, end)) = range(part) {
            if start > end {
                return None;
            }
            days.extend(start..=end);
        } else {
            days.push(number(part)?);
        }
    }
    if days.iter().any(|day| *day > 7) {
        return None;
    }
    let mut days: Vec<u32> = days.into_iter().map(|day| day % 7).collect();
    days.sort_unstable_by_key(|day| (day + 6) % 7);
    days.dedup();
    Some(days)
}

const WEEKDAY_ONE: [&str; 7] = [
    "todo domingo",
    "toda segunda-feira",
    "toda terça-feira",
    "toda quarta-feira",
    "toda quinta-feira",
    "toda sexta-feira",
    "todo sábado",
];

const WEEKDAY_MANY: [&str; 7] = [
    "domingos", "segundas", "terças", "quartas", "quintas", "sextas", "sábados",
];

/// `None` quando não há frase; `Some(None)` quando é todo dia.
#[allow(clippy::option_option)]
fn days_label(day_of_month: &str, day_of_week: &str) -> Option<Option<String>> {
    match (day_of_month, day_of_week) {
        ("*", "*") => Some(None),
        ("*", field) => {
            let days = weekdays(field)?;
            let label = match days[..] {
                [] => return None,
                [only] => WEEKDAY_ONE[only as usize].to_string(),
                _ if days.len() == 7 => return Some(None),
                [1, 2, 3, 4, 5] => "de segunda a sexta".to_string(),
                [6, 0] => "nos fins de semana".to_string(),
                [first, ..] => {
                    let names: Vec<String> = days
                        .iter()
                        .map(|day| WEEKDAY_MANY[*day as usize].to_string())
                        .collect();
                    let article = if first == 0 || first == 6 {
                        "aos"
                    } else {
                        "às"
                    };
                    format!("{article} {}", join(&names))
                }
            };
            Some(Some(label))
        }
        (field, "*") => {
            let days = numbers(field)?;
            if days.iter().any(|day| !(1..=31).contains(day)) {
                return None;
            }
            let names: Vec<String> = days.iter().map(ToString::to_string).collect();
            Some(Some(match days[..] {
                [only] => format!("todo dia {only} do mês"),
                _ => format!("nos dias {} do mês", join(&names)),
            }))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use chrono::{FixedOffset, NaiveDate};

    use super::*;

    #[test]
    fn frases_das_agendas_comuns() {
        let casos = [
            ("* * * * *", "a cada minuto"),
            ("*/15 * * * *", "a cada 15 minutos"),
            (
                "*/5 8-18 * * 1-5",
                "a cada 5 minutos, das 08:00 às 18:59, de segunda a sexta",
            ),
            ("0 * * * *", "a cada hora, em ponto"),
            ("30 */6 * * *", "a cada 6 horas, aos 30 minutos"),
            ("0 3 * * *", "todo dia às 03:00"),
            ("0 8,18 * * *", "todo dia às 08:00 e 18:00"),
            ("0 8 * * 1-5", "de segunda a sexta às 08:00"),
            ("30 7 * * 1", "toda segunda-feira às 07:30"),
            ("0 22 * * 0", "todo domingo às 22:00"),
            ("0 22 * * 7", "todo domingo às 22:00"),
            ("0 9 * * 6,0", "nos fins de semana às 09:00"),
            ("0 9 * * 1,3,5", "às segundas, quartas e sextas às 09:00"),
            ("0 9 * * 0-6", "todo dia às 09:00"),
            ("0 0 1 * *", "todo dia 1 do mês às 00:00"),
            ("15 2 1,15 * *", "nos dias 1 e 15 do mês às 02:15"),
            ("0 7-19 * * *", "a cada hora, das 07:00 às 19:00"),
            ("@daily", "todo dia às 00:00"),
            ("0 0 1 1 *", "cron 0 0 1 1 *"),
            ("0 0 * * MON", "cron 0 0 * * MON"),
        ];
        for (expressao, frase) in casos {
            assert_eq!(describe(expressao), frase, "{expressao}");
        }
    }

    #[test]
    fn expressao_invalida_explica_o_formato() {
        for ruim in ["", "0 3 * *", "61 * * * *", "0 0 3 * * *", "abc"] {
            let erro = Schedule::parse(ruim).unwrap_err().to_string();
            assert!(
                erro.contains("cron") || erro.contains("quando"),
                "{ruim}: {erro}"
            );
        }
        assert!(
            Schedule::parse("0 0 31 2 *").is_err(),
            "31 de fevereiro nunca chega"
        );
    }

    #[test]
    fn normaliza_os_espacos() {
        let agenda = Schedule::parse("  0   3 * *  * ").unwrap();
        assert_eq!(agenda.expression(), "0 3 * * *");
        assert_eq!(agenda.label(), "todo dia às 03:00");
    }

    #[test]
    fn proxima_execucao_no_fuso_local() {
        let fortaleza = FixedOffset::west_opt(3 * 3600).unwrap();
        let agora = fortaleza
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2026, 10, 23)
                    .unwrap()
                    .and_hms_opt(3, 0, 0)
                    .unwrap(),
            )
            .unwrap();
        let agenda = Schedule::parse("0 3 * * *").unwrap();
        let proxima = agenda.next_after_in(&agora).unwrap();
        assert_eq!(
            proxima.to_rfc3339(),
            "2026-10-24T03:00:00-03:00",
            "estritamente depois"
        );

        let util = Schedule::parse("0 8 * * 1-5").unwrap();
        // 23/10/2026 é sexta: a próxima depois das 03:00 é hoje às 08:00, e
        // a seguinte pula o fim de semana.
        let hoje = util.next_after_in(&agora).unwrap();
        assert_eq!(hoje.to_rfc3339(), "2026-10-23T08:00:00-03:00");
        let segunda = util.next_after_in(&hoje).unwrap();
        assert_eq!(segunda.to_rfc3339(), "2026-10-26T08:00:00-03:00");
    }

    #[test]
    fn proximas_execucoes_em_sequencia() {
        let agenda = Schedule::parse("*/15 * * * *").unwrap();
        let agora = Utc::now();
        let proximas = agenda.upcoming(agora, 3);
        assert_eq!(proximas.len(), 3);
        assert!(proximas[0] > agora);
        assert_eq!((proximas[1] - proximas[0]).num_minutes(), 15);
        assert_eq!((proximas[2] - proximas[1]).num_minutes(), 15);
    }
}
