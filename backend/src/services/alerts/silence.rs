//! Silenciamento e reconhecimento de alertas (§8.7).
//!
//! Nenhum dos dois fecha o alerta: o evento continua "aberto" para a
//! deduplicação do motor. O que muda é como ele aparece na Central de Alertas.

use chrono::{DateTime, Duration, Utc};
use sea_orm::{ActiveModelTrait, ConnectionTrait, Set};
use serde_json::{json, Value};

use crate::{
    models::alert_events,
    services::{alerts::contracts::AlertStatus, shared::errors::AppResult},
};

/// Duração padrão do silêncio, em minutos — igual à do backend anterior.
pub const DEFAULT_SILENCE_MINUTES: i64 = 60;

/// `true` quando o alerta está silenciado **e** a janela ainda não venceu.
///
/// Um `silenced` com prazo vencido volta a ser ruído legítimo: é o que impede
/// que "silenciar por 60 minutos" vire "silenciar para sempre".
#[must_use]
pub fn is_silenced(event: &alert_events::Model, now: DateTime<Utc>) -> bool {
    if event.status != AlertStatus::Silenced.as_str() {
        return false;
    }
    silenced_until(event).is_some_and(|until| until > now)
}

/// Lê `data.silencedUntil` como instante. Valor ausente ou ilegível vira
/// `None`, e o alerta é tratado como não silenciado.
#[must_use]
pub fn silenced_until(event: &alert_events::Model) -> Option<DateTime<Utc>> {
    let raw = event.data.as_ref()?.get("silencedUntil")?.as_str()?;
    DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|value| value.with_timezone(&Utc))
}

/// Silencia o alerta por `minutes` e grava o prazo em `data.silencedUntil`.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn silence_alert<C: ConnectionTrait>(
    db: &C,
    event: alert_events::Model,
    minutes: i64,
) -> AppResult<alert_events::Model> {
    let minutes = if minutes > 0 {
        minutes
    } else {
        DEFAULT_SILENCE_MINUTES
    };
    let until = Utc::now() + Duration::minutes(minutes);
    let data = merge_data(
        event.data.clone(),
        "silencedUntil",
        json!(until.to_rfc3339()),
    );

    let mut active: alert_events::ActiveModel = event.into();
    active.status = Set(AlertStatus::Silenced.as_str().into());
    active.data = Set(Some(data));
    Ok(active.update(db).await?)
}

/// Marca o alerta como reconhecido, carimbando o instante em `data`.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn acknowledge_alert<C: ConnectionTrait>(
    db: &C,
    event: alert_events::Model,
) -> AppResult<alert_events::Model> {
    let data = merge_data(
        event.data.clone(),
        "acknowledgedAt",
        json!(Utc::now().to_rfc3339()),
    );
    let mut active: alert_events::ActiveModel = event.into();
    active.status = Set(AlertStatus::Acknowledged.as_str().into());
    active.data = Set(Some(data));
    Ok(active.update(db).await?)
}

/// Acrescenta uma chave ao `data` preservando o resto.
///
/// O `data` carrega o título e o contexto do disparo; sobrescrevê-lo inteiro
/// apagaria justamente o que a tela mostra sobre o alerta.
fn merge_data(current: Option<Value>, key: &str, value: Value) -> Value {
    let mut object = match current {
        Some(Value::Object(map)) => map,
        _ => serde_json::Map::new(),
    };
    object.insert(key.to_string(), value);
    Value::Object(object)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evento(status: &str, data: Option<Value>) -> alert_events::Model {
        let now = Utc::now().into();
        alert_events::Model {
            id: 1,
            alert_rule_id: None,
            device_id: None,
            monitor_id: None,
            scope_key: None,
            status: status.into(),
            severity: "warning".into(),
            started_at: now,
            resolved_at: None,
            message: None,
            data,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn silencio_vencido_deixa_de_silenciar() {
        let passado = (Utc::now() - Duration::minutes(1)).to_rfc3339();
        let futuro = (Utc::now() + Duration::minutes(1)).to_rfc3339();
        assert!(!is_silenced(
            &evento(
                AlertStatus::Silenced.as_str(),
                Some(json!({ "silencedUntil": passado }))
            ),
            Utc::now()
        ));
        assert!(is_silenced(
            &evento(
                AlertStatus::Silenced.as_str(),
                Some(json!({ "silencedUntil": futuro }))
            ),
            Utc::now()
        ));
    }

    #[test]
    fn sem_prazo_ou_com_prazo_ilegivel_nao_conta_como_silenciado() {
        assert!(!is_silenced(
            &evento(AlertStatus::Silenced.as_str(), None),
            Utc::now()
        ));
        assert!(!is_silenced(
            &evento(
                AlertStatus::Silenced.as_str(),
                Some(json!({ "silencedUntil": "ontem" }))
            ),
            Utc::now()
        ));
    }

    #[test]
    fn status_diferente_de_silenced_nunca_e_silencio() {
        let futuro = (Utc::now() + Duration::minutes(30)).to_rfc3339();
        assert!(!is_silenced(
            &evento("active", Some(json!({ "silencedUntil": futuro }))),
            Utc::now()
        ));
    }

    #[test]
    fn merge_preserva_o_titulo_gravado_no_disparo() {
        let merged = merge_data(
            Some(json!({ "title": "Regra — Roteador", "ruleName": "Regra" })),
            "silencedUntil",
            json!("2026-08-11T12:00:00Z"),
        );
        assert_eq!(merged["title"], "Regra — Roteador");
        assert_eq!(merged["silencedUntil"], "2026-08-11T12:00:00Z");
    }
}
