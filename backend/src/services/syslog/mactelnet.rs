//! MAC-Telnet — o acesso da MikroTik por endereço MAC, sem IP.
//!
//! É o que permite configurar um equipamento cujo IP está errado, ausente ou
//! numa faixa que este servidor não alcança: o RouterOS responde a um protocolo
//! próprio sobre UDP/20561, endereçado pelo **MAC**, e o transporte é difusão
//! na rede local. O OpenWRT atende o mesmo protocolo quando o pacote
//! `mactelnetd` está instalado.
//!
//! # Duas limitações que precisam ser ditas antes
//!
//! **Só funciona na mesma rede de camada 2.** Difusão não atravessa roteador —
//! e não atravessa a ponte do Docker. Num container em rede bridge (o arranjo
//! padrão do `docker-compose.yml`) o pacote não chega ao equipamento e nada
//! responde. `network_mode: host` resolve; ver [`super::nat`], que sabe dizer
//! em qual dos dois arranjos o processo está, e a tela avisa antes de deixar
//! escolher este meio.
//!
//! **O protocolo não é documentado pelo fabricante.** O que está aqui foi
//! escrito a partir da implementação de referência de código aberto
//! (`haakonnessjoen/MAC-Telnet`), que é ela própria fruto de engenharia
//! reversa. A montagem e a leitura dos pacotes têm teste unitário; o
//! *handshake* completo contra um RouterOS real **não foi verificado neste
//! repositório**, por falta de equipamento. Um erro aqui aparece como sessão
//! que não autentica, não como dado corrompido no equipamento: o pior caso é o
//! recurso não funcionar, e a tela cair de volta no SSH.
//!
//! # O formato
//!
//! ```text
//! cabeçalho (22 bytes)
//!   0      versão = 1
//!   1      tipo (sessão/dados/ack/fim)
//!   2..8   MAC de origem
//!   8..14  MAC de destino
//!   14..16 chave de sessão (u16 big-endian)
//!   16..18 tipo de cliente = 0x0015
//!   18..22 contador de bytes (u32 big-endian)
//!
//! pacote de controle, dentro da carga de um pacote de dados
//!   0..4   marca 56 34 12 ff
//!   4      tipo
//!   5..9   tamanho (u32 big-endian)
//!   9..    conteúdo
//! ```
//!
//! O contador é um número de sequência **sobre bytes de carga**, não sobre
//! pacotes: cada lado confirma dizendo quantos bytes já viu. É o que permite
//! retransmitir sem duplicar.

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};

use md5::{Digest, Md5};
use tokio::{net::UdpSocket, time::timeout};

use crate::services::shared::errors::{AppError, AppResult};

/// Porta do protocolo, nas duas pontas. O cliente precisa **originar** desta
/// porta: o RouterOS responde para a 20561, não para a porta efêmera de onde o
/// pacote saiu.
pub const PORT: u16 = 20561;

/// Marca que abre todo pacote de controle.
const CONTROL_MAGIC: [u8; 4] = [0x56, 0x34, 0x12, 0xff];

/// Tamanho do cabeçalho e do cabeçalho de controle.
const HEADER_LEN: usize = 22;
const CONTROL_HEADER_LEN: usize = 9;

/// Identifica o cliente como MAC-Telnet (e não MAC-Winbox, que usa a mesma
/// moldura com outro valor).
const CLIENT_TYPE: [u8; 2] = [0x00, 0x15];

/// Tipos de pacote.
mod ptype {
    pub const SESSION_START: u8 = 0;
    pub const DATA: u8 = 1;
    pub const ACK: u8 = 2;
    pub const END: u8 = 255;
}

/// Tipos de pacote de controle.
mod cptype {
    pub const BEGIN_AUTH: u8 = 0;
    pub const ENCRYPTION_KEY: u8 = 1;
    pub const PASSWORD: u8 = 2;
    pub const USERNAME: u8 = 3;
    pub const TERM_TYPE: u8 = 4;
    pub const TERM_WIDTH: u8 = 5;
    pub const TERM_HEIGHT: u8 = 6;
    pub const END_AUTH: u8 = 9;
    pub const PLAIN_DATA: u8 = 0xff;
}

/// Um endereço MAC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacAddress(pub [u8; 6]);

impl MacAddress {
    /// Aceita as três grafias usuais: `AA:BB:CC:DD:EE:FF`, `AA-BB-...` e
    /// `AABBCCDDEEFF`. O cadastro e o ARP não concordam sobre qual usar, e
    /// recusar por causa do separador seria recusar por nada.
    ///
    /// # Errors
    ///
    /// Texto que não tem 12 dígitos hexadecimais.
    pub fn parse(bruto: &str) -> AppResult<Self> {
        let limpo: String = bruto
            .chars()
            .filter(|caractere| caractere.is_ascii_hexdigit())
            .collect();
        if limpo.len() != 12 {
            return Err(AppError::validation(format!(
                "`{bruto}` não é um endereço MAC válido."
            )));
        }
        let mut octetos = [0_u8; 6];
        for (indice, octeto) in octetos.iter_mut().enumerate() {
            *octeto = u8::from_str_radix(&limpo[indice * 2..indice * 2 + 2], 16)
                .map_err(|_| AppError::validation("Endereço MAC inválido."))?;
        }
        Ok(Self(octetos))
    }
}

impl std::fmt::Display for MacAddress {
    fn fmt(&self, formatador: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let [a, b, c, d, e, f] = self.0;
        write!(
            formatador,
            "{a:02X}:{b:02X}:{c:02X}:{d:02X}:{e:02X}:{f:02X}"
        )
    }
}

/// Monta o cabeçalho de um pacote.
fn cabecalho(
    tipo: u8,
    origem: MacAddress,
    destino: MacAddress,
    sessao: u16,
    contador: u32,
) -> Vec<u8> {
    let mut pacote = Vec::with_capacity(HEADER_LEN);
    pacote.push(1); // versão
    pacote.push(tipo);
    pacote.extend_from_slice(&origem.0);
    pacote.extend_from_slice(&destino.0);
    pacote.extend_from_slice(&sessao.to_be_bytes());
    pacote.extend_from_slice(&CLIENT_TYPE);
    pacote.extend_from_slice(&contador.to_be_bytes());
    pacote
}

/// Acrescenta um pacote de controle à carga e devolve quantos bytes cresceu —
/// que é exatamente o quanto o contador de sequência deve avançar.
fn controle(pacote: &mut Vec<u8>, tipo: u8, conteudo: &[u8]) -> u32 {
    pacote.extend_from_slice(&CONTROL_MAGIC);
    pacote.push(tipo);
    pacote.extend_from_slice(
        &u32::try_from(conteudo.len())
            .unwrap_or(u32::MAX)
            .to_be_bytes(),
    );
    pacote.extend_from_slice(conteudo);
    u32::try_from(CONTROL_HEADER_LEN + conteudo.len()).unwrap_or(u32::MAX)
}

/// A resposta ao desafio de autenticação.
///
/// O RouterOS manda 16 bytes aleatórios; a prova é o MD5 de um byte zero,
/// seguido da senha, seguido do desafio. O byte zero também abre o campo
/// enviado — daí os 17 bytes de resposta.
///
/// MD5 aqui não é escolha de projeto: é o que o protocolo define, e trocá-lo
/// tornaria o cliente incompatível. Ele não protege a senha contra quem observa
/// a rede o suficiente para montar um dicionário, e é mais uma razão para o
/// SSH ser o caminho preferencial.
#[must_use]
pub fn resposta_de_autenticacao(senha: &str, desafio: &[u8]) -> [u8; 17] {
    let mut hash = Md5::new();
    hash.update([0_u8]);
    hash.update(senha.as_bytes());
    hash.update(desafio);
    let digest = hash.finalize();

    let mut resposta = [0_u8; 17];
    resposta[1..].copy_from_slice(&digest);
    resposta
}

/// Um pacote de controle já separado da moldura.
#[derive(Debug, PartialEq, Eq)]
pub struct ControlPacket {
    pub tipo: u8,
    pub conteudo: Vec<u8>,
}

/// Separa a carga de um pacote de dados em pacotes de controle.
///
/// Carga que não começa com a marca é texto do terminal solto — o RouterOS
/// manda assim em algumas versões, e descartá-la esconderia justamente a
/// resposta do comando.
#[must_use]
pub fn separa_controle(carga: &[u8]) -> Vec<ControlPacket> {
    let mut pacotes = Vec::new();
    let mut indice = 0;
    while indice + CONTROL_HEADER_LEN <= carga.len() {
        if carga[indice..indice + 4] != CONTROL_MAGIC {
            // O resto é texto solto; entrega inteiro e para.
            pacotes.push(ControlPacket {
                tipo: cptype::PLAIN_DATA,
                conteudo: carga[indice..].to_vec(),
            });
            return pacotes;
        }
        let tipo = carga[indice + 4];
        let tamanho = u32::from_be_bytes([
            carga[indice + 5],
            carga[indice + 6],
            carga[indice + 7],
            carga[indice + 8],
        ]) as usize;
        let inicio = indice + CONTROL_HEADER_LEN;
        // Tamanho maior que o pacote é lixo ou truncamento. Descartar o resto é
        // a única reação segura: seguir leria outro campo como conteúdo, e
        // entregar como texto despejaria binário no transcript.
        let Some(fim) = inicio
            .checked_add(tamanho)
            .filter(|fim| *fim <= carga.len())
        else {
            return pacotes;
        };
        pacotes.push(ControlPacket {
            tipo,
            conteudo: carga[inicio..fim].to_vec(),
        });
        indice = fim;
    }
    // Sobra curta demais para ser cabeçalho de controle: é texto do terminal.
    // Um prompt de dois caracteres cai exatamente aqui, e perdê-lo esconderia o
    // sinal de que a sessão abriu.
    if indice < carga.len() {
        pacotes.push(ControlPacket {
            tipo: cptype::PLAIN_DATA,
            conteudo: carga[indice..].to_vec(),
        });
    }
    pacotes
}

/// Lê o cabeçalho de um pacote recebido.
#[must_use]
pub fn le_cabecalho(pacote: &[u8]) -> Option<(u8, u16, u32, &[u8])> {
    if pacote.len() < HEADER_LEN || pacote[0] != 1 {
        return None;
    }
    let tipo = pacote[1];
    let sessao = u16::from_be_bytes([pacote[14], pacote[15]]);
    let contador = u32::from_be_bytes([pacote[18], pacote[19], pacote[20], pacote[21]]);
    Some((tipo, sessao, contador, &pacote[HEADER_LEN..]))
}

/// Uma sessão MAC-Telnet aberta.
pub struct Shell {
    socket: UdpSocket,
    destino: SocketAddr,
    origem: MacAddress,
    alvo: MacAddress,
    sessao: u16,
    /// Bytes de carga já enviados. É o número de sequência do protocolo.
    enviados: u32,
    /// Bytes de carga já recebidos, que é o que vai em cada confirmação.
    recebidos: u32,
    /// Sobras de saída lidas durante a autenticação, para não se perderem.
    pendente: String,
}

/// Abre a sessão e autentica.
///
/// # Errors
///
/// Falha de socket, equipamento que não responde, ou credencial recusada.
pub async fn abre(
    alvo: MacAddress,
    usuario: &str,
    senha: &str,
    teto: Duration,
) -> AppResult<Shell> {
    // Originar da 20561 não é detalhe: o equipamento responde para essa porta,
    // e um socket em porta efêmera nunca receberia a resposta.
    let socket = UdpSocket::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), PORT))
        .await
        .map_err(|error| {
            AppError::business_rule(format!(
                "Não foi possível abrir a porta {PORT} para MAC-Telnet ({error}). Ela precisa \
                 estar livre neste servidor, e o processo precisa enxergar a rede local — dentro \
                 de um container em rede bridge isso não acontece."
            ))
        })?;
    socket.set_broadcast(true).map_err(|error| {
        AppError::business_rule(format!("Não foi possível habilitar difusão: {error}"))
    })?;

    let destino = SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), PORT);
    // MAC de origem zerado: o equipamento responde ao MAC do quadro Ethernet,
    // e este campo serve só para o servidor casar a sessão.
    let origem = MacAddress([0; 6]);
    // A chave de sessão é sorteada; duas sessões simultâneas com o mesmo
    // equipamento se confundiriam sem ela.
    let sessao = rand::random::<u16>();

    let mut shell = Shell {
        socket,
        destino,
        origem,
        alvo,
        sessao,
        enviados: 0,
        recebidos: 0,
        pendente: String::new(),
    };

    shell
        .envia_cru(&cabecalho(ptype::SESSION_START, origem, alvo, sessao, 0))
        .await?;

    let mut inicio = cabecalho(ptype::DATA, origem, alvo, sessao, 0);
    shell.enviados += controle(&mut inicio, cptype::BEGIN_AUTH, &[]);
    shell.envia_cru(&inicio).await?;

    shell.autentica(usuario, senha, teto).await?;
    Ok(shell)
}

impl Shell {
    async fn envia_cru(&self, pacote: &[u8]) -> AppResult<()> {
        self.socket
            .send_to(pacote, self.destino)
            .await
            .map(|_| ())
            .map_err(|error| {
                AppError::business_rule(format!("Falha ao enviar pacote MAC-Telnet: {error}"))
            })
    }

    /// Confirma o que chegou. Sem isto o equipamento retransmite tudo e a
    /// sessão morre por excesso de repetição.
    async fn confirma(&self) -> AppResult<()> {
        let ack = cabecalho(
            ptype::ACK,
            self.origem,
            self.alvo,
            self.sessao,
            self.recebidos,
        );
        self.envia_cru(&ack).await
    }

    /// Espera o desafio, responde e aguarda o fim da autenticação.
    async fn autentica(&mut self, usuario: &str, senha: &str, teto: Duration) -> AppResult<()> {
        let limite = tokio::time::Instant::now() + teto;
        let mut desafiado = false;

        while tokio::time::Instant::now() < limite {
            let Some(carga) = self.recebe_ate(Duration::from_millis(1200)).await? else {
                if desafiado {
                    // Já respondemos e o equipamento se calou: em MAC-Telnet a
                    // recusa costuma ser silêncio, não mensagem.
                    return Err(AppError::unauthorized(
                        "O equipamento não concluiu a autenticação MAC-Telnet. Verifique o usuário \
                         e a senha.",
                    ));
                }
                continue;
            };

            for pacote in separa_controle(&carga) {
                match pacote.tipo {
                    cptype::ENCRYPTION_KEY => {
                        let resposta = resposta_de_autenticacao(senha, &pacote.conteudo);
                        let mut dados = cabecalho(
                            ptype::DATA,
                            self.origem,
                            self.alvo,
                            self.sessao,
                            self.enviados,
                        );
                        let mut crescimento = 0;
                        crescimento += controle(&mut dados, cptype::PASSWORD, &resposta);
                        crescimento += controle(&mut dados, cptype::USERNAME, usuario.as_bytes());
                        crescimento += controle(&mut dados, cptype::TERM_TYPE, b"vt100");
                        // Largura e altura vão em little-endian — é a única
                        // parte do protocolo que troca de ordem.
                        crescimento +=
                            controle(&mut dados, cptype::TERM_WIDTH, &200_u16.to_le_bytes());
                        crescimento +=
                            controle(&mut dados, cptype::TERM_HEIGHT, &50_u16.to_le_bytes());
                        self.enviados += crescimento;
                        self.envia_cru(&dados).await?;
                        desafiado = true;
                    }
                    cptype::END_AUTH => return Ok(()),
                    cptype::PLAIN_DATA => {
                        self.pendente
                            .push_str(&String::from_utf8_lossy(&pacote.conteudo));
                        // Alguns firmwares não mandam END_AUTH: o prompt
                        // chegando já é a prova de que a sessão abriu.
                        if desafiado {
                            return Ok(());
                        }
                    }
                    _ => {}
                }
            }
        }
        Err(AppError::business_rule(
            "O equipamento não respondeu ao MAC-Telnet. Ele precisa estar na mesma rede local \
             deste servidor — difusão não atravessa roteador nem a ponte do Docker.",
        ))
    }

    /// Espera um pacote de dados e devolve a carga, confirmando o recebimento.
    async fn recebe_ate(&mut self, espera: Duration) -> AppResult<Option<Vec<u8>>> {
        let mut buffer = [0_u8; 1500];
        let Ok(recebido) = timeout(espera, self.socket.recv_from(&mut buffer)).await else {
            return Ok(None);
        };
        let (tamanho, _) = recebido.map_err(|error| {
            AppError::business_rule(format!("Falha ao receber pacote MAC-Telnet: {error}"))
        })?;

        let Some((tipo, sessao, contador, carga)) = le_cabecalho(&buffer[..tamanho]) else {
            return Ok(None);
        };
        // Difusão traz o tráfego de outras sessões junto; a chave separa.
        if sessao != self.sessao {
            return Ok(None);
        }
        match tipo {
            ptype::DATA => {
                self.recebidos = contador + u32::try_from(carga.len()).unwrap_or(0);
                let carga = carga.to_vec();
                self.confirma().await?;
                Ok(Some(carga))
            }
            ptype::END => Err(AppError::business_rule(
                "O equipamento encerrou a sessão MAC-Telnet.",
            )),
            _ => Ok(None),
        }
    }

    /// Envia uma linha para o terminal do equipamento.
    ///
    /// # Errors
    ///
    /// Falha de socket.
    pub async fn envia_linha(&mut self, linha: &str) -> AppResult<()> {
        let texto = format!("{linha}\r");
        let mut pacote = cabecalho(
            ptype::DATA,
            self.origem,
            self.alvo,
            self.sessao,
            self.enviados,
        );
        self.enviados += controle(&mut pacote, cptype::PLAIN_DATA, texto.as_bytes());
        self.envia_cru(&pacote).await
    }

    /// Lê até o equipamento se calar — mesmo critério do SSH e do Telnet.
    ///
    /// # Errors
    ///
    /// Falha de socket, ou sessão encerrada pelo equipamento.
    pub async fn le_ate_silenciar(
        &mut self,
        silencio: Duration,
        teto: Duration,
    ) -> AppResult<String> {
        let limite = tokio::time::Instant::now() + teto;
        let mut saida = std::mem::take(&mut self.pendente);
        loop {
            let restante = limite.saturating_duration_since(tokio::time::Instant::now());
            if restante.is_zero() {
                break;
            }
            let Some(carga) = self.recebe_ate(silencio.min(restante)).await? else {
                break;
            };
            for pacote in separa_controle(&carga) {
                if pacote.tipo == cptype::PLAIN_DATA {
                    saida.push_str(&String::from_utf8_lossy(&pacote.conteudo));
                }
            }
        }
        Ok(saida.replace('\r', ""))
    }

    /// Encerra a sessão. Falha aqui é irrelevante: a sessão morre por inatividade.
    pub async fn encerra(self) {
        let fim = cabecalho(
            ptype::END,
            self.origem,
            self.alvo,
            self.sessao,
            self.enviados,
        );
        let _ = self.envia_cru(&fim).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_mac_e_lido_nas_tres_grafias_usuais() {
        // O cadastro, o ARP e o SNMP não concordam sobre o separador.
        let esperado = MacAddress([0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
        assert_eq!(MacAddress::parse("AA:BB:CC:DD:EE:FF").unwrap(), esperado);
        assert_eq!(MacAddress::parse("aa-bb-cc-dd-ee-ff").unwrap(), esperado);
        assert_eq!(MacAddress::parse("AABBCCDDEEFF").unwrap(), esperado);
        assert_eq!(MacAddress::parse(" aa:bb:cc:dd:ee:ff ").unwrap(), esperado);
    }

    #[test]
    fn mac_incompleto_ou_com_lixo_e_recusado() {
        // Doze dígitos é o critério: aceitar menos montaria um endereço com
        // octeto zerado e enviaria a difusão para o aparelho errado.
        assert!(MacAddress::parse("AA:BB:CC:DD:EE").is_err());
        assert!(MacAddress::parse("").is_err());
        assert!(MacAddress::parse("não é mac").is_err());
        assert!(MacAddress::parse("AA:BB:CC:DD:EE:FF:00").is_err());
    }

    #[test]
    fn o_mac_volta_no_formato_canonico() {
        let mac = MacAddress::parse("aabbccddeeff").unwrap();
        assert_eq!(mac.to_string(), "AA:BB:CC:DD:EE:FF");
    }

    #[test]
    fn o_cabecalho_tem_o_formato_do_protocolo() {
        let origem = MacAddress([1, 2, 3, 4, 5, 6]);
        let destino = MacAddress([0xAA; 6]);
        let pacote = cabecalho(ptype::DATA, origem, destino, 0x1234, 0x0000_0042);

        assert_eq!(pacote.len(), HEADER_LEN);
        assert_eq!(pacote[0], 1, "versão");
        assert_eq!(pacote[1], ptype::DATA);
        assert_eq!(&pacote[2..8], &origem.0);
        assert_eq!(&pacote[8..14], &destino.0);
        assert_eq!(&pacote[14..16], &[0x12, 0x34], "sessão em big-endian");
        assert_eq!(&pacote[16..18], &CLIENT_TYPE, "MAC-Telnet, não MAC-Winbox");
        assert_eq!(&pacote[18..22], &[0, 0, 0, 0x42], "contador em big-endian");
    }

    #[test]
    fn o_pacote_de_controle_declara_marca_tipo_e_tamanho() {
        let mut pacote = Vec::new();
        let crescimento = controle(&mut pacote, cptype::USERNAME, b"admin");

        assert_eq!(&pacote[0..4], &CONTROL_MAGIC);
        assert_eq!(pacote[4], cptype::USERNAME);
        assert_eq!(&pacote[5..9], &5_u32.to_be_bytes());
        assert_eq!(&pacote[9..], b"admin");
        // O contador de sequência avança pelo pacote inteiro, cabeçalho de
        // controle incluído — não só pelo conteúdo.
        assert_eq!(crescimento, 14);
    }

    #[test]
    fn a_resposta_de_autenticacao_segue_a_formula_do_protocolo() {
        // MD5 de: byte zero + senha + desafio; o byte zero também abre o campo.
        let desafio = [0x11_u8; 16];
        let resposta = resposta_de_autenticacao("senha", &desafio);

        assert_eq!(resposta.len(), 17);
        assert_eq!(resposta[0], 0, "o campo abre com zero");

        let mut esperado = Md5::new();
        esperado.update([0_u8]);
        esperado.update(b"senha");
        esperado.update(desafio);
        assert_eq!(&resposta[1..], esperado.finalize().as_slice());
    }

    #[test]
    fn senhas_diferentes_produzem_respostas_diferentes() {
        let desafio = [0x22_u8; 16];
        assert_ne!(
            resposta_de_autenticacao("a", &desafio),
            resposta_de_autenticacao("b", &desafio)
        );
        // E o mesmo par senha/desafio é determinístico — o protocolo depende
        // disso para o servidor conferir.
        assert_eq!(
            resposta_de_autenticacao("a", &desafio),
            resposta_de_autenticacao("a", &desafio)
        );
    }

    #[test]
    fn a_carga_e_separada_em_varios_pacotes_de_controle() {
        let mut carga = Vec::new();
        controle(&mut carga, cptype::PASSWORD, &[0xAB; 17]);
        controle(&mut carga, cptype::USERNAME, b"admin");

        let pacotes = separa_controle(&carga);
        assert_eq!(pacotes.len(), 2);
        assert_eq!(pacotes[0].tipo, cptype::PASSWORD);
        assert_eq!(pacotes[0].conteudo.len(), 17);
        assert_eq!(pacotes[1].tipo, cptype::USERNAME);
        assert_eq!(pacotes[1].conteudo, b"admin");
    }

    #[test]
    fn carga_sem_a_marca_vira_texto_do_terminal() {
        // Alguns firmwares mandam a saída solta; descartar esconderia
        // justamente a resposta do comando.
        let pacotes = separa_controle(b"[admin@MikroTik] > ");
        assert_eq!(pacotes.len(), 1);
        assert_eq!(pacotes[0].tipo, cptype::PLAIN_DATA);
        assert_eq!(pacotes[0].conteudo, b"[admin@MikroTik] > ");
    }

    #[test]
    fn tamanho_maior_que_o_pacote_descarta_o_resto_em_vez_de_estourar() {
        // Pacote truncado pela rede: seguir leria outro campo como conteúdo, e
        // entregar como texto despejaria binário no transcript.
        let mut carga = Vec::new();
        carga.extend_from_slice(&CONTROL_MAGIC);
        carga.push(cptype::PLAIN_DATA);
        carga.extend_from_slice(&9999_u32.to_be_bytes());
        carga.extend_from_slice(b"curto");

        assert!(
            separa_controle(&carga).is_empty(),
            "não pode entrar em pânico"
        );
    }

    #[test]
    fn o_que_ja_foi_lido_sobrevive_ao_truncamento_no_fim() {
        // Um pacote íntegro seguido de outro cortado pela rede: perder o
        // primeiro junto jogaria fora dado que chegou inteiro.
        let mut carga = Vec::new();
        controle(&mut carga, cptype::USERNAME, b"admin");
        carga.extend_from_slice(&CONTROL_MAGIC);
        carga.push(cptype::PLAIN_DATA);
        carga.extend_from_slice(&9999_u32.to_be_bytes());

        let pacotes = separa_controle(&carga);
        assert_eq!(pacotes.len(), 1);
        assert_eq!(pacotes[0].conteudo, b"admin");
    }

    #[test]
    fn sobra_curta_demais_para_cabecalho_vira_texto() {
        // O prompt do RouterOS chega assim depois de um pacote de controle;
        // descartá-lo esconderia o sinal de que a sessão abriu.
        let mut carga = Vec::new();
        controle(&mut carga, cptype::END_AUTH, &[]);
        carga.extend_from_slice(b"> ");

        let pacotes = separa_controle(&carga);
        assert_eq!(pacotes.len(), 2);
        assert_eq!(pacotes[1].tipo, cptype::PLAIN_DATA);
        assert_eq!(pacotes[1].conteudo, b"> ");
    }

    #[test]
    fn carga_vazia_nao_produz_pacote() {
        assert!(separa_controle(&[]).is_empty());
    }

    #[test]
    fn o_cabecalho_recebido_e_lido_de_volta() {
        let origem = MacAddress([1; 6]);
        let destino = MacAddress([2; 6]);
        let mut pacote = cabecalho(ptype::DATA, origem, destino, 0xBEEF, 100);
        let crescimento = controle(&mut pacote, cptype::PLAIN_DATA, b"oi");

        let (tipo, sessao, contador, carga) = le_cabecalho(&pacote).expect("cabeçalho");
        assert_eq!(tipo, ptype::DATA);
        assert_eq!(sessao, 0xBEEF);
        assert_eq!(contador, 100);
        assert_eq!(carga.len(), crescimento as usize);
        assert_eq!(separa_controle(carga)[0].conteudo, b"oi");
    }

    #[test]
    fn pacote_curto_ou_de_outra_versao_e_ignorado() {
        assert!(le_cabecalho(&[]).is_none());
        assert!(le_cabecalho(&[1, 2, 3]).is_none());
        // Versão diferente de 1 não é este protocolo.
        let mut alheio = cabecalho(ptype::DATA, MacAddress([0; 6]), MacAddress([0; 6]), 1, 0);
        alheio[0] = 2;
        assert!(le_cabecalho(&alheio).is_none());
    }
}
