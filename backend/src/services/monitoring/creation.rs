//! Regras de cadastro de monitor: tipo e nome válidos, alvo alcançável,
//! intervalo coerente com o dispositivo e configuração montada a partir do
//! alvo informado.
//!
//! Quem cria monitor — a tela (`POST /api/monitors`) ou a IA — passa por aqui,
//! então as duas portas aplicam as mesmas regras.

use sea_orm::{ActiveModelTrait, ConnectionTrait, EntityTrait, Set};

use crate::{
    dtos::resources::MonitorInput,
    models::{devices, monitors},
    services::{
        monitoring::{execution_guard::calculate_smart_timeout_seconds, reachability},
        preferences,
        shared::errors::{AppError, AppResult},
    },
};

/// Monta a configuração do monitor a partir do que veio informado.
#[must_use]
pub fn build_configuration(
    kind: &str,
    supplied: Option<serde_json::Value>,
    target: Option<&str>,
    port: Option<i64>,
    fallback: Option<&serde_json::Value>,
) -> serde_json::Value {
    let mut config = supplied
        .or_else(|| fallback.cloned())
        .unwrap_or_else(|| serde_json::json!({}));
    let serde_json::Value::Object(ref mut object) = config else {
        return serde_json::json!({});
    };
    // O limite de resposta é definido automaticamente a partir do tipo e do
    // intervalo do monitor. Remove também valores salvos por versões antigas.
    object.remove("timeoutMs");
    if let Some(target) = target.filter(|target| !target.trim().is_empty()) {
        match kind.to_lowercase().as_str() {
            "ping" | "snmp" => {
                object.entry("host").or_insert_with(|| target.into());
            }
            "http" | "https" => {
                object.entry("url").or_insert_with(|| {
                    if target.starts_with("http") {
                        target.into()
                    } else {
                        format!("http://{target}").into()
                    }
                });
            }
            "tcp" => {
                object.entry("host").or_insert_with(|| target.into());
            }
            "dns" => {
                object.entry("domain").or_insert_with(|| target.into());
            }
            _ => {}
        }
    }
    if let Some(port) = port.filter(|port| (1..=65535).contains(port)) {
        object.entry("port").or_insert_with(|| port.into());
    }
    config
}

pub const SUPPORTED_MONITOR_TYPES: &[&str] = &[
    "ping",
    "http",
    "https",
    "tcp",
    "dns",
    "snmp",
    "ssl",
    "port_scan",
];

/// Tipo e nome obrigatórios e válidos.
///
/// # Errors
///
/// Validação quando falta tipo ou nome, o tipo não existe, o nome tem quebra
/// de linha ou o intervalo é menor que 1 s.
pub fn require_kind_name(input: &MonitorInput) -> AppResult<(&str, &str)> {
    let kind = input
        .monitor_type
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::validation("Tipo do monitor é obrigatório"))?;
    if !SUPPORTED_MONITOR_TYPES.contains(&kind.to_lowercase().as_str()) {
        return Err(AppError::validation(format!(
            "Tipo de monitor não suportado: '{kind}'."
        )));
    }
    let name = input
        .name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::validation("Nome do monitor é obrigatório"))?;
    if name.contains('\n') || name.contains('\r') {
        return Err(AppError::validation(
            "Nome do monitor não pode conter quebra de linha.",
        ));
    }
    if let Some(interval) = input.interval_seconds {
        if interval < 1 {
            return Err(AppError::validation(
                "O intervalo deve ser de pelo menos 1 segundo.",
            ));
        }
    }
    Ok((kind, name))
}

/// Intervalo de um monitor SNMP vinculado: é sempre o do dispositivo.
///
/// # Errors
///
/// Dispositivo inexistente ou intervalo informado diferente do dispositivo.
pub async fn canonical_snmp_interval<C: ConnectionTrait>(
    db: &C,
    device_id: Option<i64>,
    supplied_interval: Option<i32>,
) -> AppResult<Option<i32>> {
    let Some(device_id) = device_id else {
        return Ok(None);
    };
    let device = devices::Entity::find_by_id(device_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::not_found("Dispositivo não encontrado"))?;
    if let Some(supplied_interval) = supplied_interval.map(|value| value.max(1)) {
        if supplied_interval != device.snmp_poll_interval_seconds {
            return Err(AppError::validation(
                "O intervalo de coleta SNMP é definido no dispositivo e se aplica a todos os itens SNMP vinculados a ele",
            ));
        }
    }
    Ok(Some(device.snmp_poll_interval_seconds))
}

/// Recusa um monitor de alcance apontado para um dispositivo que a rede não
/// alcança.
///
/// A pergunta é do domínio ([`reachability`]); aqui só se carrega a linha do
/// dispositivo para poder fazê-la. Sem `device_id` não há o que checar: um
/// monitor solto aponta para um alvo que o operador informou.
///
/// # Errors
///
/// Dispositivo inexistente ou alcance não permitido.
pub async fn ensure_reach_allowed<C: ConnectionTrait>(
    db: &C,
    device_id: Option<i64>,
    kind: &str,
) -> AppResult<()> {
    if !reachability::is_reach_check(kind) {
        return Ok(());
    }
    let Some(device_id) = device_id else {
        return Ok(());
    };
    let device = devices::Entity::find_by_id(device_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::not_found("Dispositivo não encontrado"))?;
    reachability::ensure_allowed_for_device(&device, kind)
}

/// Cria o monitor aplicando as regras de cadastro.
///
/// O intervalo padrão vem das preferências, não de um literal: é este o
/// ponto de consumo que faz "Intervalo padrão de coleta por Ping" significar
/// alguma coisa. Monitor SNMP vinculado a dispositivo continua herdando o
/// intervalo do próprio dispositivo — a preferência não o atropela.
///
/// # Errors
///
/// Validação das regras acima ou erro do banco.
pub async fn create<C: ConnectionTrait>(db: &C, input: MonitorInput) -> AppResult<monitors::Model> {
    let (kind, name) = require_kind_name(&input)?;
    let kind = kind.to_string();
    let name = name.to_string();
    ensure_reach_allowed(db, input.device_id, &kind).await?;
    let enabled = input.enabled.or(input.is_enabled).unwrap_or(true);
    let default_interval = preferences::load(db).await?.default_ping_interval_seconds;
    let interval_seconds = if kind.eq_ignore_ascii_case("snmp") {
        canonical_snmp_interval(db, input.device_id, input.interval_seconds)
            .await?
            .unwrap_or_else(|| input.interval_seconds.unwrap_or(default_interval).max(1))
    } else {
        input.interval_seconds.unwrap_or(default_interval).max(1)
    };
    let timeout_seconds = calculate_smart_timeout_seconds(&kind, interval_seconds)
        .min((interval_seconds - 1).max(1))
        .max(1);
    let config = build_configuration(
        &kind,
        input.configuration,
        input.target.as_deref(),
        input.port,
        None,
    );
    Ok(monitors::ActiveModel {
        device_id: Set(input.device_id),
        probe_id: Set(input.probe_id),
        r#type: Set(kind),
        name: Set(name),
        configuration: Set(config),
        interval_seconds: Set(interval_seconds),
        timeout_seconds: Set(timeout_seconds),
        retry_count: Set(input.retry_count.unwrap_or(3).max(0)),
        enabled: Set(enabled),
        status: Set(input.status.unwrap_or_else(|| "unknown".into())),
        ..Default::default()
    }
    .insert(db)
    .await?)
}

#[cfg(test)]
mod tests {
    use super::build_configuration;

    #[test]
    fn configuracao_do_monitor_descarta_timeout_informado() {
        let config = build_configuration(
            "ping",
            Some(serde_json::json!({ "host": "127.0.0.1", "timeoutMs": 60_000 })),
            None,
            None,
            None,
        );

        assert!(config.get("timeoutMs").is_none());
    }
}
