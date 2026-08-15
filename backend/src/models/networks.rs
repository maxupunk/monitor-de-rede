use sea_orm::entity::prelude::*;

pub use super::_entities::networks::{ActiveModel, Column, Entity, Model};

use crate::services::discovery::cidr_range::{is_scannable_cidr, parse_cidr_range};

pub type Networks = Entity;

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(self, _db: &C, insert: bool) -> std::result::Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if !insert && self.updated_at.is_unchanged() {
            let mut this = self;
            this.updated_at = sea_orm::ActiveValue::Set(chrono::Utc::now().into());
            Ok(this)
        } else {
            Ok(self)
        }
    }
}

impl Model {
    /// §6.1 — `scannable`: o CIDR cadastrado é utilizável numa varredura?
    ///
    /// A tela de Redes mostra "faixa inválida" a partir disto, então o valor
    /// acompanha **toda** resposta de rede, inclusive a do `PUT` (a store do
    /// frontend substitui a linha da tabela pela resposta).
    #[must_use]
    pub fn scannable(&self) -> bool {
        is_scannable_cidr(&self.cidr)
    }

    /// §6.1 — `usableHosts`: quantos endereços a varredura percorreria.
    ///
    /// `0` quando a faixa é inválida: a serialização não pode falhar por causa
    /// de um CIDR malformado no banco — a linha precisa aparecer na tela para
    /// que alguém consiga corrigi-la.
    #[must_use]
    pub fn usable_hosts(&self) -> u32 {
        parse_cidr_range(&self.cidr).map_or(0, |range| range.usable_hosts)
    }

    /// A varredura seria truncada em `MAX_SCAN_HOSTS`? Alimenta o aviso do
    /// `POST /api/networks/:id/scan` (§7.4).
    #[must_use]
    pub fn scan_truncated(&self) -> bool {
        parse_cidr_range(&self.cidr).is_ok_and(|range| range.truncated)
    }
}

impl Entity {}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn com_cidr(cidr: &str) -> Model {
        Model {
            id: 1,
            site_id: None,
            probe_id: None,
            name: "LAN".to_string(),
            cidr: cidr.to_string(),
            gateway: None,
            vlan: None,
            dns_servers: None,
            scan_enabled: true,
            scan_interval: 3600,
            active: true,
            last_scan_at: None,
            next_scan_at: None,
            created_at: Utc::now().into(),
            updated_at: Utc::now().into(),
        }
    }

    #[test]
    fn faixa_valida_expoe_tamanho() {
        let network = com_cidr("192.168.1.0/24");
        assert!(network.scannable());
        assert_eq!(network.usable_hosts(), 254);
        assert!(!network.scan_truncated());
    }

    #[test]
    fn faixa_invalida_nao_estoura_e_zera_o_tamanho() {
        let network = com_cidr("nem-cidr");
        assert!(!network.scannable());
        assert_eq!(network.usable_hosts(), 0);
        assert!(!network.scan_truncated());
    }

    #[test]
    fn faixa_grande_e_processada_em_lotes_sem_truncamento() {
        let network = com_cidr("10.0.0.0/16");
        assert!(network.scannable());
        assert!(!network.scan_truncated());
    }
}
