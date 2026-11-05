import type { Area } from '@/bindings/Area'
import type { AreaState } from '@/bindings/AreaState'

/** `show`: área desligada com sinal de uso; `hide`: área ligada sem nada em uso. */
export type SuggestionDirection = 'show' | 'hide'

export interface AreaSuggestion {
  area: Area
  title: string
  direction: SuggestionDirection
  message: string
}

/**
 * Sugestões por evidência (ADR 017). Nunca mudam a área sozinhas: só
 * perguntam. "Ocultar" só depois que a escolha do assistente foi feita — antes
 * disso, a instalação ainda está começando e "nada em uso" é o normal.
 */
export function areaSuggestions(
  states: readonly AreaState[],
  intentChosen: boolean
): AreaSuggestion[] {
  const list: AreaSuggestion[] = []
  for (const state of states) {
    if (!state.optional) continue
    if (!state.enabled && state.evidence?.present) {
      list.push({
        area: state.area,
        title: state.title,
        direction: 'show',
        message: `Encontramos ${state.evidence.reason ?? 'sinais de uso'}. Mostrar a área ${state.title}?`,
      })
    } else if (state.enabled && state.evidence?.present === false && intentChosen) {
      list.push({
        area: state.area,
        title: state.title,
        direction: 'hide',
        message: `Nada na área ${state.title} está em uso. Ocultar do menu?`,
      })
    }
  }
  return list
}

/** Chave do aviso dispensado: uma vez por usuário, área e direção. */
export function suggestionKey(userId: number, area: Area, direction: SuggestionDirection): string {
  return `netmonitor:area-suggestion:${userId}:${area}:${direction}`
}

export function isSuggestionDismissed(
  userId: number,
  area: Area,
  direction: SuggestionDirection
): boolean {
  try {
    return localStorage.getItem(suggestionKey(userId, area, direction)) !== null
  } catch {
    return false
  }
}

export function dismissSuggestion(userId: number, area: Area, direction: SuggestionDirection) {
  try {
    localStorage.setItem(suggestionKey(userId, area, direction), new Date().toISOString())
  } catch {
    // Sem armazenamento local (modo privado): o aviso volta na próxima sessão.
  }
}
