//! `POST /api/devices/identify`: descobre o sistema **agora**, e diz como.
//!
//! Consulta o SNMP (quando há comunidade) e lê a identificação do servidor SSH,
//! as duas em paralelo. Devolve a evidência crua junto da conclusão: o campo que
//! só afirma "Linux" não tem como ser conferido, e foi exatamente assim que um
//! OpenWrt ficou identificado errado sem ninguém perceber.
//!
//! **Não grava nada.** É consulta — quem decide o que fica é o formulário.
//! Quando a evidência é fraca (`padrão`/`cadastro`), o Laya dá um palpite ao
//! lado — sugestão, nunca a conclusão.

use loco_rs::app::AppContext;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::Deserialize;

use super::{access::AccessContext, systems};
use crate::{
    models::_entities::discovery_results,
    services::{
        ai::laya::{
            config::LayaFeature,
            decisions::device_identity::{DeviceFacts, DeviceIdentity, IdentitySuggestion},
            runtime::{Lane, LayaRuntime},
        },
        discovery::laya_identity::LAYA_KEY,
        shared::errors::AppResult,
        syslog::hints,
    },
};

/// Corpo de `POST /api/devices/identify`.
///
/// Vem do **formulário**, e não de um id: o operador precisa poder identificar
/// antes de salvar, e num cadastro novo ainda não há dispositivo para consultar.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentifyInput {
    pub name: Option<String>,
    pub ip_address: Option<String>,
    pub snmp_version: Option<String>,
    pub snmp_community: Option<String>,
    pub vendor: Option<String>,
    pub model: Option<String>,
}

pub async fn identify(
    ctx: &AppContext,
    input: &IdentifyInput,
) -> AppResult<systems::IdentifyResult> {
    let host = input
        .ip_address
        .as_deref()
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .and_then(|texto| texto.parse::<std::net::IpAddr>().ok());

    let comunidade = input
        .snmp_community
        .as_deref()
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .unwrap_or(crate::services::preferences::DEFAULT_SNMP_COMMUNITY);

    let (snmp, ssh) = match host {
        Some(endereco) => {
            let consulta_snmp = async {
                hints::identidade_snmp(endereco, comunidade, input.snmp_version.as_deref()).await
            };
            tokio::join!(consulta_snmp, hints::sonda_ssh(endereco))
        }
        None => (None, (false, None)),
    };
    let ssh_banner = ssh.1;

    let descoberta = match host {
        Some(endereco) => {
            discovery_results::Entity::find()
                .filter(discovery_results::Column::IpAddress.eq(endereco.to_string()))
                .order_by_desc(discovery_results::Column::LastSeenAt)
                .one(&ctx.db)
                .await?
        }
        None => None,
    };

    let cached_sys_descr = descoberta
        .as_ref()
        .and_then(|item| campo_identidade_da_descoberta(item, "sysDescr"));
    let cached_sys_object_id = descoberta
        .as_ref()
        .and_then(|item| campo_identidade_da_descoberta(item, "sysObjectId"));
    let cached_sys_name = descoberta
        .as_ref()
        .and_then(|item| campo_identidade_da_descoberta(item, "sysName"));
    let sys_descr = snmp
        .as_ref()
        .and_then(|info| info.sys_descr.clone())
        .or(cached_sys_descr);
    let sys_object_id = snmp
        .as_ref()
        .and_then(|info| info.sys_object_id.clone())
        .or(cached_sys_object_id);

    let suggested_vendor = snmp
        .as_ref()
        .and_then(|info| info.hardware_vendor.clone())
        .or_else(|| {
            descoberta
                .as_ref()
                .and_then(|item| campo_identidade_da_descoberta(item, "hardwareVendor"))
        })
        .or_else(|| descoberta.as_ref().and_then(fabricante_da_descoberta))
        .filter(|valor| {
            snmp.as_ref()
                .and_then(|info| info.sys_descr.as_deref())
                .is_none_or(|descricao| valor.trim() != descricao.trim())
        })
        .filter(|valor| !valor.trim().is_empty());
    let suggested_model = snmp
        .as_ref()
        .and_then(|info| info.hardware_model.clone())
        .or_else(|| {
            descoberta
                .as_ref()
                .and_then(|item| campo_identidade_da_descoberta(item, "hardwareModel"))
        })
        .or_else(|| descoberta.as_ref().and_then(modelo_da_descoberta))
        .filter(|valor| !valor.trim().is_empty());
    let suggested_name = systems::suggest_name(
        snmp.as_ref()
            .and_then(|info| info.sys_name.as_deref())
            .or(cached_sys_name.as_deref()),
        descoberta
            .as_ref()
            .and_then(|item| item.hostname.as_deref()),
        descoberta
            .as_ref()
            .and_then(|item| item.mdns_name.as_deref()),
    );

    let achado = systems::detect(&systems::Evidence {
        // A declaração fica de fora: o botão existe para dizer o que o
        // **equipamento** é, e devolver de volta o que o operador acabou de
        // escolher no seletor faria a detecção concordar consigo mesma.
        declared: None,
        sys_object_id: sys_object_id.as_deref(),
        sys_descr: sys_descr.as_deref(),
        ssh_banner: ssh_banner.as_deref(),
        name: input.name.as_deref().or(suggested_name.as_deref()),
        vendor: input.vendor.as_deref().or(suggested_vendor.as_deref()),
        model: input.model.as_deref().or(suggested_model.as_deref()),
    });

    let acesso = AccessContext::load(&ctx.db)
        .await?
        .resolve_draft(input.ip_address.as_deref());
    let from_discovery = snmp.is_none() && achado.source == systems::source::SNMP;
    let laya = laya_suggestion(ctx, &achado, descoberta.as_ref(), || DeviceFacts {
        hostname: descoberta.as_ref().and_then(|item| item.hostname.clone()),
        mdns_name: descoberta.as_ref().and_then(|item| item.mdns_name.clone()),
        vendor: suggested_vendor.clone(),
        open_ports: descoberta
            .as_ref()
            .map(portas_da_descoberta)
            .unwrap_or_default(),
        ssdp_server: None,
        sys_descr: sys_descr.clone(),
        sys_object_id: sys_object_id.clone(),
        sys_name: suggested_name.clone(),
        hardware_vendor: suggested_vendor.clone(),
        hardware_model: suggested_model.clone(),
        ssh_banner: ssh_banner.clone(),
    })
    .await;
    Ok(systems::IdentifyResult {
        operating_system: achado.system.id.to_owned(),
        label: achado.system.label.to_owned(),
        source: achado.source.to_owned(),
        reason: achado.reason,
        sys_descr,
        sys_object_id,
        probed: snmp.is_some() || ssh_banner.is_some(),
        ssh_banner,
        suggested_vendor,
        suggested_model,
        suggested_name,
        access_mode: acesso.mode.id().to_owned(),
        access_mode_reason: acesso.reason,
        from_discovery,
        laya,
    })
}

/// O palpite do Laya quando a detecção ficou sem evidência forte. Reaproveita o
/// da última descoberta; sem ele, pergunta agora (fila interativa: alguém espera).
async fn laya_suggestion(
    ctx: &AppContext,
    achado: &systems::Detection,
    descoberta: Option<&discovery_results::Model>,
    facts: impl FnOnce() -> DeviceFacts,
) -> Option<IdentitySuggestion> {
    if achado.source != systems::source::DEFAULT && achado.source != systems::source::REGISTRY {
        return None;
    }
    let cached = descoberta
        .and_then(|item| item.data.as_ref()?.get("details")?.get(LAYA_KEY).cloned())
        .and_then(|value| serde_json::from_value::<IdentitySuggestion>(value).ok())
        .filter(|suggestion| !suggestion.is_empty());
    if cached.is_some() {
        return cached;
    }
    let facts = facts();
    if !facts.has_evidence() {
        return None;
    }
    let runtime = LayaRuntime::from_context(ctx);
    let settings = runtime
        .settings_for(&ctx.db, LayaFeature::DeviceIdentity)
        .await?;
    let decision = DeviceIdentity {
        ask_type: true,
        ask_system: true,
        min_confidence: settings.min_confidence,
    };
    let outcome = runtime
        .decide_with(&settings, Lane::Interactive, &facts.state(), &decision)
        .await?;
    Some(outcome.value.answered_by(&outcome.model)).filter(|suggestion| !suggestion.is_empty())
}

fn portas_da_descoberta(item: &discovery_results::Model) -> Vec<u16> {
    item.data
        .as_ref()
        .and_then(|data| data.get("openPorts"))
        .and_then(|ports| serde_json::from_value(ports.clone()).ok())
        .unwrap_or_default()
}

fn campo_identidade_da_descoberta(item: &discovery_results::Model, campo: &str) -> Option<String> {
    let detalhes = item.data.as_ref()?.get("details")?;
    detalhes
        .get("identity")
        .or_else(|| detalhes.get("snmpSystem"))
        .and_then(|identidade| identidade.get(campo))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .map(str::to_owned)
}

/// Probes mais novos podem anexar o modelo ao documento livre da descoberta.
/// Lemos os dois formatos já usados (`details.model` e
/// `details.snmp.model`) sem transformar `sysDescr` em modelo: descrição de SO
/// não é identidade de hardware.
fn modelo_da_descoberta(item: &discovery_results::Model) -> Option<String> {
    let detalhes = item.data.as_ref()?.get("details")?;
    detalhes
        .get("model")
        .or_else(|| detalhes.get("snmp")?.get("model"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .map(str::to_owned)
}

/// O scanner antigo gravava `sysDescr` em `vendor` quando o OUI não respondia.
/// Uma descrição de kernel não pode aparecer como fabricante no formulário.
fn fabricante_da_descoberta(item: &discovery_results::Model) -> Option<String> {
    item.vendor
        .as_deref()
        .map(str::trim)
        .filter(|valor| !valor.is_empty() && valor.len() <= 80)
        .filter(|valor| !crate::services::devices::adapters::registry::is_system_description(valor))
        .map(str::to_owned)
}
