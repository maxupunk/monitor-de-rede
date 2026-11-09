//! Janela de manutenção de um alvo: o horário em que um deploy aprovado
//! pode rodar (ex.: 23:00–05:00), no fuso da central.
//!
//! Puro: recebe o instante e devolve se está dentro ou quando a janela
//! abre. A janela pode atravessar a meia-noite.

use chrono::{DateTime, Duration, NaiveTime, TimeZone};

use netmonitor_core::shared::errors::{AppError, AppResult};

/// Uma janela diária `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub start: NaiveTime,
    pub end: NaiveTime,
}

/// Lê `HH:MM`.
///
/// # Errors
///
/// `Validation` fora do formato.
pub fn parse_time(value: &str) -> AppResult<NaiveTime> {
    NaiveTime::parse_from_str(value.trim(), "%H:%M")
        .map_err(|_| AppError::validation(format!("Horário inválido: \"{value}\" (use HH:MM)")))
}

impl Window {
    /// A janela das duas pontas, ou nenhuma quando as duas estão vazias.
    ///
    /// # Errors
    ///
    /// `Validation` com só uma das pontas, horário inválido ou início igual
    /// ao fim (janela vazia).
    pub fn from_columns(start: Option<&str>, end: Option<&str>) -> AppResult<Option<Self>> {
        fn blank(value: Option<&str>) -> Option<&str> {
            value.map(str::trim).filter(|value| !value.is_empty())
        }
        match (blank(start), blank(end)) {
            (None, None) => Ok(None),
            (Some(start), Some(end)) => {
                let window = Self {
                    start: parse_time(start)?,
                    end: parse_time(end)?,
                };
                if window.start == window.end {
                    return Err(AppError::validation(
                        "A janela precisa de início e fim diferentes",
                    ));
                }
                Ok(Some(window))
            }
            _ => Err(AppError::validation(
                "Informe o início e o fim da janela, ou deixe os dois vazios",
            )),
        }
    }

    #[must_use]
    pub fn contains(&self, time: NaiveTime) -> bool {
        if self.start < self.end {
            time >= self.start && time < self.end
        } else {
            // Atravessa a meia-noite: 23:00–05:00.
            time >= self.start || time < self.end
        }
    }

    /// `now` se a janela está aberta; senão, a próxima abertura.
    #[must_use]
    pub fn next_opening<Tz: TimeZone>(&self, now: DateTime<Tz>) -> DateTime<Tz> {
        if self.contains(now.time()) {
            return now;
        }
        let today = now.date_naive().and_time(self.start);
        let candidate = if now.naive_local() < today {
            today
        } else {
            today + Duration::days(1)
        };
        now.timezone()
            .from_local_datetime(&candidate)
            .earliest()
            .unwrap_or_else(|| now.clone() + Duration::hours(1))
    }
}

#[cfg(test)]
mod tests {
    use chrono::{FixedOffset, Timelike};

    use super::*;

    fn at(hour: u32, minute: u32) -> DateTime<FixedOffset> {
        FixedOffset::west_opt(3 * 3600)
            .expect("fuso")
            .with_ymd_and_hms(2026, 11, 8, hour, minute, 0)
            .single()
            .expect("instante")
    }

    fn window(start: &str, end: &str) -> Window {
        Window::from_columns(Some(start), Some(end))
            .expect("válida")
            .expect("janela")
    }

    #[test]
    fn janela_que_atravessa_a_meia_noite() {
        let night = window("23:00", "05:00");
        assert!(night.contains(at(23, 30).time()));
        assert!(night.contains(at(2, 0).time()));
        assert!(!night.contains(at(5, 0).time()));
        assert!(!night.contains(at(12, 0).time()));
    }

    #[test]
    fn proxima_abertura() {
        let night = window("23:00", "05:00");
        assert_eq!(night.next_opening(at(23, 10)), at(23, 10));
        let opening = night.next_opening(at(14, 0));
        assert_eq!((opening.hour(), opening.minute()), (23, 0));
        assert_eq!(opening.date_naive(), at(14, 0).date_naive());

        let lunch = window("12:00", "13:00");
        let opening = lunch.next_opening(at(15, 0));
        assert_eq!(
            opening.date_naive(),
            at(15, 0).date_naive() + Duration::days(1)
        );
    }

    #[test]
    fn colunas_vazias_sao_qualquer_hora() {
        assert_eq!(Window::from_columns(None, Some(" ")).expect("ok"), None);
        assert!(Window::from_columns(Some("23:00"), None).is_err());
        assert!(Window::from_columns(Some("25:00"), Some("01:00")).is_err());
        assert!(Window::from_columns(Some("01:00"), Some("01:00")).is_err());
    }
}
