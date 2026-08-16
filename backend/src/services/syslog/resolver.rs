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
//!
//! **Origem mascarada por NAT inverte a ordem.** Quando o Docker reescreve o
//! remetente para o gateway da bridge (ver [`super::nat`]), o `source_ip` deixa
//! de identificar coisa alguma: o parque inteiro chega com um endereço só. Aí
//! os passos 1 e 3 não são apenas inúteis, são perigosos — casar pelo IP
//! atribuiria **todos** os roteadores ao mesmo dispositivo. Nesse caso o
//! resolvedor pula direto para o hostname, que é a única coisa que ainda
//! distingue um remetente do outro, e o vínculo manual passa a ser por nome
//! (`host:<hostname>`) em vez de por endereço.

use std::{
    collections::HashMap,
    net::IpAddr,
    sync::Arc,
    time::{Duration, Instant},
};

use ipnet::IpNet;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use tokio::sync::RwLock;

use super::nat::NatDetector;
use crate::{
    models::{
        _entities::{devices, networks},
        system_settings,
    },
    services::shared::errors::AppResult,
};

/// Chave em `system_settings` com o mapa `ip -> device_id` do *bind* manual.
///
/// Em `system_settings`, e não numa tabela própria: são poucas linhas, escritas
/// por ação manual do operador, e o resolvedor já consulta o banco principal —
/// uma tabela nova custaria migration, entrada no `CREATION_ORDER` e uma FK que
/// não pode existir (o log mora no outro banco).
pub const BINDINGS_KEY: &str = "syslog.source_bindings";

/// Prefixo que transforma uma chave de vínculo em vínculo **por hostname**.
///
/// O mapa é um só, `chave -> device_id`, com dois formatos de chave: um IP puro
/// (`192.168.88.1`) ou um nome prefixado (`host:MikroTik-CCR`). Um mapa
/// separado custaria uma segunda chave em `system_settings`, uma segunda
/// leitura por resolução e uma segunda rota de escrita para responder à mesma
/// pergunta. O prefixo não colide com IP porque `:` não aparece em IPv4 e IPv6
/// nunca começa por `host`.
pub const HOSTNAME_BIND_PREFIX: &str = "host:";

/// Monta a chave de vínculo de um hostname.
#[must_use]
pub fn hostname_bind_key(hostname: &str) -> String {
    format!("{HOSTNAME_BIND_PREFIX}{}", hostname.trim())
}

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
    /// Quem sabe dizer se um endereço é gateway de NAT em vez de remetente.
    nat: NatDetector,
}

impl Resolver {
    /// Detecta o NAT do ambiente uma vez e guarda o resultado — o gateway de um
    /// container não muda enquanto ele vive.
    #[must_use]
    pub fn create() -> Self {
        Self::with_nat(NatDetector::detect())
    }

    #[must_use]
    pub fn with_nat(nat: NatDetector) -> Self {
        Self {
            cache: Arc::default(),
            nat,
        }
    }

    /// O detector, para o `ingest` separar limitador e lista de fontes sem
    /// refazer a conta.
    #[must_use]
    pub const fn nat(&self) -> &NatDetector {
        &self.nat
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
        let resolucao = consulta(db, origem, hostname, &self.nat).await?;
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
    nat: &NatDetector,
) -> AppResult<Resolution> {
    let texto = origem.to_string();
    let nome = hostname.map(str::trim).filter(|valor| !valor.is_empty());
    let mapa = bindings(db).await?;

    // O *bind* manual vence tudo: é a palavra do operador sobre um caso que a
    // heurística errou ou não soube decidir. Sem esta precedência, um roteador
    // com `src-address` diferente continuaria caindo em "desconhecida" mesmo
    // depois de vinculado na tela.
    //
    // O vínculo por nome vem antes do vínculo por IP porque é o mais
    // específico dos dois: atrás de NAT o IP é o mesmo para todo mundo, e um
    // vínculo de endereço herdado de antes do mascaramento arrastaria o parque
    // inteiro para um dispositivo só.
    if let Some(nome) = nome {
        if let Some(id) = vinculado(db, &mapa, &hostname_bind_key(nome)).await? {
            return Ok(Resolution::Device(id));
        }
    }

    // Atrás de NAT o endereço de origem não identifica ninguém: pular os passos
    // que dependem dele é o que impede o parque inteiro de virar um dispositivo
    // só. Sobra o hostname — e, sem ele, a fonte é honestamente desconhecida.
    if nat.is_masked(origem) {
        return Ok(por_hostname(db, nome).await?.unwrap_or(Resolution::Unknown));
    }

    if let Some(id) = vinculado(db, &mapa, &texto).await? {
        return Ok(Resolution::Device(id));
    }

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
    if let Some(resolucao) = por_hostname(db, nome).await? {
        return Ok(resolucao);
    }

    // Sem dispositivo, mas dentro de uma rede que este sistema administra: é
    // fonte legítima e a linha é gravada sem vínculo.
    if let Some(id) = rede_que_contem(db, origem).await? {
        return Ok(Resolution::Network(id));
    }

    Ok(Resolution::Unknown)
}

/// O dispositivo de um vínculo manual, se a chave existir **e** o dispositivo
/// ainda existir. Vínculo órfão (dispositivo apagado) é ignorado em silêncio:
/// devolver o id apagado gravaria log apontando para lugar nenhum.
async fn vinculado(
    db: &DatabaseConnection,
    mapa: &HashMap<String, i64>,
    chave: &str,
) -> AppResult<Option<i64>> {
    let Some(device_id) = mapa.get(chave).copied() else {
        return Ok(None);
    };
    Ok(devices::Entity::find_by_id(device_id)
        .one(db)
        .await?
        .map(|dispositivo| dispositivo.id))
}

/// Casa o `HOSTNAME` do syslog com `devices.name`. Só decide quando o nome é
/// de um dispositivo só — dois aparelhos com o mesmo nome empatam, e chutar
/// aqui é o mesmo erro de chutar no IP repetido.
async fn por_hostname(
    db: &DatabaseConnection,
    hostname: Option<&str>,
) -> AppResult<Option<Resolution>> {
    let Some(nome) = hostname else {
        return Ok(None);
    };
    let por_nome = devices::Entity::find()
        .filter(devices::Column::Name.eq(nome))
        .all(db)
        .await?;
    Ok(match por_nome.len() {
        1 => Some(Resolution::Device(por_nome[0].id)),
        0 => None,
        _ => Some(Resolution::Ambiguous(
            por_nome.into_iter().map(|d| d.id).collect(),
        )),
    })
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

/// Lê o mapa de *binds* manuais. Ausente ou corrompido vira mapa vazio: um
/// JSON quebrado não pode calar a ingestão inteira.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn bindings(db: &DatabaseConnection) -> AppResult<HashMap<String, i64>> {
    let Some(linha) = system_settings::Entity::find()
        .filter(system_settings::Column::Key.eq(BINDINGS_KEY))
        .one(db)
        .await?
    else {
        return Ok(HashMap::new());
    };
    Ok(linha
        .value
        .and_then(|texto| serde_json::from_str(&texto).ok())
        .unwrap_or_default())
}

/// Grava (ou remove, com `device_id` nulo) o vínculo manual de uma origem.
///
/// A chave é um IP (`192.168.88.1`) ou um hostname prefixado
/// (`host:MikroTik-CCR`) — ver [`HOSTNAME_BIND_PREFIX`].
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn bind(db: &DatabaseConnection, chave: &str, device_id: Option<i64>) -> AppResult<()> {
    use sea_orm::{ActiveModelTrait, ActiveValue::Set};

    let mut mapa = bindings(db).await?;
    match device_id {
        Some(id) => mapa.insert(chave.to_owned(), id),
        None => mapa.remove(chave),
    };
    let valor = serde_json::to_string(&mapa)
        .map_err(|error| crate::services::shared::errors::AppError::Internal(error.into()))?;

    match system_settings::Entity::find()
        .filter(system_settings::Column::Key.eq(BINDINGS_KEY))
        .one(db)
        .await?
    {
        Some(linha) => {
            let mut ativo: system_settings::ActiveModel = linha.into();
            ativo.value = Set(Some(valor));
            ativo.update(db).await?;
        }
        None => {
            system_settings::ActiveModel {
                key: Set(BINDINGS_KEY.to_owned()),
                value: Set(Some(valor)),
                ..Default::default()
            }
            .insert(db)
            .await?;
        }
    }
    Ok(())
}

/// CIDR inválido no cadastro não derruba a ingestão — só não casa com nada.
fn cidr_contem(cidr: &str, endereco: IpAddr) -> bool {
    cidr.parse::<IpNet>()
        .is_ok_and(|rede| rede.contains(&endereco))
}

#[cfg(test)]
mod tests {
    use super::*;
    use migration::{Migrator, MigratorTrait};
    use sea_orm::{ActiveModelTrait, ActiveValue::Set};

    async fn inventario() -> DatabaseConnection {
        let db = sea_orm::Database::connect(
            sea_orm::ConnectOptions::new("sqlite::memory:".to_owned())
                .max_connections(1)
                .min_connections(1)
                .to_owned(),
        )
        .await
        .expect("banco principal");
        Migrator::up(&db, None).await.expect("migrations");
        db
    }

    async fn dispositivo(db: &DatabaseConnection, nome: &str, ip: &str) -> i64 {
        devices::ActiveModel {
            name: Set(nome.to_owned()),
            r#type: Set("router".to_owned()),
            status: Set("online".to_owned()),
            ip_address: Set(Some(ip.to_owned())),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("dispositivo")
        .id
    }

    fn ip(texto: &str) -> IpAddr {
        texto.parse().expect("ip do teste")
    }

    /// Resolve sem cache, desembrulhando o erro de banco.
    async fn resolve(db: &DatabaseConnection, origem: &str, hostname: Option<&str>) -> Resolution {
        consulta(db, ip(origem), hostname, &nat())
            .await
            .expect("consulta")
    }

    /// Detector sem nada declarado no ambiente — a heurística das faixas de
    /// bridge basta para `172.17.0.1` ser reconhecido como gateway, e é
    /// justamente ela que precisa ser exercitada aqui.
    fn nat() -> NatDetector {
        NatDetector::none()
    }

    #[tokio::test]
    async fn atras_do_nat_o_ip_de_origem_e_ignorado_e_o_hostname_decide() {
        // O cenário real: dois roteadores, um único IP de origem. Casar pelo
        // endereço atribuiria os dois ao mesmo dispositivo.
        let db = inventario().await;
        let borda = dispositivo(&db, "MikroTik-Borda", "192.168.88.1").await;
        let filial = dispositivo(&db, "MikroTik-Filial", "192.168.99.1").await;

        assert_eq!(
            resolve(&db, "172.17.0.1", Some("MikroTik-Borda")).await,
            Resolution::Device(borda)
        );
        assert_eq!(
            resolve(&db, "172.17.0.1", Some("MikroTik-Filial")).await,
            Resolution::Device(filial)
        );
    }

    #[tokio::test]
    async fn atras_do_nat_o_cidr_nao_serve_de_rede_conhecida() {
        // Sem esta regra bastaria alguém cadastrar `172.17.0.0/16` para todo
        // log do parque virar "rede conhecida" sem vínculo nenhum — pior que
        // descartar, porque some da lista de pendências.
        let db = inventario().await;
        networks::ActiveModel {
            name: Set("bridge do docker".to_owned()),
            cidr: Set("172.17.0.0/16".to_owned()),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("rede");

        assert_eq!(resolve(&db, "172.17.0.1", None).await, Resolution::Unknown);
    }

    #[tokio::test]
    async fn atras_do_nat_o_vinculo_e_por_nome_e_o_de_ip_nao_contamina() {
        // O operador que já tinha vinculado o IP do gateway antes do
        // mascaramento ser detectado não pode arrastar o parque inteiro.
        let db = inventario().await;
        let errado = dispositivo(&db, "Vizinho", "10.0.0.9").await;
        let certo = dispositivo(&db, "MikroTik-Borda", "192.168.88.1").await;
        bind(&db, "172.17.0.1", Some(errado))
            .await
            .expect("bind ip");
        bind(
            &db,
            &hostname_bind_key("Roteador-Sem-Cadastro"),
            Some(certo),
        )
        .await
        .expect("bind hostname");

        // O vínculo de IP do gateway é ignorado.
        assert_eq!(resolve(&db, "172.17.0.1", None).await, Resolution::Unknown);
        // O de hostname vale, inclusive para nome que não é de dispositivo.
        assert_eq!(
            resolve(&db, "172.17.0.1", Some("Roteador-Sem-Cadastro")).await,
            Resolution::Device(certo)
        );
    }

    #[tokio::test]
    async fn fora_do_nat_a_ordem_antiga_continua_valendo() {
        let db = inventario().await;
        let id = dispositivo(&db, "MikroTik-Borda", "192.168.88.1").await;
        assert_eq!(
            resolve(&db, "192.168.88.1", None).await,
            Resolution::Device(id),
            "o IP continua sendo o caminho preferencial fora do NAT"
        );
    }

    #[tokio::test]
    async fn hostname_repetido_empata_em_vez_de_chutar() {
        // Dois aparelhos com o mesmo nome é o mesmo problema do IP repetido.
        let db = inventario().await;
        let a = dispositivo(&db, "roteador", "10.0.0.1").await;
        let b = dispositivo(&db, "roteador", "10.0.1.1").await;
        let resolucao = resolve(&db, "172.17.0.1", Some("roteador")).await;
        assert_eq!(resolucao, Resolution::Ambiguous(vec![a, b]));
        assert_eq!(resolucao.device_id(), None, "ambígua não chuta vínculo");
    }

    #[tokio::test]
    async fn vinculo_para_dispositivo_apagado_e_ignorado() {
        let db = inventario().await;
        bind(&db, &hostname_bind_key("fantasma"), Some(4242))
            .await
            .expect("bind");
        assert_eq!(
            resolve(&db, "172.17.0.1", Some("fantasma")).await,
            Resolution::Unknown,
            "id órfão gravaria log apontando para lugar nenhum"
        );
    }

    #[test]
    fn a_chave_de_hostname_nao_colide_com_ip() {
        assert_eq!(hostname_bind_key(" MikroTik "), "host:MikroTik");
        assert!(hostname_bind_key("x").starts_with(HOSTNAME_BIND_PREFIX));
        assert!("192.168.1.1".parse::<IpAddr>().is_ok());
        assert!(hostname_bind_key("192.168.1.1").parse::<IpAddr>().is_err());
    }

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
