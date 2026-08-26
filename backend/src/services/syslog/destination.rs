//! Destino de Syslog aplicado a cada dispositivo.
//!
//! A execução remota e a memória do que foi executado são responsabilidades
//! diferentes. Este módulo cuida da segunda: normaliza o endereço, lembra-o no
//! dispositivo e o inclui no catálogo global sem duplicatas.

use std::net::IpAddr;

use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set, TransactionTrait};

use crate::{
    models::_entities::devices,
    services::{
        server_addresses::{self, CustomAddress, ServerAddress},
        shared::errors::{AppError, AppResult},
        syslog::{hints, NatDetector},
    },
};

/// Preferência já aplicada, pronta para vencer os palpites automáticos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedHint {
    pub address: String,
    pub address_id: Option<String>,
    pub reason: String,
}

/// Normaliza o valor que será escrito no equipamento e no banco.
///
/// IPs usam sua forma canônica; hostnames são minúsculos porque DNS não
/// diferencia caixa. O mesmo critério é usado para eliminar duplicatas.
#[must_use]
pub fn normalize(raw: &str) -> Option<String> {
    let clean = hints::sanitiza_endereco(Some(raw))?;
    Some(
        clean
            .parse::<IpAddr>()
            .map_or_else(|_| clean.to_ascii_lowercase(), |ip| ip.to_string()),
    )
}

#[must_use]
pub fn same_address(left: &str, right: &str) -> bool {
    match (normalize(left), normalize(right)) {
        (Some(left), Some(right)) => left == right,
        _ => false,
    }
}

/// Devolve a preferência salva e, quando possível, o id equivalente do
/// catálogo atual.
#[must_use]
pub fn saved_hint(device: &devices::Model, addresses: &[ServerAddress]) -> Option<SavedHint> {
    let address = normalize(device.syslog_server_address.as_deref()?)?;
    let address_id = addresses
        .iter()
        .find(|entry| {
            entry
                .value
                .as_deref()
                .is_some_and(|value| same_address(value, &address))
        })
        .map(|entry| entry.id.clone());
    Some(SavedHint {
        address,
        address_id,
        reason: "último endereço aplicado neste dispositivo".to_owned(),
    })
}

/// Lembra o endereço depois de a configuração remota ter sido aplicada.
///
/// A coluna do dispositivo e o catálogo global são gravados na mesma
/// transação. Um endereço que já existe, inclusive entre os detectados, não é
/// copiado para a lista personalizada.
pub async fn remember(
    db: &DatabaseConnection,
    device_id: i64,
    raw_address: &str,
    nat: &NatDetector,
) -> AppResult<String> {
    let address = normalize(raw_address).ok_or_else(|| {
        AppError::validation("Informe um endereço válido deste servidor para o Syslog.")
    })?;
    let resolved = server_addresses::list(db, nat).await?;
    let already_catalogued = resolved.iter().any(|entry| {
        entry
            .value
            .as_deref()
            .is_some_and(|value| same_address(value, &address))
    });

    let transaction = db.begin().await?;
    let device = devices::Entity::find_by_id(device_id)
        .one(&transaction)
        .await?
        .ok_or_else(|| AppError::not_found("Dispositivo não encontrado"))?;
    let mut active: devices::ActiveModel = device.into();
    active.syslog_server_address = Set(Some(address.clone()));
    active.update(&transaction).await?;

    if !already_catalogued {
        let mut stored = server_addresses::stored(&transaction).await?;
        let duplicated = stored
            .custom
            .iter()
            .any(|entry| same_address(&entry.value, &address));
        if !duplicated {
            stored.custom.push(CustomAddress {
                id: String::new(),
                label: format!("Outro endereço — {address}"),
                value: address.clone(),
            });
            server_addresses::save(&transaction, stored).await?;
        }
    }

    transaction.commit().await?;
    Ok(address)
}

#[cfg(test)]
mod tests {
    use migration::{Migrator, MigratorTrait};
    use sea_orm::{ActiveModelTrait, ConnectOptions, Database};

    use super::*;

    async fn database() -> DatabaseConnection {
        let db = Database::connect(
            ConnectOptions::new("sqlite::memory:".to_owned())
                .max_connections(1)
                .min_connections(1)
                .to_owned(),
        )
        .await
        .expect("banco");
        Migrator::up(&db, None).await.expect("migrations");
        db
    }

    async fn device(db: &DatabaseConnection, name: &str) -> devices::Model {
        devices::ActiveModel {
            name: Set(name.to_owned()),
            r#type: Set("router".to_owned()),
            status: Set("unknown".to_owned()),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("dispositivo")
    }

    #[test]
    fn normaliza_ips_e_hostnames_para_deduplicar() {
        assert_eq!(normalize(" 2001:0db8::1 ").as_deref(), Some("2001:db8::1"));
        assert!(same_address("NETMONITOR.EXEMPLO", "netmonitor.exemplo"));
    }

    #[tokio::test]
    async fn lembra_por_dispositivo_e_cataloga_uma_unica_vez() {
        let db = database().await;
        let first = device(&db, "primeiro").await;
        let second = device(&db, "segundo").await;

        remember(&db, first.id, " NETMONITOR.EXEMPLO ", &NatDetector::none())
            .await
            .expect("lembrar primeiro");
        remember(&db, second.id, "netmonitor.exemplo", &NatDetector::none())
            .await
            .expect("lembrar segundo");

        let first = devices::Entity::find_by_id(first.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        let second = devices::Entity::find_by_id(second.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            first.syslog_server_address.as_deref(),
            Some("netmonitor.exemplo")
        );
        assert_eq!(
            second.syslog_server_address.as_deref(),
            Some("netmonitor.exemplo")
        );

        let stored = server_addresses::stored(&db).await.expect("catálogo");
        assert_eq!(stored.custom.len(), 1);
    }

    #[tokio::test]
    async fn preferencia_de_um_dispositivo_nao_vaza_para_outro() {
        let db = database().await;
        let first = device(&db, "primeiro").await;
        let second = device(&db, "segundo").await;
        remember(&db, first.id, "192.0.2.20", &NatDetector::none())
            .await
            .expect("lembrar");

        let first = devices::Entity::find_by_id(first.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        let second = devices::Entity::find_by_id(second.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        let addresses = server_addresses::list(&db, &NatDetector::none())
            .await
            .expect("endereços");

        assert_eq!(
            saved_hint(&first, &addresses).unwrap().address,
            "192.0.2.20"
        );
        assert!(saved_hint(&second, &addresses).is_none());
    }

    #[tokio::test]
    async fn falha_antes_de_encontrar_o_dispositivo_nao_altera_o_catalogo() {
        let db = database().await;
        let result = remember(&db, 999, "192.0.2.30", &NatDetector::none()).await;
        assert!(result.is_err());
        assert!(server_addresses::stored(&db)
            .await
            .expect("catálogo")
            .custom
            .is_empty());
    }
}
