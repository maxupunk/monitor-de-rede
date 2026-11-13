/**
 * Avaliação de uma resposta da IA por quem conversou: 👍 é um acerto
 * confirmado; 👎, com o motivo, aponta o erro a corrigir. Fica guardada na
 * conversa e vai na exportação dos dados do Assistente (treino e revisão).
 */
export interface AiRating {
  value: 'up' | 'down'
  /** Por que a resposta foi ruim (só no 👎). */
  reason?: string | null
}

/** Motivos oferecidos no 👎 — os erros que mais importa separar na revisão. */
export const AI_RATING_REASONS = [
  'Resposta errada',
  'Não usou os dados da rede',
  'Entendeu errado o que eu pedi',
  'Ação ou ferramenta errada',
  'Incompleta',
  'Lenta demais',
] as const

/** Clicar no mesmo polegar desfaz; no outro, troca. */
export function toggleRating(
  current: AiRating | null | undefined,
  value: AiRating['value'],
  reason: string | null = null
): AiRating | null {
  if (current?.value === value && (value === 'up' || !reason || current.reason === reason)) {
    return null
  }
  return value === 'down' ? { value, reason: reason?.trim() || null } : { value }
}
