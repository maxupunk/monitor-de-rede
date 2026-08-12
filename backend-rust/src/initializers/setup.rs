//! Anuncia, no boot, o token exigido no cadastro do primeiro usuário.
//!
//! Sem isto o token sorteado ficaria só no banco, e quem acabou de subir o
//! serviço não teria como descobri-lo — a tela de cadastro pediria um segredo
//! que ninguém viu. O log do container é o canal que quem instala sempre tem à
//! mão; a task `auth_setup_token` cobre quem chegou depois e perdeu a linha.
//!
//! **Por que um `println!` e não só um `tracing::warn!`.** Em produção o logger
//! sai em JSON (`config/production.yaml`), e ali uma linha a mais se perde entre
//! centenas de eventos de monitoramento — que é exatamente o que acontecia. O
//! quadro vai para o stdout, fora do formatador, cercado de linhas em branco: o
//! `docker compose logs` o mostra inteiro e ele não se confunde com o resto. O
//! `tracing::warn!` continua sendo emitido em seguida, para quem agrega log de
//! forma estruturada.
//!
//! Roda como `Initializer` (só no `start`, §`initializers::mod`): num `db
//! migrate` ou numa task o aviso seria ruído.

use async_trait::async_trait;
use loco_rs::{
    app::{AppContext, Initializer},
    Result,
};

use crate::services::auth::setup::{SetupService, SetupTokenOrigin, SETUP_TOKEN_ENV};

/// Largura interna do quadro. 68 colunas cabem num terminal de 80 sem quebrar.
const LARGURA: usize = 68;

pub struct SetupInitializer;

#[async_trait]
impl Initializer for SetupInitializer {
    fn name(&self) -> String {
        "setup".to_string()
    }

    async fn before_run(&self, ctx: &AppContext) -> Result<()> {
        let service = SetupService::new(&ctx.db);

        // Banco indisponível não pode derrubar o boot por causa de um aviso.
        match service.is_pending().await {
            Ok(false) => return Ok(()),
            Ok(true) => {}
            Err(error) => {
                tracing::warn!(%error, "não foi possível verificar o estado da instalação");
                return Ok(());
            }
        }

        match service.token_origin() {
            // O operador fixou o valor e já o conhece: repeti-lo no log só
            // colocaria um segredo de longa vida em disco, onde ele não estava.
            SetupTokenOrigin::Environment => {
                banner(&[
                    Linha::Titulo("INSTALAÇÃO PENDENTE"),
                    Linha::Vazia,
                    Linha::Texto("Nenhum usuário cadastrado. Abra o sistema no navegador"),
                    Linha::Texto("e crie o administrador na tela de primeiro acesso."),
                    Linha::Vazia,
                    Linha::Texto(&format!(
                        "O token de instalação é o valor de {SETUP_TOKEN_ENV},"
                    )),
                    Linha::Texto("definido no ambiente deste serviço."),
                ]);
                tracing::warn!(
                    variavel = SETUP_TOKEN_ENV,
                    "instalação pendente: cadastre o primeiro usuário"
                );
            }
            SetupTokenOrigin::Generated => match service.token().await {
                Ok(token) => {
                    banner_do_token(&token);
                    tracing::warn!(
                        setup_token = %token,
                        "instalação pendente: cadastre o primeiro usuário com este token"
                    );
                }
                Err(error) => {
                    tracing::warn!(%error, "não foi possível emitir o token de instalação");
                }
            },
        }

        Ok(())
    }
}

/// O quadro do token sorteado.
///
/// Público porque a task `auth_setup_token` imprime **o mesmo** quadro: quem viu
/// o do boot reconhece o da CLI, e o token aparece no mesmo lugar da moldura nos
/// dois. Duas formatações diferentes para o mesmo dado só fariam duvidar se é o
/// mesmo dado.
pub fn banner_do_token(token: &str) {
    banner(&[
        Linha::Titulo("INSTALAÇÃO PENDENTE"),
        Linha::Vazia,
        Linha::Texto("Nenhum usuário cadastrado. Abra o sistema no navegador"),
        Linha::Texto("e crie o administrador na tela de primeiro acesso."),
        Linha::Vazia,
        Linha::Texto("Token de instalação:"),
        Linha::Destaque(token),
        Linha::Vazia,
        Linha::Texto("Válido até o primeiro cadastro. Para vê-lo de novo:"),
        Linha::Texto("  backend_rust-cli task auth_setup_token"),
    ]);
}

/// Um tipo de linha dentro do quadro.
enum Linha<'a> {
    Titulo(&'a str),
    Texto(&'a str),
    /// Centralizada e entre espaços — para o token, que é o que se copia.
    Destaque(&'a str),
    Vazia,
}

/// Desenha o quadro no stdout, cercado de linhas em branco.
///
/// A largura é contada em **caracteres**, não em bytes: `LARGURA - texto.len()`
/// erraria o alinhamento em toda linha acentuada, que aqui são quase todas.
fn banner(linhas: &[Linha<'_>]) {
    let borda = "═".repeat(LARGURA);
    let mut saida = String::with_capacity(LARGURA * (linhas.len() + 4));

    saida.push_str(&format!("\n\n╔{borda}╗\n"));
    for linha in linhas {
        match linha {
            Linha::Titulo(texto) => saida.push_str(&centralizada(texto)),
            Linha::Destaque(texto) => {
                saida.push_str(&centralizada(""));
                saida.push_str(&centralizada(texto));
                saida.push_str(&centralizada(""));
            }
            Linha::Texto(texto) => saida.push_str(&alinhada(texto)),
            Linha::Vazia => saida.push_str(&alinhada("")),
        }
    }
    saida.push_str(&format!("╚{borda}╝\n\n"));

    println!("{saida}");
}

fn alinhada(texto: &str) -> String {
    let sobra = LARGURA.saturating_sub(texto.chars().count() + 2);
    format!("║ {texto}{} ║\n", " ".repeat(sobra))
}

fn centralizada(texto: &str) -> String {
    let largura = texto.chars().count();
    let esquerda = LARGURA.saturating_sub(largura) / 2;
    let direita = LARGURA.saturating_sub(largura + esquerda);
    format!("║{}{texto}{}║\n", " ".repeat(esquerda), " ".repeat(direita))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toda_linha_do_quadro_tem_a_mesma_largura_mesmo_com_acento() {
        let linhas = [
            alinhada("instalação pendente"),
            alinhada(""),
            centralizada("INSTALAÇÃO PENDENTE"),
            centralizada("aBc123"),
        ];
        for linha in linhas {
            // +2 pelas bordas `║`; -1 pelo `\n`.
            assert_eq!(
                linha.trim_end_matches('\n').chars().count(),
                LARGURA + 2,
                "linha desalinhada: {linha:?}"
            );
        }
    }

    #[test]
    fn texto_maior_que_o_quadro_nao_estoura_o_subtrair() {
        let longo = "x".repeat(LARGURA * 2);
        // O `saturating_sub` evita o pânico; a linha sai larga, e tudo bem.
        assert!(alinhada(&longo).contains(&longo));
        assert!(centralizada(&longo).contains(&longo));
    }
}
