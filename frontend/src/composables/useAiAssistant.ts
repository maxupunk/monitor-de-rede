import { computed, onMounted } from 'vue'
import { useAiStore, type AiChatContext } from '@/stores/ai'

/**
 * Porta de entrada das telas para o Assistente IA: diz se ele está
 * disponível e abre o chat já com a pergunta e o contexto da tela.
 */
export function useAiAssistant() {
  const aiStore = useAiStore()

  onMounted(() => {
    if (!aiStore.settings && !aiStore.loadingSettings) void aiStore.loadSettings()
  })

  const available = computed(() => aiStore.settings?.enabled === true)

  function ask(prompt: string, context: AiChatContext = {}): boolean {
    if (!available.value) return false
    return aiStore.askAbout(prompt, context)
  }

  return { available, busy: computed(() => aiStore.isStreaming), ask }
}
