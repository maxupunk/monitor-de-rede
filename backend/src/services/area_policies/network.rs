//! Área Rede.

use async_trait::async_trait;
use netmonitor_core::areas::{Area, AreaPolicy, Consequence, Evidence};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter};

use netmonitor_core::shared::text::count_noun;

use crate::{
    models::_entities::{
        device_links, device_plugin_installs, devices, link_test_schedules, monitors, networks,
        probes,
    },
    services::{agents::service::is_agent, shared::errors::AppResult},
};
use netmonitor_core::areas::monitors_keep_running;

/// Tipos de monitor que só existem com equipamento de rede.
const NETWORK_MONITOR_TYPES: [&str; 1] = ["snmp"];

pub struct NetworkPolicy;

#[async_trait]
impl AreaPolicy for NetworkPolicy {
    fn area(&self) -> Area {
        Area::Network
    }

    async fn evidence(&self, db: &DatabaseConnection) -> AppResult<Evidence> {
        let redes = networks::Entity::find().count(db).await?;
        if redes > 0 {
            return Ok(Evidence::because(format!(
                "{} cadastrada{}",
                count_noun(redes, "rede IP", "redes IP"),
                if redes == 1 { "" } else { "s" }
            )));
        }
        // O dispositivo do sistema (`system_key`) nasce sozinho: não conta.
        let snmp = devices::Entity::find()
            .filter(devices::Column::SystemKey.is_null())
            .filter(devices::Column::SnmpEnabled.eq(true))
            .count(db)
            .await?;
        if snmp > 0 {
            return Ok(Evidence::because(format!(
                "{} com SNMP",
                count_noun(snmp, "dispositivo", "dispositivos")
            )));
        }
        if device_plugin_installs::Entity::find().count(db).await? > 0 {
            return Ok(Evidence::because("plugin de dispositivo instalado"));
        }
        let sondas = probes::Entity::find()
            .filter(probes::Column::Status.ne(crate::models::probes::STATUS_REVOKED))
            .all(db)
            .await?
            .iter()
            .filter(|probe| !is_agent(probe))
            .count();
        if sondas > 0 {
            return Ok(Evidence::because("sonda de rede cadastrada"));
        }
        let manuais = device_links::Entity::find()
            .filter(device_links::Column::LinkType.eq("manual"))
            .count(db)
            .await?;
        if manuais > 0 {
            return Ok(Evidence::because("enlace cadastrado à mão na topologia"));
        }
        Ok(Evidence::none())
    }

    async fn consequences(&self, db: &DatabaseConnection) -> AppResult<Vec<Consequence>> {
        let mut out = vec![Consequence::new(
            "Topologia, descoberta, redes IP, sites, testes de velocidade, Wi-Fi, sondas, \
             diagnóstico e plugins de dispositivo saem do menu e da IA.",
        )];
        let varredura = networks::Entity::find()
            .filter(networks::Column::ScanEnabled.eq(true))
            .count(db)
            .await?;
        if varredura > 0 {
            out.push(Consequence::new(format!(
                "A varredura periódica de {} fica pausada.",
                count_noun(varredura, "rede", "redes")
            )));
        }
        let enlaces = link_test_schedules::Entity::find()
            .filter(link_test_schedules::Column::Enabled.eq(true))
            .count(db)
            .await?;
        if enlaces > 0 {
            out.push(Consequence::new(format!(
                "{} fica{} pausado{}.",
                count_noun(
                    enlaces,
                    "teste de velocidade agendado",
                    "testes de velocidade agendados"
                ),
                if enlaces == 1 { "" } else { "m" },
                if enlaces == 1 { "" } else { "s" }
            )));
        }
        let snmp = monitors::Entity::find()
            .filter(monitors::Column::Type.is_in(NETWORK_MONITOR_TYPES))
            .count(db)
            .await?;
        out.extend(monitors_keep_running(
            snmp,
            "monitor SNMP",
            "monitores SNMP",
        ));
        Ok(out)
    }
}
