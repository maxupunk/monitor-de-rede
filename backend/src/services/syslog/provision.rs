//! Ativação automática do envio de syslog, entrando no equipamento.
//!
//! O passo que mais falha na implantação nunca foi técnico: é o operador abrir
//! o terminal do roteador, colar o comando errado e não saber por que nada
//! aparece. O `snippets` resolveu metade — comando pronto, endereço já
//! preenchido. Este módulo fecha a outra metade: o servidor entra no
//! equipamento, roda o mesmo comando e devolve o que o aparelho respondeu.
//!
//! # A credencial não é guardada
//!
//! Ela chega no corpo da requisição, vive nesta função e morre com ela. Não há
//! coluna, não há `system_settings`, não há cache — nem cifrada. É uma decisão,
//! não uma pendência: guardar a senha de administrador de trinta roteadores
//! transformaria este sistema no alvo mais valioso da rede que ele monitora, e
//! o ganho seria reconfigurar o log, algo que se faz uma vez por aparelho.
//!
//! A consequência aceita é que reconfigurar exige digitar de novo. A tela diz
//! isso antes de pedir.
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

use tokio::time::timeout;

use super::{snippets, sources::SourceRegistry};
use crate::services::shared::errors::{AppError, AppResult};

/// Teto da sessão inteira. Um equipamento que não responde não pode segurar
/// um worker do Axum indefinidamente.
const TETO_DA_SESSAO: Duration = Duration::from_secs(45);

/// Teto para o TCP abrir. Roteador desligado falha rápido, e a tela diz isso em
/// vez de girar.
const TETO_DE_CONEXAO: Duration = Duration::from_secs(8);

/// Silêncio que declara um comando terminado. Ver a nota do módulo.
const SILENCIO: Duration = Duration::from_millis(700);

/// Teto por comando, quando o equipamento não para de falar.
const TETO_POR_COMANDO: Duration = Duration::from_secs(10);

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
    #[must_use]
    pub const fn default_port(self) -> u16 {
        match self {
            Self::Ssh => 22,
            Self::Telnet => 23,
            Self::MacTelnet => super::mactelnet::PORT,
        }
    }

    /// Se o acesso é endereçado por MAC em vez de por IP. Muda o que a tela
    /// precisa pedir e o que o controller precisa validar.
    #[must_use]
    pub const fn by_mac(self) -> bool {
        matches!(self, Self::MacTelnet)
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
    pub mac: Option<super::mactelnet::MacAddress>,
    pub port: u16,
    pub protocol: Protocol,
    pub username: String,
    pub password: String,
    /// Id do sistema — o mesmo do catálogo e do `snippets`.
    pub operating_system: String,
    /// Endereço deste servidor, como o roteador o enxerga.
    pub server_address: String,
    pub server_port: u16,
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
}

/// Entra no equipamento, roda os comandos e espera a primeira mensagem.
///
/// # Errors
///
/// Falha de conexão, de autenticação, ou sistema sem receita.
pub async fn run(
    pedido: &ProvisionRequest,
    sources: Option<&Arc<SourceRegistry>>,
    device_id: i64,
) -> AppResult<ProvisionOutcome> {
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

    let antes = sources.map(|registro| marcas(registro, device_id));

    let transcript = timeout(TETO_DA_SESSAO, executa(pedido, &comandos))
        .await
        .map_err(|_| {
            AppError::business_rule(
                "O equipamento não concluiu a configuração dentro do tempo limite. \
                 Verifique se o usuário informado tem permissão para alterar o log.",
            )
        })??;

    let confirmed = match (sources, antes) {
        (Some(registro), Some(antes)) => {
            Some(aguarda_primeira_mensagem(registro, device_id, antes).await)
        }
        _ => None,
    };

    Ok(ProvisionOutcome {
        commands: comandos,
        transcript: raspa(&transcript, &pedido.password),
        confirmed,
    })
}

/// Abre a sessão do protocolo escolhido e despeja os comandos nela.
async fn executa(pedido: &ProvisionRequest, comandos: &[String]) -> AppResult<String> {
    let mut sessao = match pedido.protocol {
        Protocol::Ssh => Sessao::Ssh(Box::new(ssh::abre(pedido).await?)),
        Protocol::Telnet => Sessao::Telnet(Box::new(telnet::abre(pedido).await?)),
        Protocol::MacTelnet => {
            let mac = pedido.mac.ok_or_else(|| {
                AppError::validation(
                    "O MAC-Telnet endereça o equipamento pelo MAC, e este dispositivo não tem um \
                     conhecido. Informe-o na tela.",
                )
            })?;
            Sessao::MacTelnet(Box::new(
                super::mactelnet::abre(mac, &pedido.username, &pedido.password, TETO_DE_CONEXAO)
                    .await?,
            ))
        }
    };

    // O banner de entrada é lido e descartado do fluxo de comandos, mas fica no
    // transcript: é lá que aparece "senha expirada" e outros avisos que
    // explicariam um comando recusado logo em seguida.
    let mut transcript = sessao.le_ate_silenciar().await?;

    for comando in comandos {
        sessao.envia_linha(comando).await?;
        let saida = sessao.le_ate_silenciar().await?;
        // O `sudo` do caminho Linux pergunta a senha do próprio usuário. Sem
        // esta resposta o comando morre no timeout sem dizer por quê.
        let saida = if pede_senha_de_sudo(&saida) {
            sessao.envia_linha(&pedido.password).await?;
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
    if let Some(teste) = snippets::test_command(&pedido.operating_system) {
        sessao.envia_linha(&teste).await?;
        let saida = sessao.le_ate_silenciar().await?;
        transcript.push_str(&format!("\n$ {teste}\n{saida}"));
    }

    sessao.encerra().await;
    Ok(transcript)
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

/// Quantas mensagens já haviam chegado deste dispositivo.
fn marcas(sources: &Arc<SourceRegistry>, device_id: i64) -> u64 {
    sources
        .list()
        .iter()
        .filter(|fonte| fonte.device_id == Some(device_id))
        .map(|fonte| fonte.message_count)
        .sum()
}

/// Espera a contagem subir. É o "está funcionando" da tela.
async fn aguarda_primeira_mensagem(
    sources: &Arc<SourceRegistry>,
    device_id: i64,
    antes: u64,
) -> bool {
    let ate = tokio::time::Instant::now() + TETO_DA_CONFIRMACAO;
    while tokio::time::Instant::now() < ate {
        if marcas(sources, device_id) > antes {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(400)).await;
    }
    false
}

/// Uma sessão aberta, de qualquer um dos dois protocolos.
///
/// Enum em vez de `trait` com `async fn`: são dois casos fechados, e o
/// despacho dinâmico exigiria `async_trait` só para isso.
enum Sessao {
    Ssh(Box<ssh::Shell>),
    Telnet(Box<telnet::Shell>),
    MacTelnet(Box<super::mactelnet::Shell>),
}

impl Sessao {
    async fn envia_linha(&mut self, linha: &str) -> AppResult<()> {
        match self {
            Self::Ssh(shell) => shell.envia_linha(linha).await,
            Self::Telnet(shell) => shell.envia_linha(linha).await,
            Self::MacTelnet(shell) => shell.envia_linha(linha).await,
        }
    }

    async fn le_ate_silenciar(&mut self) -> AppResult<String> {
        match self {
            Self::Ssh(shell) => shell.le_ate_silenciar().await,
            Self::Telnet(shell) => shell.le_ate_silenciar().await,
            Self::MacTelnet(shell) => shell.le_ate_silenciar(SILENCIO, TETO_POR_COMANDO).await,
        }
    }

    async fn encerra(self) {
        match self {
            Self::Ssh(shell) => shell.encerra().await,
            Self::Telnet(shell) => shell.encerra().await,
            Self::MacTelnet(shell) => shell.encerra().await,
        }
    }
}

/// Erro de rede vira mensagem de operador, não `Internal`.
///
/// O que dá errado aqui é quase sempre coisa que o usuário resolve — porta
/// fechada, senha errada, Telnet desabilitado. Um 500 com "erro interno"
/// mandaria procurar no lugar errado.
fn falha_de_acesso(detalhe: impl std::fmt::Display) -> AppError {
    AppError::business_rule(format!("Não foi possível acessar o equipamento: {detalhe}"))
}

mod ssh {
    //! Cliente SSH sobre `russh`.

    use super::{
        falha_de_acesso, AppError, AppResult, ProvisionRequest, SILENCIO, TETO_DE_CONEXAO,
        TETO_POR_COMANDO,
    };
    use russh::{
        client::{self, AuthResult, Handle},
        keys::ssh_key::PublicKey,
        ChannelMsg,
    };
    use std::sync::Arc;
    use tokio::time::{timeout, Instant};

    /// Aceita a chave de host apresentada, seja ela qual for.
    ///
    /// **É uma decisão consciente, e ela tem custo.** Não há `known_hosts` a
    /// consultar: é a primeira e única conexão a este equipamento, e não existe
    /// nada com que comparar. Recusar por não conhecer tornaria o recurso
    /// inútil; aceitar deixa a sessão exposta a um intermediário que já esteja
    /// dentro da rede administrada.
    ///
    /// O que torna o custo aceitável: a credencial é de uso único, o alvo é um
    /// IP da rede local que este mesmo sistema monitora, e quem já consegue se
    /// interpor no caminho até o roteador tem caminhos mais curtos. A tela diz
    /// que a conexão não valida a identidade do equipamento.
    struct AceitaQualquerChave;

    impl client::Handler for AceitaQualquerChave {
        type Error = russh::Error;

        async fn check_server_key(&mut self, _chave: &PublicKey) -> Result<bool, Self::Error> {
            Ok(true)
        }
    }

    pub struct Shell {
        canal: russh::Channel<client::Msg>,
        _sessao: Handle<AceitaQualquerChave>,
    }

    pub async fn abre(pedido: &ProvisionRequest) -> AppResult<Shell> {
        let config = Arc::new(client::Config {
            inactivity_timeout: Some(super::TETO_DA_SESSAO),
            ..client::Config::default()
        });

        let mut sessao = timeout(
            TETO_DE_CONEXAO,
            client::connect(config, (pedido.host, pedido.port), AceitaQualquerChave),
        )
        .await
        .map_err(|_| falha_de_acesso("tempo esgotado ao conectar na porta SSH"))?
        .map_err(falha_de_acesso)?;

        let resultado = sessao
            .authenticate_password(&pedido.username, &pedido.password)
            .await
            .map_err(falha_de_acesso)?;
        if !matches!(resultado, AuthResult::Success) {
            return Err(AppError::unauthorized(
                "Usuário ou senha recusados pelo equipamento.",
            ));
        }

        let canal = sessao
            .channel_open_session()
            .await
            .map_err(falha_de_acesso)?;
        // Largura generosa: o RouterOS quebra a linha na largura do terminal, e
        // uma coluna estreita picotaria a saída no meio das palavras.
        canal
            .request_pty(false, "vt100", 200, 50, 0, 0, &[])
            .await
            .map_err(falha_de_acesso)?;
        canal.request_shell(false).await.map_err(falha_de_acesso)?;

        Ok(Shell {
            canal,
            _sessao: sessao,
        })
    }

    impl Shell {
        pub async fn envia_linha(&mut self, linha: &str) -> AppResult<()> {
            // `\r` e não `\n`: é o que um terminal manda, e o RouterOS ignora a
            // linha que chega só com `\n`.
            self.canal
                .data(format!("{linha}\r").as_bytes())
                .await
                .map_err(falha_de_acesso)
        }

        pub async fn le_ate_silenciar(&mut self) -> AppResult<String> {
            let ate = Instant::now() + TETO_POR_COMANDO;
            let mut bruto = Vec::new();
            loop {
                let restante = ate.saturating_duration_since(Instant::now());
                if restante.is_zero() {
                    break;
                }
                match timeout(SILENCIO.min(restante), self.canal.wait()).await {
                    // Silêncio: o comando terminou.
                    Err(_) => break,
                    // Canal fechado pelo equipamento.
                    Ok(None) => break,
                    Ok(Some(ChannelMsg::Data { data })) => bruto.extend_from_slice(&data),
                    Ok(Some(ChannelMsg::ExtendedData { data, .. })) => {
                        bruto.extend_from_slice(&data);
                    }
                    Ok(Some(ChannelMsg::Eof | ChannelMsg::Close)) => break,
                    // Ajuste de janela, status de saída e afins não são saída.
                    Ok(Some(_)) => {}
                }
            }
            Ok(super::limpa(&bruto))
        }

        pub async fn encerra(self) {
            let _ = self.canal.data(&b"exit\r"[..]).await;
            let _ = self.canal.close().await;
        }
    }
}

mod telnet {
    //! Cliente Telnet mínimo — o bastante para logar e digitar.
    //!
    //! Não é uma implementação da RFC 854: é um cliente que **recusa todas as
    //! opções**. Responder `WONT` a todo `DO` e `DONT` a todo `WILL` é o
    //! comportamento previsto para um cliente que não negocia, e é o que
    //! equipamento de rede espera de um terminal simples. Negociar eco e
    //! `SGA` traria complexidade para um ganho que não existe aqui: ninguém
    //! está lendo esta sessão em tempo real.

    use super::{
        falha_de_acesso, AppError, AppResult, ProvisionRequest, SILENCIO, TETO_DE_CONEXAO,
        TETO_POR_COMANDO,
    };
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpStream,
        time::{timeout, Instant},
    };

    const IAC: u8 = 255;
    const DONT: u8 = 254;
    const DO: u8 = 253;
    const WONT: u8 = 252;
    const WILL: u8 = 251;
    /// Início de subnegociação; vai até `IAC SE`.
    const SB: u8 = 250;
    const SE: u8 = 240;

    pub struct Shell {
        fluxo: TcpStream,
    }

    pub async fn abre(pedido: &ProvisionRequest) -> AppResult<Shell> {
        let fluxo = timeout(
            TETO_DE_CONEXAO,
            TcpStream::connect((pedido.host, pedido.port)),
        )
        .await
        .map_err(|_| falha_de_acesso("tempo esgotado ao conectar na porta Telnet"))?
        .map_err(falha_de_acesso)?;

        let mut shell = Shell { fluxo };

        // O login do Telnet é conversa, não protocolo: o equipamento pede o
        // usuário, depois a senha, cada um com o seu texto. Procurar o rótulo é
        // o único caminho — e por isso as duas grafias mais comuns entram.
        let entrada = shell.le_ate_silenciar().await?;
        if contem_algum(&entrada, &["login", "username", "user name", "usuário"]) {
            shell.envia_linha(&pedido.username).await?;
            let pedido_de_senha = shell.le_ate_silenciar().await?;
            if contem_algum(&pedido_de_senha, &["password", "senha"]) {
                shell.envia_linha(&pedido.password).await?;
                let resposta = shell.le_ate_silenciar().await?;
                if contem_algum(
                    &resposta,
                    &["incorrect", "invalid", "failed", "inválid", "negad"],
                ) {
                    return Err(AppError::unauthorized(
                        "Usuário ou senha recusados pelo equipamento.",
                    ));
                }
            }
        }

        Ok(shell)
    }

    fn contem_algum(texto: &str, agulhas: &[&str]) -> bool {
        let minusculo = texto.to_ascii_lowercase();
        agulhas.iter().any(|agulha| minusculo.contains(agulha))
    }

    impl Shell {
        pub async fn envia_linha(&mut self, linha: &str) -> AppResult<()> {
            self.fluxo
                .write_all(format!("{linha}\r\n").as_bytes())
                .await
                .map_err(falha_de_acesso)
        }

        pub async fn le_ate_silenciar(&mut self) -> AppResult<String> {
            let ate = Instant::now() + TETO_POR_COMANDO;
            let mut util = Vec::new();
            let mut buffer = [0_u8; 4096];
            loop {
                let restante = ate.saturating_duration_since(Instant::now());
                if restante.is_zero() {
                    break;
                }
                match timeout(SILENCIO.min(restante), self.fluxo.read(&mut buffer)).await {
                    // Silêncio ou conexão fechada: acabou.
                    Err(_) | Ok(Ok(0)) => break,
                    Ok(Err(error)) => return Err(falha_de_acesso(error)),
                    Ok(Ok(lidos)) => {
                        let respostas = separa_comandos(&buffer[..lidos], &mut util);
                        if !respostas.is_empty() {
                            self.fluxo
                                .write_all(&respostas)
                                .await
                                .map_err(falha_de_acesso)?;
                        }
                    }
                }
            }
            Ok(super::limpa(&util))
        }

        pub async fn encerra(mut self) {
            let _ = self.fluxo.write_all(b"exit\r\n").await;
            let _ = self.fluxo.shutdown().await;
        }
    }

    /// Separa o texto dos comandos de protocolo e monta a recusa de cada opção.
    ///
    /// Devolve os bytes a mandar de volta; acumula o texto em `util`.
    fn separa_comandos(bruto: &[u8], util: &mut Vec<u8>) -> Vec<u8> {
        let mut resposta = Vec::new();
        let mut indice = 0;
        while indice < bruto.len() {
            if bruto[indice] != IAC {
                util.push(bruto[indice]);
                indice += 1;
                continue;
            }
            let Some(&verbo) = bruto.get(indice + 1) else {
                break;
            };
            match verbo {
                // `IAC IAC` é um 255 literal no texto.
                IAC => {
                    util.push(IAC);
                    indice += 2;
                }
                WILL | WONT => {
                    if let Some(&opcao) = bruto.get(indice + 2) {
                        resposta.extend_from_slice(&[IAC, DONT, opcao]);
                    }
                    indice += 3;
                }
                DO | DONT => {
                    if let Some(&opcao) = bruto.get(indice + 2) {
                        resposta.extend_from_slice(&[IAC, WONT, opcao]);
                    }
                    indice += 3;
                }
                // Subnegociação: pular até `IAC SE`, sem responder. Quem
                // recusou todas as opções não deveria recebê-la; se receber,
                // ignorar é mais seguro do que improvisar uma resposta.
                SB => {
                    indice += 2;
                    while indice < bruto.len() {
                        if bruto[indice] == IAC && bruto.get(indice + 1) == Some(&SE) {
                            indice += 2;
                            break;
                        }
                        indice += 1;
                    }
                }
                _ => indice += 2,
            }
        }
        resposta
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn as_opcoes_sao_todas_recusadas() {
            // `IAC DO ECHO` e `IAC WILL SGA`, com texto no meio.
            let bruto = [IAC, DO, 1, b'o', b'i', IAC, WILL, 3];
            let mut util = Vec::new();
            let resposta = separa_comandos(&bruto, &mut util);
            assert_eq!(util, b"oi", "o texto sai limpo dos comandos");
            assert_eq!(resposta, vec![IAC, WONT, 1, IAC, DONT, 3]);
        }

        #[test]
        fn o_255_literal_chega_ao_texto() {
            let mut util = Vec::new();
            separa_comandos(&[b'a', IAC, IAC, b'b'], &mut util);
            assert_eq!(util, vec![b'a', 255, b'b']);
        }

        #[test]
        fn a_subnegociacao_e_pulada_inteira() {
            // `IAC SB 24 ... IAC SE` seguido de texto.
            let bruto = [IAC, SB, 24, 0, b'x', b'y', IAC, SE, b'o', b'k'];
            let mut util = Vec::new();
            let resposta = separa_comandos(&bruto, &mut util);
            assert_eq!(util, b"ok", "nada da subnegociação vaza para o texto");
            assert!(resposta.is_empty(), "subnegociação não é respondida");
        }

        #[test]
        fn comando_truncado_no_fim_do_buffer_nao_estoura() {
            // O TCP corta onde quiser; um `IAC` sozinho no fim é normal.
            let mut util = Vec::new();
            separa_comandos(&[b'a', IAC], &mut util);
            separa_comandos(&[b'a', IAC, DO], &mut util);
            assert_eq!(util, b"aa");
        }
    }
}

/// Converte a saída bruta em texto legível.
///
/// `from_utf8_lossy` porque equipamento manda acentuação em latin-1 e byte de
/// controle de terminal; recusar a linha inteira por causa disso perderia o
/// diagnóstico. O `\r` sai porque a tela já quebra linha sozinha.
fn limpa(bruto: &[u8]) -> String {
    String::from_utf8_lossy(bruto).replace('\r', "")
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn o_pedido_de_senha_do_sudo_e_reconhecido_nas_duas_linguas() {
        assert!(pede_senha_de_sudo("[sudo] password for admin:"));
        assert!(pede_senha_de_sudo("[sudo] senha para admin:"));
        assert!(!pede_senha_de_sudo("Configuração aplicada."));
    }

    #[test]
    fn a_saida_bruta_perde_o_retorno_de_carro_e_aguenta_byte_invalido() {
        assert_eq!(limpa(b"linha 1\r\nlinha 2\r\n"), "linha 1\nlinha 2\n");
        // Latin-1 solto não pode derrubar o diagnóstico inteiro.
        assert!(limpa(&[b'a', 0xFF, b'b']).contains('a'));
    }
}
