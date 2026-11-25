//! Alocação de IPs (IPAM) para a rede da VPN (§8.10.1).
//!
//! A unicidade real é garantida pelo índice `UNIQUE(network_id, ip_address)` em
//! `devices`. Este alocador apenas **sugere** o próximo livre e reexecuta a
//! operação quando duas requisições concorrentes escolhem o mesmo endereço
//! (matriz de paridade #42).

use std::{collections::HashSet, future::Future, net::Ipv4Addr};

use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QuerySelect};

use netmonitor_core::{
    models::{_entities::devices as devices_entity, devices},
    shared::{
        db_errors::is_unique_violation,
        errors::{AppError, AppResult},
    },
};

use crate::cidr::{first_usable_address, is_ip_in_cidr, iterate_usable_addresses, parse_cidr};

/// Nº máximo de tentativas antes de desistir por concorrência.
pub const MAX_ATTEMPTS: u32 = 10;

/// IPs já ocupados por dispositivos da rede.
async fn used_addresses<C: ConnectionTrait>(
    db: &C,
    network_id: i64,
) -> AppResult<HashSet<Ipv4Addr>> {
    #[derive(sea_orm::FromQueryResult)]
    struct Row {
        ip_address: Option<String>,
    }
    Ok(devices::Entity::find()
        .select_only()
        .column(devices_entity::Column::IpAddress)
        .filter(devices_entity::Column::NetworkId.eq(network_id))
        .into_model::<Row>()
        .all(db)
        .await?
        .into_iter()
        .filter_map(|row| row.ip_address)
        .filter_map(|raw| raw.trim().parse::<Ipv4Addr>().ok())
        .collect())
}

/// Próximo IP livre do CIDR, pulando o endereço do servidor (primeiro
/// utilizável) e os endereços explicitamente reservados.
///
/// # Errors
///
/// Falha quando o CIDR é inválido ou a faixa está esgotada.
pub async fn find_next_free<C: ConnectionTrait>(
    db: &C,
    network_id: i64,
    cidr: &str,
    reserved: &[Ipv4Addr],
) -> AppResult<Ipv4Addr> {
    let used = used_addresses(db, network_id).await?;
    let reserved_set: HashSet<Ipv4Addr> = reserved.iter().copied().collect();
    let server_address = first_usable_address(cidr)?;

    for candidate in iterate_usable_addresses(cidr)? {
        if candidate == server_address
            || reserved_set.contains(&candidate)
            || used.contains(&candidate)
        {
            continue;
        }
        return Ok(candidate);
    }

    Err(AppError::business_rule(format!(
        "Não há endereços livres disponíveis na faixa {cidr}"
    )))
}

/// Executa `operation` com um IP livre, repetindo com o próximo endereço quando
/// outra transação vence a corrida pelo mesmo IP.
///
/// # Errors
///
/// Propaga o erro da operação quando ele **não** é colisão de unicidade, e
/// desiste depois de [`MAX_ATTEMPTS`] colisões seguidas.
pub async fn allocate<C, T, F, Fut>(
    db: &C,
    network_id: i64,
    cidr: &str,
    reserved: &[Ipv4Addr],
    operation: F,
) -> AppResult<T>
where
    C: ConnectionTrait,
    F: Fn(Ipv4Addr) -> Fut,
    Fut: Future<Output = AppResult<T>>,
{
    let mut attempted: Vec<Ipv4Addr> = reserved.to_vec();

    for _ in 0..MAX_ATTEMPTS {
        let ip_address = find_next_free(db, network_id, cidr, &attempted).await?;
        match operation(ip_address).await {
            Ok(value) => return Ok(value),
            Err(AppError::Internal(error)) => {
                // Só colisão de unicidade justifica tentar de novo: qualquer
                // outra falha repetiria o mesmo erro dez vezes e mascararia a
                // causa real.
                let is_collision = error
                    .downcast_ref::<DbErr>()
                    .is_some_and(is_unique_violation);
                if !is_collision {
                    return Err(AppError::Internal(error));
                }
                attempted.push(ip_address);
            }
            Err(other) => return Err(other),
        }
    }

    Err(AppError::business_rule(format!(
        "Não foi possível alocar um IP em {cidr} após {MAX_ATTEMPTS} tentativas (concorrência excessiva)"
    )))
}

/// Valida se um IP informado manualmente é utilizável dentro do CIDR (sem
/// tocar no banco): pertence à faixa, não é rede, broadcast nem o IP do
/// servidor.
///
/// # Errors
///
/// Devolve 400 com a mensagem que o wizard exibe.
pub fn validate_manual_ip(cidr: &str, ip_address: Ipv4Addr) -> AppResult<()> {
    let range = parse_cidr(cidr)?;
    if !is_ip_in_cidr(ip_address, cidr)? {
        return Err(AppError::business_rule(format!(
            "O endereço {ip_address} não pertence à faixa {cidr}"
        )));
    }
    if range.usable_hosts == 0
        || ip_address == range.network_address
        || ip_address == range.broadcast_address
    {
        return Err(AppError::business_rule(format!(
            "O endereço {ip_address} é reservado (rede ou broadcast) na faixa {cidr}"
        )));
    }
    if ip_address == first_usable_address(cidr)? {
        return Err(AppError::business_rule(format!(
            "O endereço {ip_address} é reservado para o servidor VPN"
        )));
    }
    Ok(())
}

/// Valida um IP informado manualmente: precisa pertencer ao CIDR, ser um host
/// utilizável e estar livre.
///
/// # Errors
///
/// Devolve 400/409 com a mensagem que o wizard exibe.
pub async fn assert_available<C: ConnectionTrait>(
    db: &C,
    network_id: i64,
    cidr: &str,
    ip_address: Ipv4Addr,
) -> AppResult<()> {
    validate_manual_ip(cidr, ip_address)?;
    let taken = devices::Entity::find()
        .select_only()
        .column(devices_entity::Column::Id)
        .filter(devices_entity::Column::NetworkId.eq(network_id))
        .filter(devices_entity::Column::IpAddress.eq(ip_address.to_string()))
        .into_tuple::<i64>()
        .one(db)
        .await?
        .is_some();
    if taken {
        return Err(AppError::conflict(format!(
            "O endereço {ip_address} já está em uso nesta rede"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ip_manual_rejeita_rede_broadcast_e_servidor() {
        let ip = |s: &str| s.parse::<Ipv4Addr>().unwrap();
        assert!(validate_manual_ip("10.8.0.0/24", ip("10.8.0.0")).is_err());
        assert!(validate_manual_ip("10.8.0.0/24", ip("10.8.0.255")).is_err());
        assert!(validate_manual_ip("10.8.0.0/24", ip("10.8.0.1")).is_err());
        assert!(validate_manual_ip("10.8.0.0/24", ip("192.168.1.5")).is_err());
        assert!(validate_manual_ip("10.8.0.0/24", ip("10.8.0.2")).is_ok());
        assert!(validate_manual_ip("10.8.0.0/24", ip("10.8.0.254")).is_ok());
    }
}
