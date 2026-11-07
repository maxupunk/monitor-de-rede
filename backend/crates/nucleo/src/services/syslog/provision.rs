//! Ativação automática do envio de syslog, entrando no equipamento.
//!
//! O passo que mais falha na implantação nunca foi técnico: é o operador abrir
//! o terminal do roteador, colar o comando errado e não saber por que nada
//! aparece. O `snippets` resolveu metade — comando pronto, endereço já
//! preenchido. Este módulo fecha a outra metade: o servidor entra no
//! equipamento, roda o mesmo comando e devolve o que o aparelho respondeu.
//!
//! # A credencial é a do equipamento, e salvar é escolha do operador
//!
//! Este módulo recebe usuário e senha prontos e não guarda nada. De onde eles
//! vêm é de [`super::login`]: digitados na hora (usados uma vez) ou a
//! credencial salva do equipamento — a mesma da aba "Plugins › Credenciais".
//! Guardar a senha de administrador de trinta roteadores tornaria o sistema um
//! alvo valioso, então ela nunca vai ao banco: "salvar" lembra o usuário e
//! deixa a senha 30 minutos em memória; o que fica de fato é o acesso dedicado
//! por chave (ADR 016).
//!
//! # Por que um shell interativo, e não `exec`
//!
//! O canal `exec` do SSH roda **um** comando e fecha. RouterOS, EdgeOS e o modo
//! `configure` do Ubiquiti são sessões com estado: `configure` abre um contexto
//! que os `set` seguintes precisam encontrar aberto. Um shell com PTY é o único
//! caminho que serve aos quatro fabricantes e ao Telnet com o mesmo código.
//!
//! # Como se sabe que um comando terminou
//!
//! Não se espera prompt. Cada fabricante tem o seu, muda com o hostname, e
//! `>`/`#` aparecem dentro da saída de comando. O critério é **silêncio**: o
//! comando acabou quando o equipamento para de falar por um tempo. É frouxo por
//! escolha — um falso "acabou" só antecipa o próximo comando, enquanto um
//! prompt não reconhecido travaria a sessão até o teto.

use std::{net::IpAddr, sync::Arc, time::Duration};

use sea_orm::DatabaseConnection;
use tokio::time::timeout;
use uuid::Uuid;

use super::{
    resolver::{self, Resolution, Resolver},
    snippets,
    sources::{SeenSource, SourceRegistry},
};
/// As sessões SSH e Telnet com o equipamento são do transporte dos plugins
/// (área Rede) e chegam pelo acesso registrado ([`DeviceAccess`]); o
/// MAC-Telnet e os prazos de uma sessão são do núcleo.
use agent_protocol::device_io::{Login, TransportKind};
use netmonitor_core::devices::access::{
    DeviceAccess, DeviceShell, SILENCIO, TETO_DA_SESSAO, TETO_DE_CONEXAO, TETO_POR_COMANDO,
};

use crate::services::{
    devices::adapters::{registry as device_adapters, DeviceAccessMethod},
    network_tools::mactelnet,
    shared::errors::{AppError, AppResult},
};

/// Quanto se espera pela primeira mensagem depois de configurar.
///
/// É o que transforma "os comandos rodaram" em "está funcionando". Sem esta
/// confirmação a tela mentiria por omissão: comando aceito e log nenhum
/// chegando é exatamente o desfecho que este recurso existe para evitar.
const TETO_DA_CONFIRMACAO: Duration = Duration::from_secs(12);

/// Como entrar no equipamento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Ssh,
    Telnet,
    /// MikroTik (e OpenWRT com `mactelnetd`): acesso por MAC, sem IP. Só
    /// funciona na mesma rede de camada 2 — ver [`super::mactelnet`].
    MacTelnet,
}

impl Protocol {
    /// Porta padrão de cada um, para a tela não obrigar a digitar o óbvio.
    #[allow(dead_code)]
    #[must_use]
    pub const fn default_port(self) -> u16 {
        match self {
            Self::Ssh => 22,
            Self::Telnet => 23,
            Self::MacTelnet => mactelnet::PORT,
        }
    }

    /// Se o acesso é endereçado por MAC em vez de por IP. Muda o que a tela
    /// precisa pedir e o que o controller precisa validar.
    #[allow(dead_code)]
    #[must_use]
    pub const fn by_mac(self) -> bool {
        matches!(self, Self::MacTelnet)
    }

    /// Onde o login deste acesso fica salvo no equipamento. O MAC-Telnet entra
    /// com o mesmo usuário do Telnet — só muda como o equipamento é endereçado.
    #[must_use]
    pub const fn credential_kind(self) -> TransportKind {
        match self {
            Self::Ssh => TransportKind::Ssh,
            Self::Telnet | Self::MacTelnet => TransportKind::Telnet,
        }
    }

    /// # Errors
    ///
    /// Protocolo desconhecido.
    pub fn parse(valor: &str) -> AppResult<Self> {
        match valor.trim().to_ascii_lowercase().as_str() {
            "ssh" => Ok(Self::Ssh),
            "telnet" => Ok(Self::Telnet),
            "mactelnet" | "mac-telnet" => Ok(Self::MacTelnet),
            outro => Err(AppError::validation(format!(
                "Protocolo não suportado: {outro}. Use ssh, telnet ou mactelnet."
            ))),
        }
    }
}

/// Tudo que uma ativação precisa. A senha entra aqui e não sai.
pub struct ProvisionRequest {
    /// Alvo por IP. Irrelevante no MAC-Telnet, que endereça por MAC.
    pub host: IpAddr,
    /// Alvo por MAC — obrigatório no MAC-Telnet, ignorado nos outros.
    pub mac: Option<mactelnet::MacAddress>,
    pub port: u16,
    pub protocol: Protocol,
    /// Usuário e senha, ou a chave do acesso dedicado (só no SSH).
    pub login: Login,
    /// Id do sistema — o mesmo do catálogo e do `snippets`.
    pub operating_system: String,
    /// Endereço deste servidor, como o roteador o enxerga.
    pub server_address: String,
    pub server_port: u16,
}

impl ProvisionRequest {
    /// A senha do login — vazia no acesso por chave, que não tem.
    fn senha(&self) -> &str {
        self.login.password.as_deref().unwrap_or_default()
    }
}

/// O que a tela mostra depois.
#[derive(Debug, Clone)]
pub struct ProvisionOutcome {
    /// Os comandos que foram enviados — nunca a credencial.
    pub commands: Vec<String>,
    /// O que o equipamento respondeu, com a senha raspada.
    pub transcript: String,
    /// Se chegou log deste dispositivo antes do teto. `None` quando não havia
    /// como confirmar (ingestão desligada).
    pub confirmed: Option<bool>,
    /// Origem exata da linha de teste desta sessão. Uso interno para associar
    /// fontes mascaradas pelo Docker sem depender do nome cadastrado.
    pub observed_source: Option<SeenSource>,
    /// Hostname/identity lido diretamente na sessão autenticada. Ele é seguro
    /// para criar o vínculo mesmo quando a linha de teste demora a chegar.
    pub identified_hostname: Option<String>,
}

/// Entra no equipamento, roda os comandos e espera a primeira mensagem.
///
/// # Errors
///
/// Falha de conexão, de autenticação, ou sistema sem receita.
pub async fn run(
    pedido: &ProvisionRequest,
    access: Option<&dyn DeviceAccess>,
    sources: Option<&Arc<SourceRegistry>>,
    device_id: i64,
) -> AppResult<ProvisionOutcome> {
    let adapter = device_adapters::find(&pedido.operating_system).ok_or_else(|| {
        AppError::validation(format!(
            "Sistema desconhecido: {}.",
            pedido.operating_system
        ))
    })?;
    if pedido.protocol == Protocol::MacTelnet
        && !adapter.supports_access(DeviceAccessMethod::MacTelnet)
    {
        return Err(AppError::validation(format!(
            "{} não oferece acesso por MAC-Telnet.",
            adapter.platform().label
        )));
    }

    let comandos = snippets::commands_for(
        &pedido.operating_system,
        &pedido.server_address,
        pedido.server_port,
    )
    .ok_or_else(|| {
        AppError::validation(format!(
            "Não há receita de configuração automática para o sistema {}.",
            pedido.operating_system
        ))
    })?;

    let marker = format!("device-{device_id}-{}", Uuid::new_v4().simple());
    let test_command = snippets::test_command_with_marker(&pedido.operating_system, &marker);
    let pending =
        PendingConfirmation::new(sources.map(Arc::as_ref), test_command.is_some(), marker);

    let (transcript, identified_hostname) = timeout(
        TETO_DA_SESSAO,
        executa(pedido, access, &comandos, test_command.as_deref()),
    )
    .await
    .map_err(|_| {
        AppError::business_rule(
            "O equipamento não concluiu a configuração dentro do tempo limite. \
                 Verifique se o usuário informado tem permissão para alterar o log.",
        )
    })??;

    let observed_source = match pending.registry() {
        Some(registro) => aguarda_primeira_mensagem(registro, pending.marker()).await,
        None => None,
    };
    let confirmed = sources.map(|_| observed_source.is_some());

    Ok(ProvisionOutcome {
        commands: comandos,
        transcript: raspa(&transcript, pedido.senha()),
        confirmed,
        observed_source,
        identified_hostname,
    })
}

/// Abre a sessão do protocolo escolhido e despeja os comandos nela.
async fn executa(
    pedido: &ProvisionRequest,
    access: Option<&dyn DeviceAccess>,
    comandos: &[String],
    test_command: Option<&str>,
) -> AppResult<(String, Option<String>)> {
    let shell = |kind| async move {
        let access = access.ok_or_else(|| {
            AppError::ServiceUnavailable(
                "O acesso por SSH e Telnet é da área Rede, que não está montada neste processo"
                    .into(),
            )
        })?;
        access
            .open_shell(kind, pedido.host, pedido.port, &pedido.login)
            .await
    };
    let mut sessao = match pedido.protocol {
        Protocol::Ssh => Sessao::Shell(shell(TransportKind::Ssh).await?),
        Protocol::Telnet => Sessao::Shell(shell(TransportKind::Telnet).await?),
        Protocol::MacTelnet => {
            let mac = pedido.mac.ok_or_else(|| {
                AppError::validation(
                    "O MAC-Telnet endereça o equipamento pelo MAC, e este dispositivo não tem um \
                     conhecido. Informe-o na tela.",
                )
            })?;
            Sessao::MacTelnet(Box::new(
                mactelnet::abre(mac, &pedido.login.username, pedido.senha(), TETO_DE_CONEXAO)
                    .await?,
            ))
        }
    };

    // O banner de entrada é lido e descartado do fluxo de comandos, mas fica no
    // transcript: é lá que aparece "senha expirada" e outros avisos que
    // explicariam um comando recusado logo em seguida.
    let mut transcript = sessao.le_ate_silenciar().await?;

    // A identidade é lida dentro da mesma sessão já autenticada. Fazer uma
    // segunda conexão duplicaria autenticação, timeout e tratamento de erro.
    // Falhar em identificar não impede a configuração: a confirmação marcada
    // ainda pode descobrir o hostname pelo pacote recebido.
    let identified_hostname =
        if let Some(command) = snippets::identity_command(&pedido.operating_system) {
            sessao.envia_linha(&command).await?;
            let output = sessao.le_ate_silenciar().await?;
            transcript.push_str(&format!("\n$ {command}\n{output}"));
            snippets::parse_identity(&output)
        } else {
            None
        };

    for comando in comandos {
        sessao.envia_linha(comando).await?;
        let saida = sessao.le_ate_silenciar().await?;
        // O `sudo` do caminho Linux pergunta a senha do próprio usuário. Sem
        // esta resposta o comando morre no timeout sem dizer por quê.
        let saida = if pede_senha_de_sudo(&saida) {
            sessao.envia_linha(pedido.senha()).await?;
            format!("{saida}{}", sessao.le_ate_silenciar().await?)
        } else {
            saida
        };
        transcript.push_str(&format!("\n$ {comando}\n{saida}"));
    }

    // A linha de teste é o que separa "configurei e o aparelho está quieto" de
    // "configurei e o caminho está bloqueado". Sem ela, o silêncio depois da
    // ativação é ambíguo: as regras enviadas cobrem tópicos que só falam quando
    // algo acontece, e um roteador saudável pode passar horas sem dizer nada.
    if let Some(teste) = test_command {
        sessao.envia_linha(teste).await?;
        let saida = sessao.le_ate_silenciar().await?;
        transcript.push_str(&format!("\n$ {teste}\n{saida}"));
    }

    sessao.encerra().await;
    Ok((transcript, identified_hostname))
}

/// Persiste o vínculo entre a identidade anunciada e o dispositivo.
///
/// O vínculo é independente do endereço de origem e, portanto, continua
/// correto tanto em rede bridge/NAT quanto em `network_mode: host`.
///
/// # Errors
///
/// Propaga falhas de persistência.
pub async fn associate_identified_hostname(
    db: &DatabaseConnection,
    runtime: Option<(&Resolver, &SourceRegistry)>,
    device_id: i64,
    hostname: &str,
) -> AppResult<()> {
    let bind_key = resolver::hostname_bind_key(hostname);
    if let Some(existing_device_id) = resolver::bindings(db).await?.get(&bind_key) {
        if *existing_device_id != device_id {
            return Err(AppError::business_rule(format!(
                "A identidade `{hostname}` já está associada a outro dispositivo. Renomeie um dos equipamentos para manter os logs separados."
            )));
        }
    }
    resolver::bind(db, &bind_key, Some(device_id)).await?;
    if let Some((resolver, sources)) = runtime {
        resolver.invalidate().await;
        sources.reclassify(&bind_key, &Resolution::Device(device_id));
    }
    Ok(())
}

/// O `sudo` pedindo senha, nas formas em que ele aparece.
fn pede_senha_de_sudo(saida: &str) -> bool {
    let minusculo = saida.to_ascii_lowercase();
    minusculo.contains("[sudo] password")
        || minusculo.contains("senha para")
        || minusculo.contains("password for")
}

/// Remove a senha do transcript.
///
/// O Telnet ecoa o que é digitado antes de o servidor desligar o eco, e um
/// equipamento mal-comportado pode devolver a linha inteira. O transcript vai
/// para a tela e pode ir para um print de suporte — a senha não vai junto.
fn raspa(transcript: &str, senha: &str) -> String {
    if senha.is_empty() {
        return transcript.to_owned();
    }
    transcript.replace(senha, "********")
}

/// Mantém a observação registrada somente durante esta ativação, inclusive se
/// a sessão falhar ou for cancelada pelo cliente.
struct PendingConfirmation<'a> {
    registry: Option<&'a SourceRegistry>,
    marker: String,
}

impl<'a> PendingConfirmation<'a> {
    fn new(registry: Option<&'a SourceRegistry>, has_test: bool, marker: String) -> Self {
        let registry = registry.filter(|_| has_test);
        if let Some(registry) = registry {
            registry.expect_confirmation(&marker);
        }
        Self { registry, marker }
    }

    const fn registry(&self) -> Option<&SourceRegistry> {
        self.registry
    }

    fn marker(&self) -> &str {
        &self.marker
    }
}

impl Drop for PendingConfirmation<'_> {
    fn drop(&mut self) {
        if let Some(registry) = self.registry {
            registry.forget_confirmation(&self.marker);
        }
    }
}

/// Espera a linha marcada desta sessão. É o "está funcionando" da tela e não
/// pode ser confundida com mensagens antigas ou com outro dispositivo.
async fn aguarda_primeira_mensagem(sources: &SourceRegistry, marker: &str) -> Option<SeenSource> {
    let ate = tokio::time::Instant::now() + TETO_DA_CONFIRMACAO;
    while tokio::time::Instant::now() < ate {
        if let Some(source) = sources.confirmation(marker) {
            return Some(source);
        }
        tokio::time::sleep(Duration::from_millis(400)).await;
    }
    None
}

/// Associa ao dispositivo a origem comprovada pela linha marcada emitida
/// dentro da sessão autenticada. Assim um hostname diferente do nome de
/// cadastro funciona mesmo quando o Docker mascara o IP real do roteador.
///
/// # Errors
///
/// Propaga falhas de persistência. Uma origem mascarada sem hostname não pode
/// ser vinculada, pois a chave seria o gateway compartilhado por todo o parque.
pub async fn associate_confirmed_source(
    db: &DatabaseConnection,
    resolver: &Resolver,
    sources: &SourceRegistry,
    device_id: i64,
    source: &SeenSource,
) -> AppResult<()> {
    if source.device_id == Some(device_id) {
        return Ok(());
    }
    if source.masked && source.hostname.is_none() {
        return Err(AppError::business_rule(
            "A linha de teste chegou por um gateway compartilhado, mas sem hostname; não é seguro vincular esse endereço a um único dispositivo.",
        ));
    }

    if source.masked {
        return associate_identified_hostname(
            db,
            Some((resolver, sources)),
            device_id,
            source.hostname.as_deref().expect("validado acima"),
        )
        .await;
    }

    resolver::bind(db, &source.bind_key, Some(device_id)).await?;
    resolver.invalidate().await;
    sources.reclassify(&source.bind_key, &Resolution::Device(device_id));
    Ok(())
}

/// Uma sessão aberta: SSH ou Telnet pelo acesso da Rede, ou o MAC-Telnet do
/// núcleo.
enum Sessao {
    Shell(Box<dyn DeviceShell>),
    MacTelnet(Box<mactelnet::Shell>),
}

impl Sessao {
    async fn envia_linha(&mut self, linha: &str) -> AppResult<()> {
        match self {
            Self::Shell(shell) => shell.send_line(linha).await,
            Self::MacTelnet(shell) => shell.envia_linha(linha).await,
        }
    }

    async fn le_ate_silenciar(&mut self) -> AppResult<String> {
        match self {
            Self::Shell(shell) => shell.read_until_silent().await,
            Self::MacTelnet(shell) => shell.le_ate_silenciar(SILENCIO, TETO_POR_COMANDO).await,
        }
    }

    async fn encerra(self) {
        match self {
            Self::Shell(shell) => shell.close().await,
            Self::MacTelnet(shell) => shell.encerra().await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use netmonitor_core::migrations::{CoreMigrator as Migrator, MigratorTrait};
    use sea_orm::Database;

    #[test]
    fn a_senha_e_raspada_do_transcript() {
        // O Telnet ecoa o que é digitado antes de o servidor desligar o eco.
        let bruto = "login: admin\r\npassword: s3nh4-secreta\r\nbem-vindo";
        let limpo = raspa(bruto, "s3nh4-secreta");
        assert!(!limpo.contains("s3nh4-secreta"));
        assert!(limpo.contains("********"));
        assert!(limpo.contains("bem-vindo"), "o resto continua legível");
    }

    #[test]
    fn senha_vazia_nao_rasga_o_transcript() {
        // Substituir string vazia inseriria a máscara entre cada caractere.
        let transcript = "tudo certo";
        assert_eq!(raspa(transcript, ""), "tudo certo");
    }

    #[test]
    fn a_porta_padrao_segue_o_protocolo() {
        assert_eq!(Protocol::Ssh.default_port(), 22);
        assert_eq!(Protocol::Telnet.default_port(), 23);
    }

    #[test]
    fn o_protocolo_e_lido_sem_diferenciar_caixa() {
        assert_eq!(Protocol::parse("SSH").unwrap(), Protocol::Ssh);
        assert_eq!(Protocol::parse(" telnet ").unwrap(), Protocol::Telnet);
        assert!(Protocol::parse("rlogin").is_err());
    }

    #[tokio::test]
    async fn o_adapter_recusa_mactelnet_em_plataforma_incompativel() {
        let request = ProvisionRequest {
            host: "127.0.0.1".parse().unwrap(),
            mac: Some(mactelnet::MacAddress::parse("02:00:00:00:00:01").unwrap()),
            port: mactelnet::PORT,
            protocol: Protocol::MacTelnet,
            login: Login::with_password("admin", Some("segredo".to_owned())),
            operating_system: "windows".to_owned(),
            server_address: "192.0.2.10".to_owned(),
            server_port: 514,
        };

        let error = run(&request, None, None, 1)
            .await
            .expect_err("deveria recusar");
        assert!(error
            .to_string()
            .contains("não oferece acesso por MAC-Telnet"));
    }

    #[test]
    fn o_pedido_de_senha_do_sudo_e_reconhecido_nas_duas_linguas() {
        assert!(pede_senha_de_sudo("[sudo] password for admin:"));
        assert!(pede_senha_de_sudo("[sudo] senha para admin:"));
        assert!(!pede_senha_de_sudo("Configuração aplicada."));
    }

    #[tokio::test]
    async fn a_identidade_autenticada_vira_vinculo_persistente() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("banco em memória");
        Migrator::up(&db, None).await.expect("migrations");
        let device = crate::models::devices::ActiveModel {
            name: sea_orm::ActiveValue::Set("Nome amigável".to_owned()),
            r#type: sea_orm::ActiveValue::Set("router".to_owned()),
            status: sea_orm::ActiveValue::Set("online".to_owned()),
            ..Default::default()
        };
        let device = sea_orm::ActiveModelTrait::insert(device, &db)
            .await
            .expect("dispositivo");

        associate_identified_hostname(&db, None, device.id, "Roteador-Borda.local.")
            .await
            .expect("associação");

        let bindings = resolver::bindings(&db).await.expect("vínculos");
        assert_eq!(bindings.get("host:roteador-borda.local"), Some(&device.id));
    }

    #[tokio::test]
    async fn a_identidade_repetida_nao_e_transferida_para_outro_dispositivo() {
        let mut options = sea_orm::ConnectOptions::new("sqlite::memory:".to_owned());
        options.max_connections(1).min_connections(1);
        let db = Database::connect(options).await.expect("banco em memória");
        Migrator::up(&db, None).await.expect("migrations");
        let first = crate::models::devices::ActiveModel {
            name: sea_orm::ActiveValue::Set("Primeiro".to_owned()),
            r#type: sea_orm::ActiveValue::Set("router".to_owned()),
            status: sea_orm::ActiveValue::Set("online".to_owned()),
            ..Default::default()
        };
        let first = sea_orm::ActiveModelTrait::insert(first, &db)
            .await
            .expect("primeiro");
        let second = crate::models::devices::ActiveModel {
            name: sea_orm::ActiveValue::Set("Segundo".to_owned()),
            r#type: sea_orm::ActiveValue::Set("router".to_owned()),
            status: sea_orm::ActiveValue::Set("online".to_owned()),
            ..Default::default()
        };
        let second = sea_orm::ActiveModelTrait::insert(second, &db)
            .await
            .expect("segundo");

        associate_identified_hostname(&db, None, first.id, "openwrt")
            .await
            .expect("primeiro vínculo");
        assert!(
            associate_identified_hostname(&db, None, second.id, "openwrt")
                .await
                .is_err(),
            "uma identidade compartilhada não pode trocar de dono silenciosamente"
        );
        assert_eq!(
            resolver::bindings(&db)
                .await
                .expect("vínculos")
                .get("host:openwrt"),
            Some(&first.id)
        );
    }
}
