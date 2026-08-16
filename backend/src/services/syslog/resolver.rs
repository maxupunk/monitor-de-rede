//! Resolve o IP de origem para um dispositivo cadastrado.
//!
//! É aqui que mora a regra que protege o disco: **fonte que não resolve não
//! grava**. Sem ela, um host solto na rede — ou um scanner — enche o volume
//! sozinho.
//!
//! A ordem de tentativa é do mais específico para o menos:
//!
//! 1. `source_ip` bate com `devices.ip_address`;
//! 2. o `HOSTNAME` do syslog bate com `devices.name` (salva o roteador que
//!    envia por um IP diferente do cadastrado — RouterOS com `src-address`);
//! 3. o IP cai no CIDR de alguma `networks` cadastrada: fonte legítima, sem
//!    dispositivo conhecido;
//! 4. nada disso: fonte desconhecida.
//!
//! **IP não é único em `devices`.** O índice único é
//! `devices_network_ip_unique`, sobre `(network_id, ip_address)`: dois
//! dispositivos em redes diferentes podem legitimamente ter `192.168.1.1`.
//! Quando isso acontece o desempate é pelo CIDR da rede; se ainda empatar, o
//! resolvedor **não adivinha**. Vincular ao dispositivo errado é pior do que
//! não vincular — contamina a aba de logs do aparelho e, na Fase 6, dispararia
//! alerta no alvo errado.

use std::{
    collections::HashMap,
    net::IpAddr,
    sync::Arc,
    time::{Duration, Instant},
};

use ipnet::IpNet;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use tokio::sync::RwLock;

use crate::{
    models::_entities::{devices, networks},
    services::shared::errors::AppResult,
};

/// Por quanto tempo uma resolução vale sem reconsultar o banco.
///
/// Consultar `devices` a cada linha, a 200 msg/s, seria desperdício puro: o
/// inventário muda em minutos, não em milissegundos. Trinta segundos é curto o
/// bastante para um dispositivo recém-cadastrado começar a vincular sem
/// reiniciar nada.
const TTL: Duration = Duration::from_secs(30);

/// O que o resolvedor concluiu sobre uma origem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// Vinculado a um dispositivo.
    Device(i64),
    /// Dentro de uma rede cadastrada, sem dispositivo correspondente. Fonte
    /// legítima: grava com `device_id` nulo.
    Network(i64),
    /// O IP existe em mais de um dispositivo e o CIDR não desempatou. Grava sem
    /// vínculo e espera o *bind* manual.
    Ambiguous(Vec<i64>),
    /// Não bate com nada cadastrado.
    Unknown,
}

impl Resolution {
    /// Se a linha pode ser gravada — a regra do §5 do roadmap.
    #[must_use]
    pub const fn is_known(&self) -> bool {
        !matches!(self, Self::Unknown)
    }

    /// O `device_id` que vai para a coluna. Ambíguo grava sem vínculo de
    /// propósito.
    #[must_use]
    pub const fn device_id(&self) -> Option<i64> {
        match self {
            Self::Device(id) => Some(*id),
            _ => None,
        }
    }
}

/// Cache de resoluções, compartilhado entre o listener UDP e o TCP.
#[derive(Clone, Default)]
pub struct Resolver {
    cache: Arc<RwLock<HashMap<String, (Resolution, Instant)>>>,
}

impl Resolver {
    #[must_use]
    pub fn create() -> Self {
        Self::default()
    }

    /// Resolve, usando o cache quando ele ainda vale.
    ///
    /// A chave inclui o hostname porque ele participa da decisão: duas linhas
    /// do mesmo IP com hostnames diferentes são casos diferentes.
    ///
    /// # Errors
    ///
    /// Propaga erro do banco.
    pub async fn resolve(
        &self,
        db: &DatabaseConnection,
        origem: IpAddr,
        hostname: Option<&str>,
    ) -> AppResult<Resolution> {
        let chave = format!("{origem}|{}", hostname.unwrap_or_default());
        if let Some((resolucao, gravado_em)) = self.cache.read().await.get(&chave) {
            if gravado_em.elapsed() < TTL {
                return Ok(resolucao.clone());
            }
        }
        let resolucao = consulta(db, origem, hostname).await?;
        self.cache
            .write()
            .await
            .insert(chave, (resolucao.clone(), Instant::now()));
        Ok(resolucao)
    }

    /// Esvazia o cache. Usado quando o inventário muda por ação do usuário —
    /// cadastro de dispositivo, *bind* manual de IP.
    pub async fn invalidate(&self) {
        self.cache.write().await.clear();
    }
}

/// A consulta em si, sem cache. Separada para o teste poder exercitá-la direto.
async fn consulta(
    db: &DatabaseConnection,
    origem: IpAddr,
    hostname: Option<&str>,
) -> AppResult<Resolution> {
    let texto = origem.to_string();

    let candidatos = devices::Entity::find()
        .filter(devices::Column::IpAddress.eq(texto.clone()))
        .all(db)
        .await?;

    match candidatos.len() {
        1 => return Ok(Resolution::Device(candidatos[0].id)),
        0 => {}
        _ => return desempata(db, &candidatos, origem).await,
    }

    // O roteador pode enviar por um IP que não é o cadastrado (RouterOS com
    // `src-address`, interface secundária). O nome que ele se dá continua
    // valendo.
    if let Some(nome) = hostname.filter(|valor| !valor.is_empty()) {
        let por_nome = devices::Entity::find()
            .filter(devices::Column::Name.eq(nome))
            .all(db)
            .await?;
        if por_nome.len() == 1 {
            return Ok(Resolution::Device(por_nome[0].id));
        }
    }

    // Sem dispositivo, mas dentro de uma rede que este sistema administra: é
    // fonte legítima e a linha é gravada sem vínculo.
    if let Some(id) = rede_que_contem(db, origem).await? {
        return Ok(Resolution::Network(id));
    }

    Ok(Resolution::Unknown)
}

/// Mesmo IP em mais de um dispositivo: desempata pelo CIDR da rede de cada um.
async fn desempata(
    db: &DatabaseConnection,
    candidatos: &[devices::Model],
    origem: IpAddr,
) -> AppResult<Resolution> {
    let mut compativeis = Vec::new();
    for candidato in candidatos {
        let Some(network_id) = candidato.network_id else {
            continue;
        };
        let Some(rede) = networks::Entity::find_by_id(network_id).one(db).await? else {
            continue;
        };
        if cidr_contem(&rede.cidr, origem) {
            compativeis.push(candidato.id);
        }
    }
    if compativeis.len() == 1 {
        return Ok(Resolution::Device(compativeis[0]));
    }
    // Zero compatíveis é tão ambíguo quanto dois: em nenhum dos casos há
    // motivo para preferir um dispositivo ao outro.
    let ids = if compativeis.is_empty() {
        candidatos.iter().map(|d| d.id).collect()
    } else {
        compativeis
    };
    Ok(Resolution::Ambiguous(ids))
}

/// A primeira rede cadastrada cujo CIDR contém o IP.
async fn rede_que_contem(db: &DatabaseConnection, origem: IpAddr) -> AppResult<Option<i64>> {
    let redes = networks::Entity::find().all(db).await?;
    Ok(redes
        .into_iter()
        .find(|rede| cidr_contem(&rede.cidr, origem))
        .map(|rede| rede.id))
}

/// CIDR inválido no cadastro não derruba a ingestão — só não casa com nada.
fn cidr_contem(cidr: &str, endereco: IpAddr) -> bool {
    cidr.parse::<IpNet>()
        .is_ok_and(|rede| rede.contains(&endereco))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_cidr_decide_a_pertinencia_e_lixo_no_cadastro_nao_derruba() {
        let ip: IpAddr = "192.168.1.50".parse().unwrap();
        assert!(cidr_contem("192.168.1.0/24", ip));
        assert!(!cidr_contem("10.0.0.0/8", ip));
        assert!(!cidr_contem("não é cidr", ip), "CIDR inválido só não casa");
        assert!(!cidr_contem("", ip));
    }

    #[test]
    fn so_fonte_conhecida_grava() {
        assert!(Resolution::Device(1).is_known());
        assert!(Resolution::Network(2).is_known());
        // Ambígua é conhecida: o IP existe no inventário, só não se sabe em
        // qual dispositivo. Descartar a linha esconderia log de aparelho
        // cadastrado.
        assert!(Resolution::Ambiguous(vec![1, 2]).is_known());
        assert!(!Resolution::Unknown.is_known());
    }

    #[test]
    fn so_o_vinculo_certo_preenche_device_id() {
        assert_eq!(Resolution::Device(7).device_id(), Some(7));
        // Ambígua grava sem vínculo: chutar contaminaria a aba do aparelho e,
        // na Fase 6, dispararia alerta no alvo errado.
        assert_eq!(Resolution::Ambiguous(vec![1, 2]).device_id(), None);
        assert_eq!(Resolution::Network(3).device_id(), None);
        assert_eq!(Resolution::Unknown.device_id(), None);
    }
}
