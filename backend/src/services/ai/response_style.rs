//! Estilo de resposta do assistente: quanto texto a IA devolve por turno.
//!
//! O estilo atua em duas pontas: a diretiva no system prompt (o modelo sabe
//! que precisa ser curto) e o teto `max_tokens` enviado ao provedor (o modelo
//! não consegue passar do limite nem se ignorar a diretiva).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Teto de tokens de saída no modo direto. Folgado o bastante para uma
/// chamada de ferramenta com argumentos e um fechamento em poucas linhas —
/// e para modelos de raciocínio, que gastam parte do teto antes de responder.
/// Quem encurta a resposta é a diretiva; o teto só impede o excesso.
const CONCISE_MAX_OUTPUT_TOKENS: u32 = 1024;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum AiResponseStyle {
    /// Resposta mínima: conclusão e ação, sem introdução nem repetição de dados.
    #[default]
    Concise,
    /// Resposta explicativa, com contexto e raciocínio.
    Normal,
}

impl AiResponseStyle {
    /// Instrução de formato anexada ao system prompt.
    #[must_use]
    pub const fn directive(self) -> &'static str {
        match self {
            Self::Concise => {
                "FORMATO DA RESPOSTA (modo direto): responda com o mínimo de palavras. \
Sem saudação, sem introdução, sem repetir a pergunta e sem resumir o que as ferramentas retornaram. \
Entregue só o diagnóstico e a ação recomendada, em até 5 linhas ou tópicos curtos. \
Números com unidade (ms, %, bps). Quando um gráfico for exibido, não descreva os pontos: cite apenas o que é anormal."
            }
            Self::Normal => {
                "FORMATO DA RESPOSTA: seja claro e técnico. Explique a causa provável, as evidências \
que a sustentam e os passos de correção. Use tópicos e números com unidade (ms, %, bps). \
Quando um gráfico for exibido, comente as tendências relevantes sem listar ponto a ponto."
            }
        }
    }

    /// Teto de tokens de saída enviado ao provedor; `None` deixa o padrão dele.
    #[must_use]
    pub const fn max_output_tokens(self) -> Option<u32> {
        match self {
            Self::Concise => Some(CONCISE_MAX_OUTPUT_TOKENS),
            Self::Normal => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_padrao_e_o_modo_direto() {
        assert_eq!(AiResponseStyle::default(), AiResponseStyle::Concise);
    }

    #[test]
    fn so_o_modo_direto_limita_a_saida() {
        assert_eq!(
            AiResponseStyle::Concise.max_output_tokens(),
            Some(CONCISE_MAX_OUTPUT_TOKENS)
        );
        assert_eq!(AiResponseStyle::Normal.max_output_tokens(), None);
    }

    #[test]
    fn serializa_em_minusculas() {
        assert_eq!(
            serde_json::to_string(&AiResponseStyle::Concise).unwrap(),
            "\"concise\""
        );
        let normal: AiResponseStyle = serde_json::from_str("\"normal\"").unwrap();
        assert_eq!(normal, AiResponseStyle::Normal);
    }
}
