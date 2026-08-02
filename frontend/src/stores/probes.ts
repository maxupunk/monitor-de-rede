import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'

export interface Probe {
  id: number
  name: string
  location?: string
  ipAddress?: string
  status: 'online' | 'offline' | 'revoked'
  lastHeartbeatAt?: string
  createdAt?: string
}

export const useProbesStore = defineStore('probes', () => {
  const probes = ref<Probe[]>([])
  const loading = ref(false)
  const testingId = ref<number | null>(null)
  const error = ref<string | null>(null)

  async function fetchProbes() {
    loading.value = true
    error.value = null
    try {
      probes.value = await apiService.get<Probe[]>('/probes')
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao carregar probes'
    } finally {
      loading.value = false
    }
  }

  async function revokeProbe(probeId: number): Promise<boolean> {
    try {
      await apiService.post(`/probes/${probeId}/revoke`)
      const p = probes.value.find((item) => item.id === probeId)
      if (p) p.status = 'revoked'
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao revogar probe'
      return false
    }
  }

  async function testProbe(probeId: number): Promise<boolean> {
    testingId.value = probeId
    try {
      await apiService.post(`/probes/${probeId}/test`)
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao testar comunicação com probe'
      return false
    } finally {
      testingId.value = null
    }
  }

  return {
    probes,
    loading,
    testingId,
    error,
    fetchProbes,
    revokeProbe,
    testProbe,
  }
})
