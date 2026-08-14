//! Alocação de IPs (IPAM) para a rede da VPN (§8.10.1).
//!
//! A unicidade real é garantida pelo índice `UNIQUE(network_id, ip_address)` em
//! `devices`. Este alocador apenas **sugere** o próximo livre e reexecuta a
//! operação quando duas requisições concorrentes escolhem o mesmo endereço
//! (matriz de paridade #42).

use std::{collections::HashSet, future::Future, net::Ipv4Addr};

use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QuerySelect};

use crate::{
    models::{_entities::devices as devices_entity, devices},
    services::{
        shared::errors::{AppError, AppResult},
        vpn::cidr::{first_usable_address, is_ip_in_cidr, iterate_usable_addresses},
    },
};

/// Nº máximo de tentativas antes de desistir por concorrência.
pub const MAX_ATTEMPTS: u32 = 10;

/// Identifica violação de unicidade em PostgreSQL (23505) e SQLite
/// (`SQLITE_CONSTRAINT_UNIQUE`), sem acoplar o serviço ao driver.
///
/// O `sea-orm` não expõe o código do erro de forma portátil, então a checagem
/// é pelo texto — o mesmo critério (e a mesma fragilidade) do backend anterior.
#[must_use]
pub fn is_unique_violation(error: &DbErr) -> bool {
    let message = error.to_string().to_lowercase();
    message.contains("23505")
        || message.contains("sqlite_constraint_unique")
        || message.contains("unique constraint")
        || message.contains("duplicate key")
}

/// IPs já ocupados por dispositivos da rede.
async fn used_addresses<C: ConnectionTrait>(db: &C, network_id: i64) -> AppResult<HashSet<String>> {
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
    let server_address = first_usable_address(cidr)?;

    for candidate in iterate_usable_addresses(cidr)? {
        if candidate == server_address || reserved.contains(&candidate) {
            continue;
        }
        if !used.contains(&candidate.to_string()) {
            return Ok(candidate);
        }
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

/// Valida um IP informado manualmente: precisa pertencer ao CIDR e estar livre.
///
/// # Errors
///
/// Devolve 400 com a mensagem que o wizard exibe.
pub async fn assert_available<C: ConnectionTrait>(
    db: &C,
    network_id: i64,
    cidr: &str,
    ip_address: Ipv4Addr,
) -> AppResult<()> {
    if !is_ip_in_cidr(ip_address, cidr)? {
        return Err(AppError::business_rule(format!(
            "O endereço {ip_address} não pertence à faixa {cidr}"
        )));
    }
    if ip_address == first_usable_address(cidr)? {
        return Err(AppError::business_rule(format!(
            "O endereço {ip_address} é reservado para o servidor VPN"
        )));
    }
    if used_addresses(db, network_id)
        .await?
        .contains(&ip_address.to_string())
    {
        return Err(AppError::business_rule(format!(
            "O endereço {ip_address} já está em uso nesta rede"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconhece_colisao_de_unicidade_nos_dois_bancos() {
        assert!(is_unique_violation(&DbErr::Custom(
            "error returned from database: (code: 2067) UNIQUE constraint failed: devices.ip_address"
                .into()
        )));
        assert!(is_unique_violation(&DbErr::Custom(
            "duplicate key value violates unique constraint (23505)".into()
        )));
    }

    #[test]
    fn erro_comum_de_banco_nao_e_confundido_com_colisao() {
        assert!(!is_unique_violation(&DbErr::Custom(
            "connection closed".into()
        )));
        assert!(!is_unique_violation(&DbErr::RecordNotFound("sumiu".into())));
    }
}
