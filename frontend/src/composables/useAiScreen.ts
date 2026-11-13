import { computed } from 'vue'
import { useRoute } from 'vue-router'
import { useAiScreenStore } from '@/stores/aiScreen'
import {
  currentScreen,
  screenAttachment,
  type AiSuggestion,
  type ScreenAttachment,
} from '@/utils/aiScreen'

/**
 * A tela aberta, do ponto de vista do chat: qual é, se o usuário a dispensou,
 * as perguntas sugeridas para ela e o anexo que vai junto da pergunta. Um
 * lugar só para o campo do chat e para as sugestões da conversa vazia.
 */
export function useAiScreen() {
  const route = useRoute()
  const store = useAiScreenStore()

  const screen = computed(() => currentScreen(route))
  const active = computed(() => screen.value !== null && !store.isDismissed(screen.value.key))

  const hint = computed(() => {
    const label = screen.value?.meta.label ?? ''
    const detail = store.detail ? ` (${store.detail})` : ''
    return `A IA considera o que está na tela ${label}${detail}. Feche para perguntar sem ela.`
  })

  /** Sugestões da tela — só enquanto ela vale para a conversa. */
  const suggestions = computed<AiSuggestion[]>(() =>
    active.value ? (screen.value?.meta.suggestions ?? []) : []
  )

  /** O que vai junto da pergunta, ou `null` sem tela (ou dispensada). */
  function attachment(): ScreenAttachment | null {
    return screen.value && active.value ? screenAttachment(screen.value, store.detail) : null
  }

  function dismiss() {
    if (screen.value) store.dismiss(screen.value.key)
  }

  return { screen, active, hint, suggestions, attachment, dismiss, restore: store.restore }
}
