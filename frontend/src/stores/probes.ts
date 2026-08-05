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

  /** Aplica o payload SSE `probe:status` no probe já carregado */
  function applyRealtimeStatus(data: Record<string, unknown>) {
    const id = Number(data.id ?? data.probeId)
    if (!id) return
    const probe = probes.value.find((p) => p.id === id)
    if (!probe) {
      // Probe novo (ex.: primeiro heartbeat) — recarrega a lista
      fetchProbes()
      return
    }

    if (data.status) probe.status = data.status as Probe['status']
    if (data.name) probe.name = String(data.name)
    if (data.lastSeenAt) probe.lastHeartbeatAt = String(data.lastSeenAt)
  }

  return {
    probes,
    loading,
    testingId,
    error,
    applyRealtimeStatus,
    fetchProbes,
    revokeProbe,
    testProbe,
  }
})
