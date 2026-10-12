//! Depois da varredura, o Laya dá o palpite onde a heurística ficou em dúvida:
//! tipo `unknown`/`web_device` ou sistema sem evidência.
//!
//! Roda em segundo plano, depois que a execução já foi gravada e concluída —
//! a varredura nunca espera por ele. O palpite vai para `details.laya` do
//! resultado (JSON, sem migração) e, se a sessão ao vivo ainda é desta
//! execução, para o host dela: a tela recebe pelo stream da varredura que já
//! está aberto, sem chamada nova.

use loco_rs::app::AppContext;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::Value;

use super::{device_identifier::scores, merger::DiscoveredHost, service::ScanSessionService};
use crate::{
    models::discovery_results,
    services::{
        ai::laya::{
            config::LayaFeature,
            decisions::device_identity::{DeviceFacts, DeviceIdentity, IdentitySuggestion},
            runtime::{Lane, LayaRuntime},
        },
        shared::errors::AppResult,
    },
};

/// Chave do palpite em `details` do resultado e em `data` do host ao vivo.
pub const LAYA_KEY: &str = "laya";
/// Uma rede grande não vira uma fila de minutos no Ollaya.
const MAX_HOSTS_PER_RUN: usize = 64;

fn type_is_uncertain(device_type: Option<&str>) -> bool {
    matches!(device_type, None | Some("unknown" | "web_device" | ""))
}

fn system_is_uncertain(data: &Value) -> bool {
    match data.get("identity") {
        None | Some(Value::Null) => true,
        Some(identity) => {
            identity.get("source").and_then(Value::as_str) == Some("padrão")
                || identity.get("operatingSystem").and_then(Value::as_str) == Some("other")
        }
    }
}

/// O que perguntar sobre este host; `None` se a heurística já resolveu tudo.
#[must_use]
pub fn plan_for(
    host: &DiscoveredHost,
    min_confidence: u8,
) -> Option<(DeviceFacts, DeviceIdentity)> {
    let decision = DeviceIdentity {
        ask_type: type_is_uncertain(host.device_type.as_deref()),
        ask_system: system_is_uncertain(&host.data),
        min_confidence,
    };
    if !decision.ask_type && !decision.ask_system {
        return None;
    }
    let facts = DeviceFacts::from_discovery(
        host.hostname.as_deref(),
        host.mdns_name.as_deref(),
        host.vendor.as_deref(),
        &host.open_ports,
        &host.data,
    );
    facts.has_evidence().then_some((facts, decision))
}

/// Quanto sobra da confiança de um palpite sem nenhuma evidência a favor.
const UNSUPPORTED_FACTOR: f64 = 0.6;

/// Confere o palpite de tipo contra a heurística. Quando ela viu evidência
/// de **outros** tipos e nenhuma do tipo escolhido, o palpite perde 40% da
/// confiança e some se cair abaixo do mínimo — foi assim que um OpenWrt
/// (Net-SNMP, SSH, HTTP) virou "NAS 98%". Sem evidência alguma, o palpite
/// fica como veio: é justamente quando o Laya mais ajuda.
#[must_use]
pub fn calibrate(
    mut suggestion: IdentitySuggestion,
    host: &DiscoveredHost,
    min_confidence: u8,
) -> IdentitySuggestion {
    let Some(device_type) = suggestion.device_type.as_mut() else {
        return suggestion;
    };
    let scores = scores(host, None);
    let supported = scores.iter().any(|(kind, _)| *kind == device_type.value);
    if !supported && !scores.is_empty() {
        device_type.confidence =
            (device_type.confidence * UNSUPPORTED_FACTOR * 10.0).round() / 10.0;
        if device_type.confidence < f64::from(min_confidence) {
            suggestion.device_type = None;
        }
    }
    suggestion
}

/// Dispara o enriquecimento sem prender quem chamou.
pub fn spawn(ctx: &AppContext, run_id: i64, hosts: Vec<DiscoveredHost>) {
    let ctx = ctx.clone();
    tokio::spawn(async move {
        if let Err(error) = enrich(&ctx, run_id, &hosts).await {
            tracing::debug!(%error, run_id, "laya: enriquecimento da descoberta falhou");
        }
    });
}

/// Pergunta ao Laya sobre os hosts em dúvida e grava os palpites. Devolve
/// quantos hosts ganharam sugestão.
pub async fn enrich(ctx: &AppContext, run_id: i64, hosts: &[DiscoveredHost]) -> AppResult<usize> {
    let runtime = LayaRuntime::from_context(ctx);
    let Some(settings) = runtime
        .settings_for(&ctx.db, LayaFeature::DeviceIdentity)
        .await
    else {
        return Ok(0);
    };

    let mut enriched = 0;
    for host in hosts {
        if enriched >= MAX_HOSTS_PER_RUN {
            break;
        }
        let Some((facts, decision)) = plan_for(host, settings.min_confidence) else {
            continue;
        };
        let Some(outcome) = runtime
            .decide_with(&settings, Lane::Background, &facts.state(), &decision)
            .await
        else {
            continue;
        };
        let suggestion = calibrate(
            outcome.value.answered_by(&outcome.model),
            host,
            settings.min_confidence,
        );
        if suggestion.is_empty() {
            continue;
        }
        store(ctx, run_id, &host.ip_address, &suggestion).await?;
        enriched += 1;
    }
    Ok(enriched)
}

async fn store(
    ctx: &AppContext,
    run_id: i64,
    ip_address: &str,
    suggestion: &IdentitySuggestion,
) -> AppResult<()> {
    let value = serde_json::to_value(suggestion).unwrap_or(Value::Null);
    let row = discovery_results::Entity::find()
        .filter(crate::models::_entities::discovery_results::Column::DiscoveryRunId.eq(run_id))
        .filter(crate::models::_entities::discovery_results::Column::IpAddress.eq(ip_address))
        .one(&ctx.db)
        .await?;
    // O IP pode ter virado dispositivo enquanto o Laya pensava: nada a gravar.
    if let Some(row) = row {
        let mut data = row.data.clone().unwrap_or_else(|| serde_json::json!({}));
        if !data.get("details").is_some_and(Value::is_object) {
            data["details"] = serde_json::json!({});
        }
        data["details"][LAYA_KEY] = value.clone();
        let mut active: discovery_results::ActiveModel = row.into();
        active.data = Set(Some(data));
        active.update(&ctx.db).await?;
    }
    if let Ok(session) = ScanSessionService::from_context(ctx) {
        session
            .annotate_host(run_id, ip_address, LAYA_KEY, value)
            .await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn host(device_type: Option<&str>, data: Value) -> DiscoveredHost {
        DiscoveredHost {
            ip_address: "10.0.0.9".into(),
            vendor: Some("Hikvision".into()),
            device_type: device_type.map(str::to_string),
            open_ports: vec![554],
            data,
            ..DiscoveredHost::default()
        }
    }

    #[test]
    fn so_pergunta_o_que_a_heuristica_nao_resolveu() {
        let snmp_identified =
            json!({ "identity": { "operatingSystem": "routeros", "source": "snmp" } });
        assert!(plan_for(&host(Some("router"), snmp_identified.clone()), 60).is_none());

        let (_, decision) = plan_for(&host(Some("web_device"), snmp_identified), 60).unwrap();
        assert!(decision.ask_type && !decision.ask_system);

        let (_, decision) = plan_for(&host(Some("camera"), json!({})), 60).unwrap();
        assert!(!decision.ask_type && decision.ask_system);

        let weak = json!({ "identity": { "operatingSystem": "other", "source": "padrão" } });
        assert!(
            plan_for(&host(Some("camera"), weak), 60)
                .unwrap()
                .1
                .ask_system
        );
    }

    fn suggestion(kind: &str, confidence: f64) -> IdentitySuggestion {
        IdentitySuggestion {
            device_type: Some(crate::services::ai::laya::suggestion::LayaSuggestion {
                value: kind.into(),
                confidence,
                model: "laya:multilingual".into(),
            }),
            operating_system: None,
        }
    }

    #[test]
    fn palpite_sem_evidencia_a_favor_perde_confianca() {
        // O caso real: OpenWrt com Net-SNMP, SSH e HTTP chamado de NAS.
        let openwrt = DiscoveredHost {
            ip_address: "10.0.0.2".into(),
            vendor: Some("Net-SNMP".into()),
            open_ports: vec![22, 53, 80, 443],
            data: json!({}),
            ..DiscoveredHost::default()
        };
        let nas = calibrate(suggestion("nas", 98.0), &openwrt, 60);
        assert!(nas.device_type.is_none(), "58,8% fica abaixo do mínimo");

        let router = calibrate(suggestion("router", 90.0), &openwrt, 60);
        assert_eq!(router.device_type.unwrap().confidence, 90.0);
    }

    #[test]
    fn sem_evidencia_nenhuma_o_palpite_fica_como_veio() {
        let bare = DiscoveredHost {
            ip_address: "10.0.0.9".into(),
            ..DiscoveredHost::default()
        };
        let kept = calibrate(suggestion("iot", 80.0), &bare, 60);
        assert_eq!(kept.device_type.unwrap().confidence, 80.0);
    }

    #[test]
    fn sem_evidencia_nao_pergunta() {
        let bare = DiscoveredHost {
            ip_address: "10.0.0.9".into(),
            ..DiscoveredHost::default()
        };
        assert!(plan_for(&bare, 60).is_none());
    }
}
