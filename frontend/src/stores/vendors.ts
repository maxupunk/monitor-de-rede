import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'

/** De onde veio o nome: registro do IEEE, tabela embutida ou ninguém sabe. */
export type VendorSource = 'ieee' | 'builtin' | 'none'

/** O tipo de aparelho que o fabricante sugere sozinho. */
export interface VendorTypeHint {
  deviceType: string
  confidence: number
  reasons: string[]
}

export interface MacVendorLookup {
  /** Nome curto, para a tela ("Espressif"). */
  vendor: string | null
  /** Nome completo registrado ("Espressif Inc."). */
  organization: string | null
  source: VendorSource
  /** MAC aleatório (privacidade) ou de máquina virtual — sem dono no IEEE. */
  locallyAdministered: boolean
  hint: VendorTypeHint | null
}

export interface VendorRegistryStatus {
  entries: number
  updatedAt: string | null
  builtinEntries: number
  stale: boolean
  autoUpdate: boolean
  updating: boolean
  sources: string[]
}

export interface VendorRefreshOutcome {
  entries: number
  byRegistry: Record<string, number>
  updatedAt: string
}

/** Dígitos hexadecimais do MAC — a chave do cache e o que o backend aceita. */
export function macDigits(mac: string): string {
  return mac.replace(/[^0-9a-f]/gi, '').toLowerCase()
}

/** Mínimo para consultar: os 6 dígitos do bloco do fabricante. */
export const MIN_MAC_DIGITS = 6

export const useVendorsStore = defineStore('vendors', () => {
  const status = ref<VendorRegistryStatus | null>(null)
  const loadingStatus = ref(false)
  const refreshing = ref(false)
  const error = ref<string | null>(null)
  const cache = new Map<string, MacVendorLookup>()

  async function fetchStatus(): Promise<VendorRegistryStatus | null> {
    loadingStatus.value = true
    try {
      status.value = await apiService.get<VendorRegistryStatus>('/vendors/status')
      return status.value
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao consultar a base de fabricantes'
      return null
    } finally {
      loadingStatus.value = false
    }
  }

  /** Baixa o registro do IEEE de novo. Pode levar um minuto num enlace lento. */
  async function refresh(): Promise<VendorRefreshOutcome | null> {
    refreshing.value = true
    error.value = null
    try {
      const outcome = await apiService.post<VendorRefreshOutcome>('/vendors/refresh', undefined, {
        timeoutMs: 5 * 60_000,
      })
      cache.clear()
      await fetchStatus()
      return outcome
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao atualizar a base de fabricantes'
      return null
    } finally {
      refreshing.value = false
    }
  }

  /** Fabricante de um MAC; o mesmo bloco só é perguntado uma vez. */
  async function lookup(mac: string): Promise<MacVendorLookup | null> {
    const digits = macDigits(mac)
    if (digits.length < MIN_MAC_DIGITS) return null
    const key = digits.slice(0, 12)
    const cached = cache.get(key)
    if (cached) return cached
    try {
      const result = await apiService.get<MacVendorLookup>(
        `/vendors/lookup?mac=${encodeURIComponent(key)}`
      )
      cache.set(key, result)
      return result
    } catch {
      return null
    }
  }

  return { status, loadingStatus, refreshing, error, fetchStatus, refresh, lookup }
})
