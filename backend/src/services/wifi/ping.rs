//! Latência do equipamento até uma estação, medida de dentro do AP pelo papel
//! `ping` do plugin. Sem resposta é sem média — nunca um número inventado.

use std::net::IpAddr;

use loco_rs::app::AppContext;
use serde_json::json;

use super::{
    contract,
    contract::PingReply,
    quality,
    source::{self, Requester},
};
use crate::{
    dtos::wifi::{WifiPingInput, WifiPingResult},
    services::{
        plugins::actions,
        shared::errors::{AppError, AppResult},
    },
};

/// Mede a latência do equipamento até `input.target`.
///
/// # Errors
///
/// IP inválido; equipamento inexistente, sem plugin Wi-Fi ou com plugin que
/// não mede latência; falha da ação no equipamento.
pub async fn ping(
    ctx: &AppContext,
    input: &WifiPingInput,
    user_id: Option<i64>,
) -> AppResult<WifiPingResult> {
    let target: IpAddr = input
        .target
        .trim()
        .parse()
        .map_err(|_| AppError::validation("Informe o endereço IP da estação."))?;
    let device = actions::device(ctx, input.device_id).await?;
    let source = source::for_device(&ctx.db, device.id)
        .await?
        .ok_or_else(|| {
            AppError::business_rule(format!("{} não tem um plugin de Wi-Fi ativo.", device.name))
        })?;
    let action = source.roles.ping.as_deref().ok_or_else(|| {
        AppError::business_rule(format!(
            "O plugin `{}` não mede latência até as estações.",
            source.slug
        ))
    })?;
    let output = source
        .run(
            ctx,
            device.id,
            action,
            json!({ "target": target.to_string() }),
            Requester::user(user_id),
            format!("Latência até a estação {target}"),
        )
        .await?;
    let reply = contract::parse_ping(output).map_err(|error| {
        AppError::service_unavailable(format!("O plugin `{}`: {error}.", source.slug))
    })?;
    Ok(result(device.id, target, reply))
}

fn result(device_id: i64, target: IpAddr, reply: PingReply) -> WifiPingResult {
    let PingReply {
        sent,
        received,
        avg_ms,
    } = reply;
    let received = received.min(sent);
    let loss_pct = if sent == 0 {
        100.0
    } else {
        f64::from(sent - received) / f64::from(sent) * 100.0
    };
    let avg_ms = avg_ms.filter(|_| received > 0);
    WifiPingResult {
        device_id,
        target: target.to_string(),
        sent,
        received,
        loss_pct: (loss_pct * 10.0).round() / 10.0,
        avg_ms,
        grade: quality::latency(avg_ms, received < sent),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dtos::wifi::WifiGrade;

    fn alvo() -> IpAddr {
        "192.168.1.150".parse().unwrap()
    }

    #[test]
    fn resposta_completa_e_classificada() {
        let medido = result(
            7,
            alvo(),
            PingReply {
                sent: 3,
                received: 3,
                avg_ms: Some(4.2),
            },
        );
        assert_eq!(medido.loss_pct, 0.0);
        assert_eq!(medido.avg_ms, Some(4.2));
        assert_eq!(medido.grade, WifiGrade::Excellent);
    }

    #[test]
    fn perda_e_silencio_sao_criticos_sem_media_inventada() {
        let parcial = result(
            7,
            alvo(),
            PingReply {
                sent: 3,
                received: 2,
                avg_ms: Some(3.0),
            },
        );
        assert_eq!(parcial.loss_pct, 33.3);
        assert_eq!(parcial.grade, WifiGrade::Critical);

        let mudo = result(
            7,
            alvo(),
            PingReply {
                sent: 3,
                received: 0,
                avg_ms: Some(1.0),
            },
        );
        assert_eq!(mudo.loss_pct, 100.0);
        assert_eq!(mudo.avg_ms, None, "sem resposta, sem média");
        assert_eq!(mudo.grade, WifiGrade::Critical);
    }
}
