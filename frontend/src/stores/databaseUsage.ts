import { ref } from 'vue'
import { defineStore } from 'pinia'
import { apiService } from '@/services/apiService'
import type { DatabaseUsage } from '@/bindings/DatabaseUsage'

/**
 * O que ocupa o espaço do banco (`GET /settings/database-usage`).
 *
 * Carrega sob demanda — quando o diálogo abre ou o operador pede para
 * atualizar —, nunca sozinho: medir tabela por tabela conta linhas no SQLite,
 * e isso não é coisa para rodar em ciclo.
 */
export const useDatabaseUsageStore = defineStore('databaseUsage', () => {
  const usage = ref<DatabaseUsage | null>(null)
  const loading = ref(false)
  const error = ref<string | null>(null)

  async function fetchUsage(): Promise<DatabaseUsage | null> {
    loading.value = true
    error.value = null
    try {
      // Contar as linhas de um banco grande leva alguns segundos.
      usage.value = await apiService.get<DatabaseUsage>('/settings/database-usage', {
        timeoutMs: 120_000,
      })
    } catch (err) {
      error.value =
        err instanceof Error ? err.message : 'Não foi possível medir o espaço ocupado pelo banco.'
    } finally {
      loading.value = false
    }
    return usage.value
  }

  return { usage, loading, error, fetchUsage }
})
