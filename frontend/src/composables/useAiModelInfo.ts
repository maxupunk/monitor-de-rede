import { computed } from 'vue'
import { useAiStore } from '@/stores/ai'
import {
  DEFAULT_MODELS,
  DRIVER_LABELS,
  modelForDriver,
  type AiDriver,
} from '@/components/settings/ai/aiProviders'

function isKnownDriver(driver: string | undefined): driver is AiDriver {
  return driver !== undefined && driver in DRIVER_LABELS
}

/** Provedor e modelo configurados do Assistente IA, para cabeçalhos e cartões. */
export function useAiModelInfo() {
  const aiStore = useAiStore()

  const driverLabel = computed(() => {
    const driver = aiStore.settings?.activeDriver
    return isKnownDriver(driver) ? DRIVER_LABELS[driver] : 'Provedor desconhecido'
  })

  const modelLabel = computed(() => {
    const settings = aiStore.settings
    if (!settings) return 'Carregando...'
    if (!isKnownDriver(settings.activeDriver)) return 'Padrão'
    return modelForDriver(settings) || DEFAULT_MODELS[settings.activeDriver]
  })

  return { driverLabel, modelLabel }
}
