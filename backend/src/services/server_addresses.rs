//! Por onde os equipamentos alcançam este servidor.
//!
//! # O conceito
//!
//! Um servidor, várias portas de entrada. Quando se configura um roteador para
//! mandar syslog (ou qualquer outra coisa) para o NetMonitor, a pergunta é
//! sempre a mesma — *para qual endereço?* — e a resposta honesta é **depende de
//! onde o equipamento está**:
//!
//! | de onde o equipamento fala | por onde ele alcança |
//! |---|---|
//! | mesma rede local | o IP deste servidor na LAN |
//! | atrás do túnel | o IP deste servidor dentro da VPN |
//! | outro local, pela internet | o IP público ou o DDNS |
//!
//! Sem um lugar para isso, cada tela reinventava um palpite. A tela de logs
//! mandava o host da barra de endereços do navegador — que é `localhost` para
//! quem abre a interface na própria máquina, e `localhost` gravado num roteador
//! o faz mandar o log para si mesmo, sem erro e sem nada chegando.
//!
//! # A lista é descoberta, não digitada
//!
//! Três dos endereços o sistema já conhece, e obrigar o operador a redigitá-los
//! seria pedir que ele repita o que a máquina sabe:
//!
//! - **Rede local** — a rota de saída deste servidor;
//! - **Túnel VPN** — o primeiro endereço útil da faixa do WireGuard, que é o
//!   mesmo cálculo que o `server_service` usa para configurar a interface;
//! - **Internet** — o `public_endpoint` do servidor VPN, que o operador já teve
//!   de preencher para os peers funcionarem.
//!
//! O que se guarda é só o que o operador **acrescentou ou corrigiu**. Guardar
//! também os detectados os congelaria: o IP da LAN muda, e a lista continuaria
//! mostrando o antigo com toda a confiança.
//!
//! # O que se guarda
//!
//! Em `system_settings`, chave [`STORAGE_KEY`], pelo mesmo motivo do
//! `syslog.source_bindings`: são poucas linhas, escritas por ação manual, sem
//! FK para lugar nenhum — e a tabela já entra no backup e no `truncate` de
//! teste sem trabalho adicional.

use std::{collections::HashMap, net::IpAddr};

use sea_orm::{DatabaseConnection, EntityTrait};
use serde::{Deserialize, Serialize};

use crate::{
    models::{
        _entities::{devices, networks, vpn_servers},
        system_settings,
    },
    services::{
        devices::access,
        shared::errors::{AppError, AppResult},
        syslog::{hints, nat::NatDetector},
        vpn::cidr::first_usable_address,
    },
};

/// Chave em `system_settings`.
pub const STORAGE_KEY: &str = "server.addresses";

/// Destino usado só para descobrir a rota de saída.
///
/// É a faixa de documentação da RFC 5737: não existe host lá, e **nada é
/// transmitido** — o `connect` de UDP apenas faz o kernel consultar a tabela de
/// rotas. Usar um IP real (um resolvedor público, por exemplo) daria o mesmo
/// resultado e insinuaria um tráfego que não acontece.
const DESTINO_DE_SONDAGEM: &str = "192.0.2.1";

/// Os três tipos que o sistema conhece, mais o que o operador inventar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressKind {
    Lan,
    Vpn,
    Public,
    Custom,
}

impl AddressKind {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Lan => "lan",
            Self::Vpn => "vpn",
            Self::Public => "public",
            Self::Custom => "custom",
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Lan => "Rede local",
            Self::Vpn => "Túnel VPN",
            Self::Public => "Internet",
            Self::Custom => "Outro endereço",
        }
    }

    /// A frase que explica **quando** usar este endereço. É ela que carrega o
    /// conceito na tela — sem ela a lista vira três IPs sem critério.
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Lan => "Para equipamentos na mesma rede que este servidor",
            Self::Vpn => "Para equipamentos que chegam pelo túnel WireGuard",
            Self::Public => "Para equipamentos em outro local, que chegam pela internet",
            Self::Custom => "Endereço definido por você",
        }
    }
}

/// Uma entrada da lista, já resolvida.
#[derive(Debug, Clone)]
pub struct ServerAddress {
    pub id: String,
    pub kind: AddressKind,
    pub label: String,
    pub description: String,
    /// O que vale hoje: a correção do operador, ou o detectado.
    pub value: Option<String>,
    /// O que o servidor descobriu sozinho, para a tela poder oferecer
    /// "voltar ao detectado" depois de uma correção.
    pub detected: Option<String>,
    pub overridden: bool,
    /// De onde veio o valor. A tela mostra: palpite apresentado como certeza é
    /// pior do que campo vazio.
    pub source: String,
}

/// O documento guardado — só o que o operador acrescentou ou corrigiu.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredAddresses {
    /// Correções sobre os detectados, por tipo (`lan`, `vpn`, `public`).
    #[serde(default)]
    pub overrides: HashMap<String, String>,
    #[serde(default)]
    pub custom: Vec<CustomAddress>,
    /// Qual usar quando nada indicar outro. Opcional de propósito: sem escolha
    /// explícita, a sugestão vem da rota até o equipamento, que acerta mais.
    #[serde(default)]
    pub preferred_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomAddress {
    pub id: String,
    pub label: String,
    pub value: String,
}

/// Lê o documento. JSON corrompido vira documento vazio: um valor ilegível não
/// pode impedir a tela de abrir nem apagar os detectados.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn stored(db: &DatabaseConnection) -> AppResult<StoredAddresses> {
    Ok(system_settings::Model::get(db, STORAGE_KEY)
        .await?
        .and_then(|linha| linha.value)
        .and_then(|texto| serde_json::from_str(&texto).ok())
        .unwrap_or_default())
}

/// Grava o documento, validando antes.
///
/// # Errors
///
/// Endereço inutilizável, rótulo vazio, ou erro do banco.
pub async fn save(db: &DatabaseConnection, mut documento: StoredAddresses) -> AppResult<()> {
    // Descartar os brancos **antes** de validar: correção em branco significa
    // "voltar ao detectado", e validá-la primeiro tornaria isso impossível —
    // string vazia não passa em nenhum crivo de endereço.
    documento
        .overrides
        .retain(|_, valor| !valor.trim().is_empty());

    for (tipo, valor) in &documento.overrides {
        valida_endereco(
            valor,
            &format!("o endereço de \"{}\"", rotulo_do_tipo(tipo)),
        )?;
    }
    for entrada in &mut documento.custom {
        if entrada.label.trim().is_empty() {
            return Err(AppError::validation(
                "Todo endereço personalizado precisa de um nome que diga quando usá-lo.",
            ));
        }
        entrada.label = entrada.label.trim().to_owned();
        valida_endereco(
            &entrada.value,
            &format!("o endereço de \"{}\"", entrada.label),
        )?;
        entrada.value = entrada.value.trim().to_owned();
        if entrada.id.trim().is_empty() {
            entrada.id = format!("custom:{}", uuid::Uuid::new_v4());
        }
    }
    let texto = serde_json::to_string(&documento)
        .map_err(|error| AppError::Internal(anyhow::Error::new(error)))?;
    system_settings::Model::set(db, STORAGE_KEY, Some(texto)).await?;
    Ok(())
}

/// Apaga a correção de um tipo quando ela virou cópia do valor detectado.
///
/// Chamada quando a fonte do detectado muda — hoje, quando o servidor VPN grava
/// um `public_endpoint` novo. Sem isto a correção sobrevive como cópia inerte, e
/// **na mudança seguinte ela vence em silêncio**: os peers passariam a usar o
/// endpoint novo enquanto o syslog continuaria apontando para o antigo, sem
/// nada na tela indicando que as duas coisas se separaram.
///
/// Devolve se algo foi apagado.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn forget_override_if(
    db: &DatabaseConnection,
    tipo: &str,
    detectado: &str,
) -> AppResult<bool> {
    let detectado = detectado.trim();
    if detectado.is_empty() {
        return Ok(false);
    }
    let mut documento = stored(db).await?;
    let redundante = documento
        .overrides
        .get(tipo)
        .is_some_and(|valor| valor.trim() == detectado);
    if !redundante {
        return Ok(false);
    }
    documento.overrides.remove(tipo);
    save(db, documento).await?;
    Ok(true)
}

fn rotulo_do_tipo(tipo: &str) -> &'static str {
    match tipo {
        "lan" => AddressKind::Lan.label(),
        "vpn" => AddressKind::Vpn.label(),
        "public" => AddressKind::Public.label(),
        _ => AddressKind::Custom.label(),
    }
}

/// Recusa o que não serve para ser gravado num equipamento.
///
/// `localhost` é o caso que motivou tudo isto: aceito por qualquer comando de
/// configuração, e faz o aparelho mandar o tráfego para si mesmo.
fn valida_endereco(valor: &str, campo: &str) -> AppResult<()> {
    if hints::sanitiza_endereco(Some(valor)).is_none() {
        return Err(AppError::validation(format!(
            "`{}` não serve como {campo}: é um endereço que aponta o equipamento para ele mesmo. \
             Informe o IP ou o nome pelo qual ele alcança este servidor.",
            valor.trim()
        )));
    }
    Ok(())
}

/// Monta a lista completa: detectados, corrigidos e personalizados.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn list(db: &DatabaseConnection, nat: &NatDetector) -> AppResult<Vec<ServerAddress>> {
    let documento = stored(db).await?;
    let (vpn_address, public_endpoint) = do_servidor_vpn(db).await?;

    let mut lista = vec![
        monta(AddressKind::Lan, da_rede_local(nat), &documento, nat),
        monta(AddressKind::Vpn, vpn_address, &documento, nat),
        monta(AddressKind::Public, public_endpoint, &documento, nat),
    ];
    lista.extend(documento.custom.iter().map(|entrada| ServerAddress {
        id: entrada.id.clone(),
        kind: AddressKind::Custom,
        label: entrada.label.clone(),
        description: AddressKind::Custom.description().to_owned(),
        value: Some(entrada.value.clone()),
        detected: None,
        overridden: true,
        source: "definido por você".to_owned(),
    }));
    Ok(lista)
}

fn monta(
    kind: AddressKind,
    detectado: Option<String>,
    documento: &StoredAddresses,
    nat: &NatDetector,
) -> ServerAddress {
    let correcao = documento
        .overrides
        .get(kind.id())
        .map(|valor| valor.trim().to_owned())
        .filter(|valor| !valor.is_empty());
    let overridden = correcao.is_some();
    let source = if overridden {
        "corrigido por você".to_owned()
    } else if detectado.is_some() {
        match kind {
            AddressKind::Lan => "detectado neste servidor".to_owned(),
            AddressKind::Vpn | AddressKind::Public => "do servidor WireGuard".to_owned(),
            AddressKind::Custom => "definido por você".to_owned(),
        }
    } else {
        motivo_de_nao_detectar(kind, nat)
    };

    ServerAddress {
        id: kind.id().to_owned(),
        kind,
        label: kind.label().to_owned(),
        description: kind.description().to_owned(),
        value: correcao.clone().or_else(|| detectado.clone()),
        detected: detectado,
        overridden,
        source,
    }
}

/// Por que um endereço não foi descoberto.
///
/// Dizer só "não detectado" deixaria o operador procurando defeito. Cada caso
/// tem uma causa concreta, e a do container é a mais comum e a menos óbvia.
fn motivo_de_nao_detectar(kind: AddressKind, nat: &NatDetector) -> String {
    match kind {
        AddressKind::Lan if nat.bridged_container() => {
            "não detectado — em container de rede bridge o servidor enxerga só a ponte, não o IP \
             da máquina"
                .to_owned()
        }
        AddressKind::Lan => "não detectado — informe o IP deste servidor na rede".to_owned(),
        AddressKind::Vpn => "não detectado — nenhum servidor WireGuard configurado".to_owned(),
        AddressKind::Public => {
            "não detectado — o servidor WireGuard está sem endereço público".to_owned()
        }
        AddressKind::Custom => "definido por você".to_owned(),
    }
}

/// O endereço de saída deste servidor.
///
/// Num container em rede bridge a rota devolve o IP da ponte, que é correto
/// para o kernel e inútil para um roteador. Devolver `None` ali é mais honesto
/// do que oferecer `172.17.0.2` como "o endereço da rede local".
fn da_rede_local(nat: &NatDetector) -> Option<String> {
    if nat.bridged_container() {
        return None;
    }
    DESTINO_DE_SONDAGEM
        .parse::<IpAddr>()
        .ok()
        .and_then(hints::local_address_toward)
        .map(|endereco| endereco.to_string())
}

/// O endereço do servidor dentro do túnel e o endpoint público.
///
/// O endereço do túnel **não é uma coluna**: é o primeiro endereço útil da
/// faixa da rede vinculada, o mesmo cálculo que o `server_service` usa para
/// montar a interface. Duplicar a conta aqui divergiria dela na primeira
/// mudança — daí a chamada ao mesmo helper.
async fn do_servidor_vpn(db: &DatabaseConnection) -> AppResult<(Option<String>, Option<String>)> {
    let Some(servidor) = vpn_servers::Entity::find().one(db).await? else {
        return Ok((None, None));
    };
    let publico = servidor
        .public_endpoint
        .as_deref()
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .map(str::to_owned);

    let endereco = match networks::Entity::find_by_id(servidor.network_id)
        .one(db)
        .await?
    {
        Some(rede) => first_usable_address(&rede.cidr)
            .ok()
            .map(|endereco| endereco.to_string()),
        None => None,
    };
    Ok((endereco, publico))
}

/// Qual endereço oferecer para um equipamento específico, e por quê.
///
/// A ordem não é arbitrária:
///
/// 1. **a forma de acesso declarada no cadastro**, porque declaração não é
///    palpite — quem declarou sabe de algo que este servidor não observa;
/// 2. **a rota de saída**: se o sistema operacional sai por `10.8.0.1` para
///    falar com o equipamento, é por `10.8.0.1` que ele volta. Resolve LAN e
///    túnel de uma vez, sem classificar nada;
/// 3. **a forma de acesso deduzida** — peer da VPN, faixa do túnel, endereço
///    privado ou público — para quando a rota não resolve;
/// 4. o marcado como padrão, e por fim o único que existir.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn suggest_for(
    db: &DatabaseConnection,
    dispositivo: &devices::Model,
    lista: &[ServerAddress],
) -> AppResult<Option<(String, String)>> {
    let contexto = access::AccessContext::load(db).await?;
    suggest_with(db, dispositivo, lista, &contexto.resolve(dispositivo)).await
}

/// Igual a [`suggest_for`], com a forma de acesso já resolvida.
///
/// Existe para quem já carregou o [`access::AccessContext`] — resolver de novo
/// custaria três consultas para chegar à mesma conclusão.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn suggest_with(
    db: &DatabaseConnection,
    dispositivo: &devices::Model,
    lista: &[ServerAddress],
    acesso: &access::ResolvedAccess,
) -> AppResult<Option<(String, String)>> {
    if acesso.declared {
        if let Some(entrada) = com_valor(lista, acesso.mode.address_kind()) {
            return Ok(Some((
                entrada,
                format!(
                    "o cadastro diz que este equipamento acessa por {}",
                    acesso.mode.label().to_lowercase()
                ),
            )));
        }
    }

    let host = dispositivo
        .ip_address
        .as_deref()
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .and_then(|texto| texto.parse::<IpAddr>().ok());

    if let Some(saida) = host.and_then(hints::local_address_toward) {
        let saida = saida.to_string();
        if let Some(entrada) = lista
            .iter()
            .find(|item| item.value.as_deref() == Some(&saida))
        {
            return Ok(Some((
                entrada.id.clone(),
                format!("este servidor alcança o equipamento por {saida}"),
            )));
        }
    }

    // A dedução entra quando a rota não resolveu: um peer de VPN que ainda não
    // fechou o túnel não tem rota, e o endereço do túnel continua sendo a
    // aposta certa — é por ele que o equipamento vai falar com este servidor.
    if let Some(entrada) = com_valor(lista, acesso.mode.address_kind()) {
        return Ok(Some((entrada, acesso.reason.clone())));
    }

    let documento = stored(db).await?;
    if let Some(preferido) = documento
        .preferred_id
        .as_deref()
        .and_then(|id| com_valor(lista, id))
    {
        return Ok(Some((preferido, "marcado como padrão".to_owned())));
    }

    Ok(lista
        .iter()
        .find(|item| item.value.is_some())
        .map(|item| (item.id.clone(), "único endereço disponível".to_owned())))
}

fn com_valor(lista: &[ServerAddress], id: &str) -> Option<String> {
    lista
        .iter()
        .find(|item| item.id == id && item.value.is_some())
        .map(|item| item.id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use migration::{Migrator, MigratorTrait};
    use sea_orm::{ActiveModelTrait, ActiveValue::Set};

    async fn banco() -> DatabaseConnection {
        let db = sea_orm::Database::connect(
            sea_orm::ConnectOptions::new("sqlite::memory:".to_owned())
                .max_connections(1)
                .min_connections(1)
                .to_owned(),
        )
        .await
        .expect("banco");
        Migrator::up(&db, None).await.expect("migrations");
        db
    }

    fn documento_vazio() -> StoredAddresses {
        StoredAddresses::default()
    }

    #[tokio::test]
    async fn a_lista_traz_os_tres_tipos_conhecidos_mesmo_sem_nada_configurado() {
        // Tipo não detectado continua na lista, com o motivo: escondê-lo faria
        // o operador não descobrir que aquela situação existe.
        let db = banco().await;
        let lista = list(&db, &NatDetector::none()).await.expect("lista");

        assert_eq!(lista.len(), 3);
        assert_eq!(lista[0].kind, AddressKind::Lan);
        assert_eq!(lista[1].kind, AddressKind::Vpn);
        assert_eq!(lista[2].kind, AddressKind::Public);
        assert!(lista[1].value.is_none(), "sem VPN configurada");
        assert!(
            lista[1].source.contains("nenhum servidor WireGuard"),
            "o motivo precisa ser concreto: {}",
            lista[1].source
        );
    }

    #[tokio::test]
    async fn o_endereco_do_tunel_sai_da_faixa_da_rede_vinculada() {
        // Não é coluna: é o primeiro endereço útil do CIDR, o mesmo cálculo que
        // o `server_service` usa para montar a interface.
        let db = banco().await;
        let rede = networks::ActiveModel {
            name: Set("VPN".to_owned()),
            cidr: Set("10.8.0.0/24".to_owned()),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("rede");
        vpn_servers::ActiveModel {
            network_id: Set(rede.id),
            interface_name: Set("wg0".to_owned()),
            listen_port: Set(51820),
            public_endpoint: Set(Some("casa.ddns.net".to_owned())),
            public_key: Set("chave".to_owned()),
            private_key_encrypted: Set("cifrada".to_owned()),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("servidor vpn");

        let lista = list(&db, &NatDetector::none()).await.expect("lista");
        assert_eq!(lista[1].value.as_deref(), Some("10.8.0.1"));
        assert_eq!(lista[1].source, "do servidor WireGuard");
        assert_eq!(lista[2].value.as_deref(), Some("casa.ddns.net"));
    }

    #[tokio::test]
    async fn a_correcao_do_operador_vence_o_detectado_e_pode_ser_desfeita() {
        let db = banco().await;
        let mut documento = documento_vazio();
        documento
            .overrides
            .insert("lan".to_owned(), "192.168.1.10".to_owned());
        save(&db, documento).await.expect("gravar");

        let lista = list(&db, &NatDetector::none()).await.expect("lista");
        assert_eq!(lista[0].value.as_deref(), Some("192.168.1.10"));
        assert!(lista[0].overridden);
        assert_eq!(lista[0].source, "corrigido por você");

        // Correção em branco significa "voltar ao detectado", não gravar vazio.
        let mut documento = stored(&db).await.expect("ler");
        documento
            .overrides
            .insert("lan".to_owned(), "  ".to_owned());
        save(&db, documento).await.expect("gravar");
        let lista = list(&db, &NatDetector::none()).await.expect("lista");
        assert!(!lista[0].overridden, "a correção precisava sumir");
    }

    #[tokio::test]
    async fn endereco_que_aponta_para_o_proprio_equipamento_e_recusado() {
        // O erro que motivou o módulo inteiro.
        let db = banco().await;
        for ruim in ["localhost", "127.0.0.1", "::1", "0.0.0.0"] {
            let mut documento = documento_vazio();
            documento
                .overrides
                .insert("lan".to_owned(), ruim.to_owned());
            let erro = save(&db, documento).await.expect_err("devia recusar");
            assert!(
                format!("{erro:?}").contains("aponta o equipamento para ele mesmo"),
                "aceitou {ruim}"
            );
        }
    }

    #[tokio::test]
    async fn o_personalizado_exige_nome_e_ganha_id_sozinho() {
        let db = banco().await;
        let mut documento = documento_vazio();
        documento.custom.push(CustomAddress {
            id: String::new(),
            label: "  ".to_owned(),
            value: "172.20.0.5".to_owned(),
        });
        // Sem nome, a lista viraria três IPs sem critério — que é justamente o
        // que este recurso existe para evitar.
        assert!(save(&db, documento.clone()).await.is_err());

        documento.custom[0].label = " Filial Norte ".to_owned();
        save(&db, documento).await.expect("gravar");

        let guardado = stored(&db).await.expect("ler");
        assert_eq!(guardado.custom[0].label, "Filial Norte");
        assert!(guardado.custom[0].id.starts_with("custom:"));

        let lista = list(&db, &NatDetector::none()).await.expect("lista");
        assert_eq!(lista.len(), 4);
        assert_eq!(lista[3].label, "Filial Norte");
        assert_eq!(lista[3].kind, AddressKind::Custom);
    }

    #[tokio::test]
    async fn json_corrompido_nao_apaga_a_lista() {
        let db = banco().await;
        system_settings::Model::set(&db, STORAGE_KEY, Some("{não é json".to_owned()))
            .await
            .expect("gravar lixo");
        let lista = list(&db, &NatDetector::none()).await.expect("lista");
        assert_eq!(lista.len(), 3, "os detectados continuam de pé");
    }

    #[tokio::test]
    async fn a_sugestao_segue_a_rota_de_saida_ate_o_equipamento() {
        // A rota resolve LAN e túnel de uma vez: se o sistema sai por X para
        // falar com o equipamento, é por X que ele volta.
        let db = banco().await;
        let dispositivo = devices::ActiveModel {
            name: Set("rt".to_owned()),
            r#type: Set("router".to_owned()),
            status: Set("online".to_owned()),
            ip_address: Set(Some("192.0.2.9".to_owned())),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("dispositivo");

        let Some(saida) = hints::local_address_toward("192.0.2.9".parse().unwrap()) else {
            // Máquina sem rota padrão: não há o que exercitar.
            return;
        };
        let saida = saida.to_string();

        // Um personalizado com exatamente o endereço de saída. O que se afirma
        // é o **valor** sugerido, não qual entrada o carrega: se a rede local
        // detectada tiver o mesmo endereço, escolher qualquer uma das duas é
        // igualmente certo — e é isso que precisa continuar valendo.
        let mut documento = documento_vazio();
        documento.custom.push(CustomAddress {
            id: "custom:teste".to_owned(),
            label: "Saída".to_owned(),
            value: saida.clone(),
        });
        save(&db, documento).await.expect("gravar");

        let lista = list(&db, &NatDetector::none()).await.expect("lista");
        let (id, motivo) = suggest_for(&db, &dispositivo, &lista)
            .await
            .expect("sugestão")
            .expect("alguma");

        let escolhida = lista.iter().find(|item| item.id == id).expect("na lista");
        assert_eq!(
            escolhida.value.as_deref(),
            Some(saida.as_str()),
            "sugeriu um endereço por onde o servidor não sai para este equipamento"
        );
        assert!(motivo.contains("alcança o equipamento por"), "{motivo}");
    }

    #[tokio::test]
    async fn a_forma_de_acesso_declarada_escolhe_o_endereco_sozinha() {
        // O ponto do recurso: sem declaração, um equipamento sem rota resolvida
        // cairia no primeiro endereço da lista. Com ela, o cadastro decide — e a
        // tela de ativação de log deixa de perguntar.
        let db = banco().await;
        let mut documento = documento_vazio();
        documento
            .overrides
            .insert("public".to_owned(), "casa.ddns.net".to_owned());
        documento
            .overrides
            .insert("lan".to_owned(), "192.168.1.10".to_owned());
        save(&db, documento).await.expect("gravar");

        let mut dispositivo = devices::ActiveModel {
            name: Set("filial".to_owned()),
            r#type: Set("router".to_owned()),
            status: Set("online".to_owned()),
            // IP privado: a dedução diria "rede local", e estaria errada.
            ip_address: Set(Some("192.168.90.1".to_owned())),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("dispositivo");
        dispositivo.access_mode = Some("remote".to_owned());

        let lista = list(&db, &NatDetector::none()).await.expect("lista");
        let (id, motivo) = suggest_for(&db, &dispositivo, &lista)
            .await
            .expect("sugestão")
            .expect("alguma");
        assert_eq!(id, "public");
        assert!(motivo.contains("cadastro"), "{motivo}");
    }

    #[tokio::test]
    async fn a_correcao_que_virou_copia_do_detectado_e_esquecida() {
        // Mantê-la seria guardar uma bomba-relógio: na próxima mudança do
        // endpoint da VPN ela venceria em silêncio, e o syslog passaria a
        // apontar para um endereço que os peers já não usam.
        let db = banco().await;
        let mut documento = documento_vazio();
        documento
            .overrides
            .insert("public".to_owned(), "casa.ddns.net".to_owned());
        save(&db, documento).await.expect("gravar");

        assert!(!forget_override_if(&db, "public", "outro.ddns.net")
            .await
            .expect("verificar"));
        assert!(stored(&db)
            .await
            .expect("ler")
            .overrides
            .contains_key("public"));

        assert!(forget_override_if(&db, "public", " casa.ddns.net ")
            .await
            .expect("verificar"));
        assert!(!stored(&db)
            .await
            .expect("ler")
            .overrides
            .contains_key("public"));
    }

    #[tokio::test]
    async fn sem_nenhum_endereco_a_sugestao_e_ausencia_e_nao_um_chute() {
        let db = banco().await;
        let dispositivo = devices::ActiveModel {
            name: Set("rt".to_owned()),
            r#type: Set("router".to_owned()),
            status: Set("online".to_owned()),
            ip_address: Set(None),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("dispositivo");

        let lista = vec![monta(
            AddressKind::Lan,
            None,
            &documento_vazio(),
            &NatDetector::none(),
        )];
        assert!(suggest_for(&db, &dispositivo, &lista)
            .await
            .expect("sugestão")
            .is_none());
    }
}
