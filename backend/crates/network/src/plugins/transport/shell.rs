//! As sessões interativas com um equipamento — SSH sobre `russh` e um Telnet
//! mínimo —, compartilhadas pelo transporte dos plugins (canal `exec`) e pela
//! ativação do log no roteador (`services::syslog::provision`, que digita os
//! comandos numa shell).

use std::time::Duration;

use netmonitor_core::shared::errors::{AppError, AppResult};

/// Teto da sessão inteira. Um equipamento que não responde não pode segurar
/// um worker do Axum indefinidamente.
pub const TETO_DA_SESSAO: Duration = Duration::from_secs(45);
/// Teto para o TCP abrir. Roteador desligado falha rápido, e a tela diz isso em
/// vez de girar.
pub const TETO_DE_CONEXAO: Duration = Duration::from_secs(8);
/// Silêncio que declara um comando terminado. Ver a nota do módulo.
pub const SILENCIO: Duration = Duration::from_millis(700);
/// Teto por comando, quando o equipamento não para de falar.
pub const TETO_POR_COMANDO: Duration = Duration::from_secs(10);

/// Erro de rede vira mensagem de operador, não `Internal`.
///
/// O que dá errado aqui é quase sempre coisa que o usuário resolve — porta
/// fechada, senha errada, Telnet desabilitado. Um 500 com "erro interno"
/// mandaria procurar no lugar errado.
pub fn falha_de_acesso(detalhe: impl std::fmt::Display) -> AppError {
    AppError::business_rule(format!("Não foi possível acessar o equipamento: {detalhe}"))
}

pub mod ssh {
    //! Cliente SSH sobre `russh`.
    //!
    //! [`conecta`] é compartilhado com o transporte dos plugins de dispositivo
    //! (`services::plugins::transport::local`), que usa o canal `exec`.

    use super::{
        falha_de_acesso, AppError, AppResult, SILENCIO, TETO_DE_CONEXAO, TETO_POR_COMANDO,
    };
    use crate::plugins::transport::Login;
    use netmonitor_core::shared::ssh::{self as ssh_comum, PinnedHostKey};
    use russh::{
        client::{self, AuthResult, Handle},
        ChannelMsg,
    };
    use std::{net::IpAddr, sync::Arc};
    use tokio::time::{timeout, Instant};

    /// A sessão SSH com um equipamento.
    ///
    /// A chave de host é conferida pelo [`PinnedHostKey`]: sem pin (login
    /// digitado, credencial por senha) ela é aceita e só anotada — não há
    /// `known_hosts` com que comparar, e recusar por não conhecer tornaria o
    /// recurso inútil. O acesso dedicado (ADR 016) fixa a chave vista no
    /// provisionamento, e dali em diante outra chave derruba a conexão antes de
    /// a chave do NetMonitor ser apresentada.
    pub type Sessao = Handle<PinnedHostKey>;

    pub struct Shell {
        canal: russh::Channel<client::Msg>,
        _sessao: Sessao,
    }

    /// Conecta e autentica — por chave quando o login traz uma, por senha
    /// senão. Devolve também a impressão digital da chave de host vista, que o
    /// provisionamento do acesso dedicado fixa.
    pub async fn conecta_vendo_host(
        host: IpAddr,
        port: u16,
        login: &Login,
    ) -> AppResult<(Sessao, String)> {
        let config = Arc::new(client::Config {
            inactivity_timeout: Some(super::TETO_DA_SESSAO),
            ..client::Config::default()
        });
        let handler = PinnedHostKey::new(login.host_key.clone());
        let vista = Arc::clone(&handler.observed);

        let conectado = timeout(
            TETO_DE_CONEXAO,
            client::connect(config, (host, port), handler),
        )
        .await
        .map_err(|_| falha_de_acesso("tempo esgotado ao conectar na porta SSH"))?;
        let mut sessao = match conectado {
            Ok(sessao) => sessao,
            Err(erro) => {
                if let (Some(esperada), Some(recebida)) =
                    (login.host_key.as_deref(), ssh_comum::observed(&vista))
                {
                    if esperada != recebida {
                        return Err(AppError::business_rule(format!(
                            "A identidade do equipamento mudou (esperada {esperada}, recebida                              {recebida}). Se ele foi trocado ou reinstalado, refaça o acesso                              dedicado; se não, alguém pode estar no caminho até ele."
                        )));
                    }
                }
                return Err(falha_de_acesso(erro));
            }
        };

        let aceito = if let Some(chave) = login.private_key.as_deref() {
            ssh_comum::authenticate_key(&mut sessao, &login.username, chave, None)
                .await
                .map_err(falha_de_acesso)?
        } else {
            let resultado = sessao
                .authenticate_password(
                    &login.username,
                    login.password.as_deref().unwrap_or_default(),
                )
                .await
                .map_err(falha_de_acesso)?;
            matches!(resultado, AuthResult::Success)
        };
        if !aceito {
            return Err(AppError::unauthorized(if login.private_key.is_some() {
                "O equipamento recusou a chave do acesso dedicado. Ela pode ter sido removida                  lá: refaça o acesso dedicado."
            } else {
                "Usuário ou senha recusados pelo equipamento."
            }));
        }
        Ok((sessao, ssh_comum::observed(&vista).unwrap_or_default()))
    }

    /// [`conecta_vendo_host`] para quem não precisa da chave de host. A sessão
    /// devolvida abre quantos canais o chamador precisar.
    pub async fn conecta(host: IpAddr, port: u16, login: &Login) -> AppResult<Sessao> {
        conecta_vendo_host(host, port, login)
            .await
            .map(|(sessao, _)| sessao)
    }

    pub async fn abre(host: IpAddr, port: u16, login: &Login) -> AppResult<Shell> {
        let sessao = conecta(host, port, login).await?;

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

pub mod telnet {
    //! Cliente Telnet mínimo — o bastante para logar e digitar.
    //!
    //! Não é uma implementação da RFC 854: é um cliente que **recusa todas as
    //! opções**. Responder `WONT` a todo `DO` e `DONT` a todo `WILL` é o
    //! comportamento previsto para um cliente que não negocia, e é o que
    //! equipamento de rede espera de um terminal simples. Negociar eco e
    //! `SGA` traria complexidade para um ganho que não existe aqui: ninguém
    //! está lendo esta sessão em tempo real.

    use super::{
        falha_de_acesso, AppError, AppResult, SILENCIO, TETO_DE_CONEXAO, TETO_POR_COMANDO,
    };
    use std::net::IpAddr;
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

    pub async fn abre(host: IpAddr, port: u16, username: &str, password: &str) -> AppResult<Shell> {
        let fluxo = timeout(TETO_DE_CONEXAO, TcpStream::connect((host, port)))
            .await
            .map_err(|_| falha_de_acesso("tempo esgotado ao conectar na porta Telnet"))?
            .map_err(falha_de_acesso)?;

        let mut shell = Shell { fluxo };

        // O login do Telnet é conversa, não protocolo: o equipamento pede o
        // usuário, depois a senha, cada um com o seu texto. Procurar o rótulo é
        // o único caminho — e por isso as duas grafias mais comuns entram.
        let entrada = shell.le_ate_silenciar().await?;
        if contem_algum(&entrada, &["login", "username", "user name", "usuário"]) {
            shell.envia_linha(username).await?;
            let pedido_de_senha = shell.le_ate_silenciar().await?;
            if contem_algum(&pedido_de_senha, &["password", "senha"]) {
                shell.envia_linha(password).await?;
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
pub fn limpa(bruto: &[u8]) -> String {
    String::from_utf8_lossy(bruto).replace('\r', "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_saida_bruta_perde_o_retorno_de_carro_e_aguenta_byte_invalido() {
        assert_eq!(limpa(b"linha 1\r\nlinha 2\r\n"), "linha 1\nlinha 2\n");
        // Latin-1 solto não pode derrubar o diagnóstico inteiro.
        assert!(limpa(&[b'a', 0xFF, b'b']).contains('a'));
    }
}
