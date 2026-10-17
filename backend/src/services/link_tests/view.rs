//! Linhas do banco → contrato da tela (§5.1). Funções puras: o `GET`, o SSE
//! e o executor montam o `LinkTest` pelo mesmo caminho.

use sea_orm::prelude::DateTimeWithTimeZone;
use serde::de::DeserializeOwned;

use super::model::{
    from_db, LinkTest, LinkTestEndpoint, LinkTestEndpointKind, LinkTestKind, LinkTestLeg,
    LinkTestLegKind, LinkTestOrigin, LinkTestPlanLeg, LinkTestSample, LinkTestSchedule,
    LinkTestSide, LinkTestSpec, LinkTestStage, LinkTestStatus, LinkTestStepStatus, LinkTestSummary,
    LinkTestUserRef,
};
use crate::models::{link_test_legs, link_test_schedules, link_tests};

/// Instante no formato do contrato (RFC 3339).
#[must_use]
pub fn iso(at: &DateTimeWithTimeZone) -> String {
    at.to_rfc3339()
}

fn iso_opt(at: Option<&DateTimeWithTimeZone>) -> Option<String> {
    at.map(iso)
}

fn json_or<T: DeserializeOwned>(value: &serde_json::Value, fallback: T) -> T {
    serde_json::from_value(value.clone()).unwrap_or(fallback)
}

fn unknown_endpoint() -> LinkTestEndpoint {
    LinkTestEndpoint {
        kind: LinkTestEndpointKind::Device,
        device_id: None,
        agent_id: None,
        name: "?".to_owned(),
        ip: None,
        platform: None,
    }
}

/// A perna como a tela a vê. `samples` só quando pedido (detalhe).
#[must_use]
pub fn leg(row: &link_test_legs::Model, with_samples: bool) -> LinkTestLeg {
    LinkTestLeg {
        plan: LinkTestPlanLeg {
            seq: u32::try_from(row.seq).unwrap_or_default(),
            kind: from_db(&row.kind).unwrap_or(LinkTestLegKind::Link),
            a: json_or(&row.a, unknown_endpoint()),
            b: json_or(&row.b, unknown_endpoint()),
            edge_id: row.edge_id,
            engine: row.engine.as_deref().and_then(from_db),
            server_side: row.server_side.as_deref().and_then(from_db::<LinkTestSide>),
            duration_s: u8::try_from(row.duration_s).unwrap_or_default(),
            runnable: row.runnable,
            issues: json_or(&row.issues, Vec::new()),
        },
        status: from_db(&row.status).unwrap_or(LinkTestStepStatus::Pending),
        stages: json_or(&row.stages, LinkTestStage::all_pending()),
        forward_bps: row.forward_bps,
        reverse_bps: row.reverse_bps,
        retransmits: row.retransmits,
        error: row.error.clone(),
        started_at: iso_opt(row.started_at.as_ref()),
        finished_at: iso_opt(row.finished_at.as_ref()),
        samples: with_samples.then(|| {
            row.samples
                .as_ref()
                .map(|samples| json_or::<Vec<LinkTestSample>>(samples, Vec::new()))
                .unwrap_or_default()
        }),
    }
}

/// O teste completo, com as pernas em ordem.
#[must_use]
pub fn test(
    row: &link_tests::Model,
    legs: &[link_test_legs::Model],
    requested_by: Option<LinkTestUserRef>,
    with_samples: bool,
) -> LinkTest {
    let mut legs: Vec<LinkTestLeg> = legs
        .iter()
        .filter(|leg| leg.test_id == row.id)
        .map(|row| leg(row, with_samples))
        .collect();
    legs.sort_by_key(|leg| leg.plan.seq);
    LinkTest {
        id: row.id,
        kind: from_db(&row.kind).unwrap_or(LinkTestKind::Link),
        status: from_db(&row.status).unwrap_or(LinkTestStatus::Failed),
        origin: from_db(&row.origin).unwrap_or(LinkTestOrigin::User),
        schedule_id: row.schedule_id,
        link_id: row.link_id,
        source_device_id: row.source_device_id,
        target_device_id: row.target_device_id,
        spec: json_or(&row.spec, LinkTestSpec::default()),
        legs,
        summary: row
            .summary
            .as_ref()
            .and_then(|summary| serde_json::from_value::<LinkTestSummary>(summary.clone()).ok()),
        error: row.error.clone(),
        requested_by,
        created_at: iso(&row.created_at),
        started_at: iso_opt(row.started_at.as_ref()),
        finished_at: iso_opt(row.finished_at.as_ref()),
    }
}

/// Um agendamento.
#[must_use]
pub fn schedule(row: &link_test_schedules::Model) -> LinkTestSchedule {
    LinkTestSchedule {
        id: row.id,
        name: row.name.clone(),
        kind: from_db(&row.kind).unwrap_or(LinkTestKind::Link),
        link_id: row.link_id,
        source_device_id: row.source_device_id,
        target_device_id: row.target_device_id,
        spec: json_or(&row.spec, LinkTestSpec::default()),
        interval_minutes: row.interval_minutes,
        window_start: row.window_start.clone(),
        window_end: row.window_end.clone(),
        enabled: row.enabled,
        last_run_at: iso_opt(row.last_run_at.as_ref()),
        next_run_at: iso_opt(row.next_run_at.as_ref()),
        last_test_id: row.last_test_id,
        last_status: row.last_status.as_deref().and_then(from_db),
        created_at: iso(&row.created_at),
        updated_at: iso(&row.updated_at),
    }
}

/// `,1,5,9,` — a coluna de filtro por dispositivo.
#[must_use]
pub fn device_ids_column(ids: impl IntoIterator<Item = i64>) -> String {
    let ids: std::collections::BTreeSet<i64> = ids.into_iter().collect();
    if ids.is_empty() {
        return String::new();
    }
    let joined: Vec<String> = ids.iter().map(ToString::to_string).collect();
    format!(",{},", joined.join(","))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coluna_de_dispositivos_casa_por_like() {
        assert_eq!(device_ids_column([5, 1, 5, 9]), ",1,5,9,");
        assert_eq!(device_ids_column([]), "");
        assert!(device_ids_column([1, 15]).contains(",15,"));
        assert!(!device_ids_column([15]).contains(",1,"));
    }
}
