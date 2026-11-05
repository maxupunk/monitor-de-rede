//! Área VPN.

use async_trait::async_trait;
use netmonitor_core::areas::{Area, AreaPolicy, Blocker, Consequence, Evidence};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter};

use netmonitor_core::shared::text::count_noun;
use netmonitor_core::{
    models::_entities::{vpn_peers, vpn_servers},
    shared::errors::AppResult,
};

pub struct VpnPolicy;

#[async_trait]
impl AreaPolicy for VpnPolicy {
    fn area(&self) -> Area {
        Area::Vpn
    }

    async fn evidence(&self, db: &DatabaseConnection) -> AppResult<Evidence> {
        let servidores = vpn_servers::Entity::find().count(db).await?;
        Ok(if servidores > 0 {
            Evidence::because("servidor VPN configurado")
        } else {
            Evidence::none()
        })
    }

    async fn consequences(&self, db: &DatabaseConnection) -> AppResult<Vec<Consequence>> {
        let mut out = vec![Consequence::new(
            "O menu VPN e a aba VPN dos dispositivos somem.",
        )];
        let peers = vpn_peers::Entity::find().count(db).await?;
        if peers > 0 {
            out.push(Consequence::new(format!(
                "{} continua{} cadastrado{} — nada é apagado.",
                count_noun(peers, "dispositivo VPN", "dispositivos VPN"),
                if peers == 1 { "" } else { "m" },
                if peers == 1 { "" } else { "s" }
            )));
        }
        Ok(out)
    }

    /// Esconder um túnel vivo do administrador deixaria o acesso remoto aberto
    /// sem ninguém ver: primeiro se desativa o servidor, depois a área.
    async fn blocker(&self, db: &DatabaseConnection) -> AppResult<Option<Blocker>> {
        let ativos = vpn_servers::Entity::find()
            .filter(vpn_servers::Column::Active.eq(true))
            .count(db)
            .await?;
        Ok((ativos > 0).then(|| Blocker {
            message: "O servidor VPN está ativo. Desative-o em VPN › Servidor antes de desligar \
                      a área — esconder um túnel ativo deixaria o acesso remoto aberto sem \
                      ninguém ver."
                .to_owned(),
        }))
    }
}
