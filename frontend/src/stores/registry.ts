import { ref } from 'vue'
import { defineStore } from 'pinia'
import { apiService } from '@/services/apiService'
import type { RegistryExceptionInput } from '@/bindings/RegistryExceptionInput'
import type { RegistryExceptionView } from '@/bindings/RegistryExceptionView'
import type { RegistryFindingView } from '@/bindings/RegistryFindingView'
import type { RegistryOverview } from '@/bindings/RegistryOverview'
import type { RegistryRepositoryDetail } from '@/bindings/RegistryRepositoryDetail'
import type { RegistrySettings } from '@/bindings/RegistrySettings'
import type { RegistryTokenCreated } from '@/bindings/RegistryTokenCreated'
import type { RegistryTokenInput } from '@/bindings/RegistryTokenInput'
import type { RegistryTokenView } from '@/bindings/RegistryTokenView'

/**
 * O registry de imagens da central (ADR 022). A visão geral é lida ao abrir a
 * página; `registry:changed` (imagem nova, análise concluída, limpeza) a
 * relê uma vez — sem consulta periódica (AGENTS §9).
 */
export const useRegistryStore = defineStore('registry', () => {
  const overview = ref<RegistryOverview | null>(null)
  const loading = ref(false)
  const error = ref<string | null>(null)
  /** Quantas vezes o catálogo mudou: quem mostra um detalhe relê ao ver mudar. */
  const revision = ref(0)

  async function fetchOverview(): Promise<void> {
    loading.value = true
    error.value = null
    try {
      overview.value = await apiService.get<RegistryOverview>('/registry')
    } catch (err) {
      error.value = err instanceof Error ? err.message : 'Erro ao carregar o registry'
    } finally {
      loading.value = false
    }
  }

  /** O que o SSE chama. */
  function refreshIfLoaded() {
    revision.value += 1
    if (overview.value) void fetchOverview()
  }

  const detail = (repositoryId: number) =>
    apiService.get<RegistryRepositoryDetail>(`/registry/repositories/${repositoryId}`)
  const findings = (versionId: number) =>
    apiService.get<RegistryFindingView[]>(`/registry/versions/${versionId}/findings`)
  const rescan = (versionId: number) =>
    apiService.post<RegistryFindingView[]>(`/registry/versions/${versionId}/scan`, {})
  const pin = (versionId: number, pinned: boolean) =>
    apiService.post(`/registry/versions/${versionId}/pin`, { pinned })
  const removeVersion = (versionId: number) => apiService.delete(`/registry/versions/${versionId}`)

  async function saveSettings(settings: RegistrySettings) {
    await apiService.put<RegistrySettings>('/registry/settings', settings)
    await fetchOverview()
  }

  const collect = () => apiService.post<{ freedBytes: number }>('/registry/collect', {})
  const syncAdvisories = () => apiService.post('/registry/advisories/sync', {})

  const tokens = () => apiService.get<RegistryTokenView[]>('/registry/tokens')
  const createToken = (input: RegistryTokenInput) =>
    apiService.post<RegistryTokenCreated>('/registry/tokens', input)
  const revokeToken = (id: number) => apiService.delete(`/registry/tokens/${id}`)

  const exceptions = () => apiService.get<RegistryExceptionView[]>('/registry/exceptions')
  const addException = (input: RegistryExceptionInput) =>
    apiService.post('/registry/exceptions', input)
  const removeException = (id: number) => apiService.delete(`/registry/exceptions/${id}`)

  return {
    overview,
    loading,
    error,
    revision,
    fetchOverview,
    refreshIfLoaded,
    detail,
    findings,
    rescan,
    pin,
    removeVersion,
    saveSettings,
    collect,
    syncAdvisories,
    tokens,
    createToken,
    revokeToken,
    exceptions,
    addException,
    removeException,
  }
})
