import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'
import type { Preferences } from '@/bindings/Preferences'

export type { Preferences }

/** Espelham as constantes do backend; a validação de verdade é lá. */
export const MIN_PING_INTERVAL_SECONDS = 10
export const MAX_PING_INTERVAL_SECONDS = 86_400

export function defaultPreferences(): Preferences {
  return {
    defaultPingIntervalSeconds: 60,
    defaultSnmpCommunity: 'public',
    autoDiscoveryEnabled: true,
  }
}

/**
 * Preferências globais do sistema.
 *
 * Cada uma tem um ponto de consumo real no backend — intervalo de monitor novo,
 * comunidade de dispositivo SNMP novo e trava da varredura periódica. A tela
 * diz qual é: uma preferência sem efeito declarado é indistinguível de uma que
 * não funciona.
 */
export const usePreferencesStore = defineStore('preferences', () => {
  const preferences = ref<Preferences>(defaultPreferences())
  const loading = ref(false)
  const saving = ref(false)
  const loaded = ref(false)
  const error = ref('')

  async function fetchAll(force = false): Promise<void> {
    if (loading.value || (loaded.value && !force)) return
    loading.value = true
    error.value = ''
    try {
      preferences.value = await apiService.get<Preferences>('/settings')
      loaded.value = true
    } catch (erro) {
      error.value =
        erro instanceof Error ? erro.message : 'Não foi possível carregar as preferências.'
    } finally {
      loading.value = false
    }
  }

  /**
   * Grava e adota o que o servidor devolveu.
   *
   * O corpo da resposta é o documento **já validado e aparado** — adotar o que
   * foi digitado deixaria a tela mostrando uma coisa e o sistema usando outra.
   */
  async function save(valores: Preferences): Promise<boolean> {
    saving.value = true
    error.value = ''
    try {
      preferences.value = await apiService.put<Preferences>('/settings', valores)
      loaded.value = true
      return true
    } catch (erro) {
      error.value =
        erro instanceof Error ? erro.message : 'Não foi possível salvar as preferências.'
      return false
    } finally {
      saving.value = false
    }
  }

  return { preferences, loading, saving, loaded, error, fetchAll, save }
})
