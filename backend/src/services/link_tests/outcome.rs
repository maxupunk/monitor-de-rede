//! Desfecho de pernas e testes — funções puras.
//!
//! * perna: `succeeded` se toda etapa não pulada deu certo, `failed` se
//!   alguma falhou, `cancelled` se o teste foi cancelado no meio, `skipped`
//!   se nada rodou;
//! * teste: `succeeded` (todas as pernas), `partial` (algumas), `failed`
//!   (nenhuma) ou `cancelled`;
//! * resumo: o **gargalo** é a perna, entre as que mediram, com a menor vazão
//!   em qualquer dos sentidos.

use super::model::{LinkTestStage, LinkTestStatus, LinkTestStepStatus, LinkTestSummary};

/// Estado da perna a partir das etapas.
#[must_use]
pub fn leg_status(stages: &[LinkTestStage]) -> LinkTestStepStatus {
    let statuses: Vec<LinkTestStepStatus> = stages.iter().map(|stage| stage.status).collect();
    if statuses.contains(&LinkTestStepStatus::Cancelled) {
        return LinkTestStepStatus::Cancelled;
    }
    if statuses
        .iter()
        .all(|status| *status == LinkTestStepStatus::Skipped)
    {
        return LinkTestStepStatus::Skipped;
    }
    if statuses.iter().all(|status| {
        matches!(
            status,
            LinkTestStepStatus::Succeeded | LinkTestStepStatus::Skipped
        )
    }) {
        LinkTestStepStatus::Succeeded
    } else {
        LinkTestStepStatus::Failed
    }
}

/// Estado do teste a partir das pernas.
#[must_use]
pub fn test_status(legs: &[LinkTestStepStatus], cancelled: bool) -> LinkTestStatus {
    if cancelled || legs.contains(&LinkTestStepStatus::Cancelled) {
        return LinkTestStatus::Cancelled;
    }
    let ok = legs
        .iter()
        .filter(|status| **status == LinkTestStepStatus::Succeeded)
        .count();
    if ok > 0 && ok == legs.len() {
        LinkTestStatus::Succeeded
    } else if ok > 0 {
        LinkTestStatus::Partial
    } else {
        LinkTestStatus::Failed
    }
}

/// Uma perna para o resumo: `(seq, estado, forward, reverse)`.
pub type LegRates = (u32, LinkTestStepStatus, Option<f64>, Option<f64>);

fn min_of(values: impl Iterator<Item = f64>) -> Option<f64> {
    values.fold(None, |acc, value| {
        Some(acc.map_or(value, |acc: f64| acc.min(value)))
    })
}

/// Resumo do teste; `None` quando nenhuma perna mediu.
#[must_use]
pub fn summarize(legs: &[LegRates]) -> Option<LinkTestSummary> {
    let measured: Vec<&LegRates> = legs
        .iter()
        .filter(|(_, status, forward, reverse)| {
            *status == LinkTestStepStatus::Succeeded && (forward.is_some() || reverse.is_some())
        })
        .collect();
    if measured.is_empty() {
        return None;
    }
    let bottleneck = measured
        .iter()
        .filter_map(|(seq, _, forward, reverse)| {
            min_of(forward.iter().chain(reverse.iter()).copied()).map(|rate| (*seq, rate))
        })
        .min_by(|(_, x), (_, y)| x.total_cmp(y))
        .map(|(seq, _)| seq);
    Some(LinkTestSummary {
        bottleneck_leg_seq: bottleneck,
        min_forward_bps: min_of(measured.iter().filter_map(|leg| leg.2)),
        min_reverse_bps: min_of(measured.iter().filter_map(|leg| leg.3)),
    })
}

/// A primeira mensagem de etapa que falhou, para o erro da perna.
#[must_use]
pub fn first_failure(stages: &[LinkTestStage]) -> Option<String> {
    stages
        .iter()
        .find(|stage| stage.status == LinkTestStepStatus::Failed)
        .map(|stage| {
            stage
                .message
                .clone()
                .unwrap_or_else(|| format!("A etapa \"{}\" falhou", stage.label))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::link_tests::model::LinkTestStageKey;
    use LinkTestStepStatus::{Cancelled, Failed, Pending, Skipped, Succeeded};

    fn stages(statuses: [LinkTestStepStatus; 6]) -> Vec<LinkTestStage> {
        LinkTestStageKey::ALL
            .into_iter()
            .zip(statuses)
            .map(|(key, status)| LinkTestStage {
                status,
                ..LinkTestStage::pending(key)
            })
            .collect()
    }

    #[test]
    fn estado_da_perna() {
        assert_eq!(
            leg_status(&stages([
                Skipped, Skipped, Succeeded, Succeeded, Succeeded, Succeeded
            ])),
            Succeeded
        );
        assert_eq!(
            leg_status(&stages([
                Skipped, Skipped, Succeeded, Succeeded, Skipped, Succeeded
            ])),
            Succeeded,
            "sentido não pedido fica pulado"
        );
        assert_eq!(
            leg_status(&stages([
                Skipped, Skipped, Succeeded, Failed, Succeeded, Succeeded
            ])),
            Failed
        );
        assert_eq!(
            leg_status(&stages([
                Succeeded, Skipped, Succeeded, Cancelled, Cancelled, Succeeded
            ])),
            Cancelled
        );
        assert_eq!(leg_status(&stages([Skipped; 6])), Skipped);
        assert_eq!(
            leg_status(&stages([
                Skipped, Skipped, Succeeded, Pending, Pending, Pending
            ])),
            Failed,
            "etapa que não chegou ao fim não é sucesso"
        );
    }

    #[test]
    fn estado_do_teste() {
        assert_eq!(
            test_status(&[Succeeded, Succeeded], false),
            LinkTestStatus::Succeeded
        );
        assert_eq!(
            test_status(&[Succeeded, Failed], false),
            LinkTestStatus::Partial
        );
        assert_eq!(
            test_status(&[Succeeded, Skipped], false),
            LinkTestStatus::Partial
        );
        assert_eq!(
            test_status(&[Failed, Skipped], false),
            LinkTestStatus::Failed
        );
        assert_eq!(test_status(&[], false), LinkTestStatus::Failed);
        assert_eq!(test_status(&[Succeeded], true), LinkTestStatus::Cancelled);
        assert_eq!(
            test_status(&[Succeeded, Cancelled], false),
            LinkTestStatus::Cancelled
        );
    }

    #[test]
    fn resumo_aponta_o_gargalo() {
        let legs = [
            (1, Succeeded, Some(900e6), Some(850e6)),
            (2, Succeeded, Some(400e6), Some(95e6)),
            (3, Failed, Some(1.0), None),
            (4, Succeeded, Some(300e6), None),
        ];
        let summary = summarize(&legs).unwrap();
        assert_eq!(summary.bottleneck_leg_seq, Some(2), "95 Mbit/s na volta");
        assert_eq!(summary.min_forward_bps, Some(300e6));
        assert_eq!(summary.min_reverse_bps, Some(95e6));
        assert_eq!(summarize(&[(1, Failed, None, None)]), None);
        assert_eq!(summarize(&[]), None);
    }

    #[test]
    fn primeira_falha_vira_o_erro_da_perna() {
        let mut lista = stages([Skipped, Skipped, Succeeded, Failed, Failed, Succeeded]);
        lista[3].message = Some("recusado".into());
        assert_eq!(first_failure(&lista).as_deref(), Some("recusado"));
        lista[3].message = None;
        assert_eq!(
            first_failure(&lista).as_deref(),
            Some("A etapa \"A → B\" falhou")
        );
        assert_eq!(first_failure(&stages([Skipped; 6])), None);
    }
}
