import { computed } from 'vue'
import { useAiStore } from '@/stores/ai'

/** Modelo usado quando o provedor está configurado sem modelo escolhido. */
const DEFAULT_MODELS: Record<string, string> = {
  opencode: 'muse-spark-1.3-contributor-free',
  openrouter: 'openrouter/free',
  ollama: 'llama3.2',
}

const DRIVER_LABELS: Record<string, string> = {
  opencode: 'OpenCode Go / Zen',
  openrouter: 'OpenRouter Gateway',
  ollama: 'Ollama Local',
}

/** Provedor e modelo configurados do Assistente IA, para cabeçalhos e cartões. */
export function useAiModelInfo() {
  const aiStore = useAiStore()

  const driverLabel = computed(
    () => DRIVER_LABELS[aiStore.settings?.activeDriver ?? ''] ?? 'Provedor desconhecido'
  )

  const modelLabel = computed(() => {
    const settings = aiStore.settings
    if (!settings) return 'Carregando...'
    const configured = {
      opencode: settings.opencodeModel,
      openrouter: settings.openrouterModel,
      ollama: settings.ollamaModel,
    }[settings.activeDriver]
    return configured || DEFAULT_MODELS[settings.activeDriver] || 'Padrão'
  })

  return { driverLabel, modelLabel }
}
