//! Os passos que faltam para uma perna rodar e que o operador faz ali mesmo,
//! no plano (ADR 015 §5): **instalar** o plugin de teste certo no equipamento
//! e **cadastrar o acesso** dele.
//!
//! O plugin sugerido sai da mesma régua que liga um embutido sozinho no
//! cadastro ([`compat::fits_platform`]): o sistema do equipamento é conhecido
//! e está na lista do plugin. Sistema desconhecido não ganha sugestão — o
//! motivo continua por extenso, sem botão.

use std::cmp::Reverse;

use sea_orm::{ColumnTrait, Condition, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder};

use super::{model::LinkTestFix, negotiate::Capabilities};
use crate::{
    models::{devices, plugins},
    services::{
        plugins::{
            compat::{self, Compat, DeviceFacts},
            credentials::{self, AccessGap},
            service,
        },
        shared::errors::AppResult,
    },
};

/// Um plugin de teste que o equipamento pode receber.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginOffer {
    pub plugin_id: i64,
    pub slug: String,
    pub name: String,
    /// O que a ponta passaria a servir e dirigir com ele.
    pub caps: Capabilities,
    /// O acesso que ainda faltaria para o plugin entrar no equipamento.
    pub access: Option<AccessGap>,
}

impl PluginOffer {
    /// Os passos para a ponta usar este plugin: instalar e, se faltar,
    /// cadastrar o acesso.
    #[must_use]
    pub fn fixes(&self, device_id: i64, device_name: &str) -> Vec<LinkTestFix> {
        let mut fixes = vec![LinkTestFix::InstallPlugin {
            device_id,
            device_name: device_name.to_owned(),
            plugin_id: self.plugin_id,
            plugin_slug: self.slug.clone(),
            plugin_name: self.name.clone(),
            message: format!("{device_name} precisa do plugin «{}»", self.name),
        }];
        fixes.extend(
            self.access
                .map(|gap| access_fix(device_id, device_name, gap)),
        );
        fixes
    }
}

/// O passo "cadastrar o acesso" de uma lacuna.
#[must_use]
pub fn access_fix(device_id: i64, device_name: &str, gap: AccessGap) -> LinkTestFix {
    LinkTestFix::AddCredential {
        device_id,
        device_name: device_name.to_owned(),
        transport: gap.kind(),
        message: gap.describe(device_name),
    }
}

/// Os plugins de teste que servem ao equipamento e ficam ativos ao instalar,
/// do mais para o menos indicado: o exclusivo dele, depois o validado, depois
/// o provável; empate fica pelo nome.
///
/// # Errors
///
/// Erro do banco ou manifesto gravado fora do formato.
pub async fn offers<C: ConnectionTrait>(
    db: &C,
    device: &devices::Model,
) -> AppResult<Vec<PluginOffer>> {
    service::ensure_builtins(db).await?;
    let facts = DeviceFacts::from_device(device);
    let rows = plugins::Entity::find()
        .filter(
            Condition::any()
                .add(plugins::Column::DeviceId.is_null())
                .add(plugins::Column::DeviceId.eq(device.id)),
        )
        .order_by_asc(plugins::Column::Name)
        .all(db)
        .await?;
    let mut ranked = Vec::new();
    for model in rows {
        if !service::active_once_installed(&model) {
            continue;
        }
        let package = service::package_of(&model)?;
        let Some(roles) = package.manifest.throughput.as_ref() else {
            continue;
        };
        let exclusive = model.device_id == Some(device.id);
        let level = if exclusive {
            Compat::Validated
        } else if compat::fits_platform(&package.manifest, &package.compatibility, &facts) {
            compat::evaluate(&package.manifest, &package.compatibility, &facts).level
        } else {
            continue;
        };
        ranked.push((
            (exclusive, level),
            PluginOffer {
                plugin_id: model.id,
                slug: model.slug.clone(),
                name: model.name.clone(),
                caps: Capabilities::from_roles(roles),
                access: credentials::access_gap(db, device.id, &package.manifest.transports)
                    .await?,
            },
        ));
    }
    // Estável: dentro do mesmo nível, a ordem do nome fica.
    ranked.sort_by_key(|(rank, _)| Reverse(*rank));
    Ok(ranked.into_iter().map(|(_, offer)| offer).collect())
}

#[cfg(test)]
mod tests {
    use migration::{Migrator, MigratorTrait};
    use sea_orm::{ActiveModelTrait, Database, DatabaseConnection, Set};

    use super::*;
    use crate::services::plugins::{
        credentials::{CredentialInput, SecretStorage},
        manifest::TransportKind,
    };

    async fn db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        db
    }

    async fn device(db: &DatabaseConnection, name: &str, os: Option<&str>) -> devices::Model {
        devices::ActiveModel {
            name: Set(name.into()),
            r#type: Set("router".into()),
            status: Set("online".into()),
            ip_address: Set(Some("10.0.0.9".into())),
            operating_system: Set(os.map(str::to_owned)),
            ..Default::default()
        }
        .insert(db)
        .await
        .unwrap()
    }

    async fn first_slug(db: &DatabaseConnection, os: Option<&str>) -> Option<String> {
        let device = device(db, "AP-terraço", os).await;
        offers(db, &device)
            .await
            .unwrap()
            .into_iter()
            .next()
            .map(|offer| offer.slug)
    }

    #[tokio::test]
    async fn sugere_o_plugin_de_teste_pelo_sistema() {
        let db = db().await;
        assert_eq!(
            first_slug(&db, Some("openwrt")).await.as_deref(),
            Some("openwrt-link-test")
        );
        assert_eq!(
            first_slug(&db, Some("routeros")).await.as_deref(),
            Some("mikrotik-routeros")
        );
        assert_eq!(
            first_slug(&db, Some("linux")).await.as_deref(),
            Some("linux-link-test")
        );
        assert_eq!(first_slug(&db, None).await, None, "sistema desconhecido");
    }

    #[tokio::test]
    async fn oferta_diz_o_acesso_que_falta() {
        let db = db().await;
        let ap = device(&db, "AP-terraço", Some("openwrt")).await;
        let offer = offers(&db, &ap).await.unwrap().remove(0);
        assert_eq!(offer.access, Some(AccessGap::Missing(TransportKind::Ssh)));
        let fixes = offer.fixes(ap.id, &ap.name);
        assert_eq!(fixes.len(), 2, "{fixes:?}");
        assert!(matches!(
            &fixes[0],
            LinkTestFix::InstallPlugin { plugin_slug, message, .. }
                if plugin_slug == "openwrt-link-test" && message.starts_with("AP-terraço precisa do plugin")
        ));
        assert!(matches!(
            &fixes[1],
            LinkTestFix::AddCredential { transport: TransportKind::Ssh, message, .. }
                if message == "AP-terraço está sem acesso SSH"
        ));

        credentials::upsert(
            &db,
            ap.id,
            CredentialInput {
                kind: TransportKind::Ssh,
                username: "root".into(),
                secret: Some("segredo".into()),
                storage: SecretStorage::Ask,
                port: None,
                https: None,
                via_probe_id: None,
            },
        )
        .await
        .unwrap();
        let offer = offers(&db, &ap).await.unwrap().remove(0);
        assert_eq!(offer.access, None);
        assert_eq!(offer.fixes(ap.id, &ap.name).len(), 1, "só instalar");
    }
}
