//! Estimativa da janela pelo nome do modelo — a reserva para quando o
//! provedor não informa (sem rede, catálogo fora do ar, modelo desconhecido).
//! Errar para menos é preferível: avisa cedo que a conversa está ficando longa.

/// Janela quando a família não é reconhecida: o piso dos modelos atuais com
/// suporte a ferramentas.
pub const DEFAULT_CONTEXT_WINDOW: u64 = 32_768;

/// Famílias conhecidas, da mais específica para a mais genérica: a primeira
/// cujo trecho aparece no id vence.
const FAMILIES: &[(&str, u64)] = &[
    ("gemini", 1_048_576),
    ("gpt-4.1", 1_047_576),
    ("claude", 200_000),
    ("gpt-5", 400_000),
    ("o3", 200_000),
    ("o4", 200_000),
    ("gpt-4o", 128_000),
    ("gpt-oss", 131_072),
    ("kimi", 131_072),
    ("glm", 131_072),
    ("deepseek", 131_072),
    ("qwen3", 131_072),
    ("qwen2.5", 32_768),
    ("qwen", 32_768),
    ("llama-4", 1_048_576),
    ("llama4", 1_048_576),
    ("llama3", 131_072),
    ("llama-3", 131_072),
    ("mistral-small", 131_072),
    ("mistral-large", 131_072),
    ("mistral", 32_768),
    ("gemma3", 131_072),
    ("gemma-3", 131_072),
    ("gemma", 8_192),
    ("phi", 16_384),
    ("grok", 131_072),
    ("minimax", 1_000_000),
];

/// Janela estimada (em tokens) do modelo `model`.
#[must_use]
pub fn estimate(model: &str) -> u64 {
    let id = model.to_ascii_lowercase();
    FAMILIES
        .iter()
        .find(|(family, _)| id.contains(family))
        .map_or(DEFAULT_CONTEXT_WINDOW, |(_, window)| *window)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconhece_familias_pelo_id_do_provedor() {
        assert_eq!(estimate("anthropic/claude-sonnet-4"), 200_000);
        assert_eq!(estimate("google/gemini-2.5-flash"), 1_048_576);
        assert_eq!(estimate("openai/gpt-4o-mini"), 128_000);
        assert_eq!(estimate("meta-llama/llama-3.3-70b-instruct:free"), 131_072);
        assert_eq!(estimate("llama3.2"), 131_072);
    }

    #[test]
    fn familia_especifica_vence_a_generica() {
        assert_eq!(estimate("qwen3:8b"), 131_072);
        assert_eq!(estimate("qwen2.5-coder"), 32_768);
        assert_eq!(estimate("mistralai/mistral-small-3.2"), 131_072);
        assert_eq!(estimate("GPT-4.1-mini"), 1_047_576);
    }

    #[test]
    fn modelo_desconhecido_cai_no_piso() {
        assert_eq!(estimate("modelo-caseiro"), DEFAULT_CONTEXT_WINDOW);
        assert_eq!(estimate(""), DEFAULT_CONTEXT_WINDOW);
    }
}
