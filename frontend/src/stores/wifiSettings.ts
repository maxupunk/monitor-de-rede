import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'
import type { WifiSettings } from '@/bindings/WifiSettings'

function messageOf(error: unknown, fallback: string): string {
  return error instanceof Error && error.message ? error.message : fallback
}

/**
 * As Configurações de Wi-Fi: de quanto em quanto tempo o mapa e a leitura em
 * segundo plano leem os APs, o ambiente da estimativa de distância e o
 * histórico de roaming. Lidas ao abrir o cartão e gravadas pelo botão — sem
 * ciclo automático (AGENTS §9).
 */
export const useWifiSettingsStore = defineStore('wifiSettings', () => {
  const settings = ref<WifiSettings | null>(null)
  const loading = ref(false)
  const saving = ref(false)
  const error = ref<string | null>(null)

  async function fetch(): Promise<WifiSettings | null> {
    loading.value = true
    error.value = null
    try {
      settings.value = await apiService.get<WifiSettings>('/wifi/settings')
      return settings.value
    } catch (err: unknown) {
      error.value = messageOf(err, 'Não foi possível carregar as configurações de Wi-Fi.')
      return null
    } finally {
      loading.value = false
    }
  }

  async function save(input: WifiSettings): Promise<boolean> {
    saving.value = true
    error.value = null
    try {
      settings.value = await apiService.put<WifiSettings>('/wifi/settings', input)
      return true
    } catch (err: unknown) {
      error.value = messageOf(err, 'Não foi possível salvar as configurações de Wi-Fi.')
      return false
    } finally {
      saving.value = false
    }
  }

  return { settings, loading, saving, error, fetch, save }
})
