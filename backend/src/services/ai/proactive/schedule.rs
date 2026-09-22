//! Regras de tempo da IA proativa, sem relógio embutido: quem chama passa o
//! "agora", e os testes passam o instante que quiserem.

use std::collections::VecDeque;

use chrono::{DateTime, Datelike, Duration, TimeZone, Timelike, Utc, Weekday};

use super::config::AiDigestSchedule;

/// Janela deslizante de uma hora para os resumos automáticos.
#[derive(Debug, Default)]
pub struct HourlyLimiter {
    hits: VecDeque<DateTime<Utc>>,
}

impl HourlyLimiter {
    /// Registra um uso se ainda cabe no teto da última hora.
    pub fn try_acquire(&mut self, now: DateTime<Utc>, max_per_hour: u32) -> bool {
        let window_start = now - Duration::hours(1);
        while self.hits.front().is_some_and(|hit| *hit <= window_start) {
            self.hits.pop_front();
        }
        if self.hits.len() >= max_per_hour as usize {
            return false;
        }
        self.hits.push_back(now);
        true
    }
}

/// Horas cobertas por um resumo desta frequência.
#[must_use]
pub const fn period_hours(schedule: AiDigestSchedule) -> i64 {
    match schedule {
        AiDigestSchedule::Weekly => 24 * 7,
        AiDigestSchedule::Off | AiDigestSchedule::Daily => 24,
    }
}

/// O resumo periódico já deveria ter saído?
///
/// Diário: uma vez por dia, a partir da hora configurada. Semanal: uma vez
/// por semana ISO, a partir da hora configurada da segunda-feira — se o
/// servidor estava desligado na segunda, sai assim que voltar.
#[must_use]
pub fn is_digest_due<Tz: TimeZone>(
    now: &DateTime<Tz>,
    last_sent: Option<DateTime<Utc>>,
    schedule: AiDigestSchedule,
    hour: u8,
) -> bool {
    let last_sent = last_sent.map(|at| at.with_timezone(&now.timezone()));
    let reached_hour = now.hour() >= u32::from(hour);
    match schedule {
        AiDigestSchedule::Off => false,
        AiDigestSchedule::Daily => {
            reached_hour
                && last_sent
                    .as_ref()
                    .is_none_or(|sent| sent.date_naive() < now.date_naive())
        }
        AiDigestSchedule::Weekly => {
            let started = now.weekday() != Weekday::Mon || reached_hour;
            started
                && last_sent
                    .as_ref()
                    .is_none_or(|sent| sent.iso_week() != now.iso_week())
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::FixedOffset;

    use super::*;

    fn brt(y: i32, m: u32, d: u32, h: u32) -> DateTime<FixedOffset> {
        FixedOffset::west_opt(3 * 3600)
            .unwrap()
            .with_ymd_and_hms(y, m, d, h, 0, 0)
            .unwrap()
    }

    #[test]
    fn limitador_libera_de_novo_depois_de_uma_hora() {
        let mut limiter = HourlyLimiter::default();
        let t0 = Utc.with_ymd_and_hms(2026, 9, 21, 10, 0, 0).unwrap();
        assert!(limiter.try_acquire(t0, 2));
        assert!(limiter.try_acquire(t0 + Duration::minutes(1), 2));
        assert!(!limiter.try_acquire(t0 + Duration::minutes(2), 2));
        assert!(limiter.try_acquire(t0 + Duration::minutes(61), 2));
    }

    #[test]
    fn diario_sai_uma_vez_a_partir_da_hora() {
        let schedule = AiDigestSchedule::Daily;
        assert!(!is_digest_due(&brt(2026, 9, 21, 7), None, schedule, 8));
        assert!(is_digest_due(&brt(2026, 9, 21, 8), None, schedule, 8));

        let sent_today = brt(2026, 9, 21, 8).with_timezone(&Utc);
        assert!(!is_digest_due(
            &brt(2026, 9, 21, 20),
            Some(sent_today),
            schedule,
            8
        ));
        assert!(is_digest_due(
            &brt(2026, 9, 22, 9),
            Some(sent_today),
            schedule,
            8
        ));
    }

    #[test]
    fn dia_local_manda_mesmo_quando_utc_ja_virou() {
        // 23h de Brasília já é o dia seguinte em UTC; o envio das 8h locais
        // do mesmo dia não pode contar como "amanhã".
        let sent = brt(2026, 9, 21, 8).with_timezone(&Utc);
        assert!(!is_digest_due(
            &brt(2026, 9, 21, 23),
            Some(sent),
            AiDigestSchedule::Daily,
            8
        ));
    }

    #[test]
    fn semanal_sai_na_segunda_ou_assim_que_voltar() {
        let schedule = AiDigestSchedule::Weekly;
        // 21/09/2026 é segunda-feira.
        assert!(!is_digest_due(&brt(2026, 9, 21, 7), None, schedule, 8));
        assert!(is_digest_due(&brt(2026, 9, 21, 8), None, schedule, 8));
        assert!(
            is_digest_due(&brt(2026, 9, 23, 3), None, schedule, 8),
            "perdeu a segunda"
        );

        let sent = brt(2026, 9, 21, 8).with_timezone(&Utc);
        assert!(!is_digest_due(
            &brt(2026, 9, 25, 12),
            Some(sent),
            schedule,
            8
        ));
        assert!(is_digest_due(&brt(2026, 9, 28, 8), Some(sent), schedule, 8));
    }

    #[test]
    fn desligado_nunca_sai() {
        assert!(!is_digest_due(
            &brt(2026, 9, 21, 12),
            None,
            AiDigestSchedule::Off,
            0
        ));
    }
}
