use sea_orm::{entity::prelude::*, ExprTrait, QueryOrder, QuerySelect};

pub use super::_entities::monitors::{ActiveModel, Column, Entity, Model};

pub type Monitors = Entity;

/// Quantos monitores vencidos um ciclo do scheduler recolhe (§9.2). O teto
/// existe para um atraso acumulado não virar uma rajada única que estoura o
/// pool de conexões.
pub const DUE_BATCH_LIMIT: u64 = 50;

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(self, _db: &C, insert: bool) -> std::result::Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if !insert && self.updated_at.is_unchanged() {
            let mut this = self;
            this.updated_at = sea_orm::ActiveValue::Set(chrono::Utc::now().into());
            Ok(this)
        } else {
            Ok(self)
        }
    }
}

impl Model {
    /// §6.1 — `target`: o alvo legível da checagem.
    ///
    /// A ordem `host → url → domain` é a do backend atual e **não** é
    /// arbitrária: um monitor HTTP guarda `url`, um TCP guarda `host` + `port`,
    /// um DNS guarda `domain`. String vazia quando não há nenhum — o frontend
    /// exibe direto e `null` viraria "undefined" na tabela.
    #[must_use]
    pub fn target(&self) -> String {
        for key in ["host", "url", "domain"] {
            if let Some(value) = self.configuration.get(key).and_then(|v| v.as_str()) {
                if !value.is_empty() {
                    return value.to_string();
                }
            }
        }
        String::new()
    }

    /// §6.1 — `port`: só existe para os tipos que têm porta (TCP, alguns HTTP).
    #[must_use]
    pub fn port(&self) -> Option<i64> {
        self.configuration.get("port").and_then(|v| {
            // O frontend às vezes manda a porta como string ("8080"); o Adonis
            // aceitava as duas formas porque o JSON não é tipado.
            v.as_i64()
                .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        })
    }

    /// §6.1 — `isEnabled`: espelho de `enabled`.
    ///
    /// Redundante no banco, mas o frontend lê `isEnabled` em algumas telas e
    /// `enabled` em outras. Remover um dos dois é mudança de contrato.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Entity {
    /// Monitores vencidos: o laço central do `scheduler_run` (§9.2).
    ///
    /// `next_run_at IS NULL` entra porque um monitor recém-criado nunca foi
    /// agendado e precisa rodar na primeira oportunidade. A ordenação por
    /// `next_run_at` faz o mais atrasado ir primeiro.
    ///
    /// Consulta desenhada para o índice `monitors_enabled_next_run_at_index`.
    pub fn find_due(now: DateTimeWithTimeZone) -> Select<Entity> {
        Entity::find()
            .filter(Column::Enabled.eq(true))
            .filter(Column::NextRunAt.is_null().or(Column::NextRunAt.lte(now)))
            .order_by_asc(Column::NextRunAt)
            .limit(DUE_BATCH_LIMIT)
    }

    /// Monitores habilitados de um dispositivo — usado no recálculo de status
    /// (§8) e em `GET /api/devices/:id/monitors` (§7.5).
    ///
    /// Desenhada para o índice `monitors_device_id_enabled_index`.
    pub fn find_enabled_for_device(device_id: i64) -> Select<Entity> {
        Entity::find()
            .filter(Column::DeviceId.eq(device_id))
            .filter(Column::Enabled.eq(true))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::json;

    fn com_config(configuration: serde_json::Value) -> Model {
        Model {
            id: 1,
            device_id: None,
            probe_id: None,
            r#type: "tcp".to_string(),
            name: "Teste".to_string(),
            configuration,
            interval_seconds: 15,
            timeout_seconds: 10,
            retry_count: 3,
            enabled: true,
            next_run_at: None,
            last_run_at: None,
            status: "unknown".to_string(),
            created_at: Utc::now().into(),
            updated_at: Utc::now().into(),
        }
    }

    #[test]
    fn target_segue_a_ordem_host_url_domain() {
        assert_eq!(com_config(json!({"host": "10.0.0.1"})).target(), "10.0.0.1");
        assert_eq!(
            com_config(json!({"url": "https://exemplo.com"})).target(),
            "https://exemplo.com"
        );
        assert_eq!(
            com_config(json!({"domain": "exemplo.com"})).target(),
            "exemplo.com"
        );
        // `host` vence quando há mais de um.
        assert_eq!(
            com_config(json!({"host": "10.0.0.1", "url": "https://x"})).target(),
            "10.0.0.1"
        );
    }

    #[test]
    fn target_vazio_em_vez_de_nulo() {
        assert_eq!(com_config(json!({})).target(), "");
        assert_eq!(com_config(json!({"host": ""})).target(), "");
        assert_eq!(com_config(json!({"outra": "coisa"})).target(), "");
    }

    #[test]
    fn port_aceita_numero_e_string() {
        assert_eq!(com_config(json!({"port": 8080})).port(), Some(8080));
        assert_eq!(com_config(json!({"port": "8080"})).port(), Some(8080));
        assert_eq!(com_config(json!({})).port(), None);
        assert_eq!(com_config(json!({"port": "nem-porta"})).port(), None);
    }

    #[test]
    fn is_enabled_espelha_enabled() {
        let mut monitor = com_config(json!({}));
        assert!(monitor.is_enabled());
        monitor.enabled = false;
        assert!(!monitor.is_enabled());
    }
}
