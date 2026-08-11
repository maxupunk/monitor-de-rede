//! Registro de servidores DNS persistidos.

use crate::{
    models::dns_servers,
    services::{
        network_tools::dns::latency::{DnsProtocol, DnsServerTarget, DEFAULT_DNS_SERVERS},
        shared::errors::AppResult,
    },
};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

pub struct DnsServerRegistry;
impl DnsServerRegistry {
    /// Semeia apenas um banco vazio; nunca recria uma escolha removida pelo usuário.
    pub async fn ensure_defaults(db: &sea_orm::DatabaseConnection) -> AppResult<()> {
        if dns_servers::Entity::find().one(db).await?.is_some() {
            return Ok(());
        }
        for (name, address) in DEFAULT_DNS_SERVERS {
            dns_servers::ActiveModel {
                name: Set((*name).into()),
                address: Set((*address).into()),
                protocol: Set("udp".into()),
                is_default: Set(true),
                description: Set(Some("Resolvedor público padrão".into())),
                ..Default::default()
            }
            .insert(db)
            .await?;
        }
        Ok(())
    }
    pub async fn benchmark_targets(
        db: &sea_orm::DatabaseConnection,
    ) -> AppResult<Vec<DnsServerTarget>> {
        let defaults = dns_servers::Entity::find()
            .filter(dns_servers::Column::IsDefault.eq(true))
            .all(db)
            .await?;
        let rows = if defaults.is_empty() {
            dns_servers::Entity::find().all(db).await?
        } else {
            defaults
        };
        Ok(rows
            .into_iter()
            .filter_map(|row| {
                DnsProtocol::parse(Some(&row.protocol))
                    .ok()
                    .map(|protocol| DnsServerTarget {
                        server: row.address,
                        label: Some(row.name),
                        protocol,
                    })
            })
            .collect())
    }
}
