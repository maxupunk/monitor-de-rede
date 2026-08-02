import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'

export interface TopologyNode {
  id: number
  name: string
  type: string
  vendor?: string
  status: 'online' | 'offline' | 'warning' | 'unknown'
  ipAddress?: string
  x?: number
  y?: number
}

export interface TopologyEdge {
  id: number
  sourceDeviceId: number
  targetDeviceId: number
  sourceInterfaceId?: number
  targetInterfaceId?: number
  linkType: 'manual' | 'lldp' | 'cdp' | 'subnet'
  confidenceScore?: number
  status?: string
}

export interface TopologyData {
  nodes: TopologyNode[]
  edges: TopologyEdge[]
}

export const useTopologyStore = defineStore('topology', () => {
  const nodes = ref<TopologyNode[]>([])
  const edges = ref<TopologyEdge[]>([])
  const loading = ref(false)
  const recalculating = ref(false)
  const error = ref<string | null>(null)

  async function fetchTopology() {
    loading.value = true
    error.value = null
    try {
      const data = await apiService.get<TopologyData>('/topology')
      nodes.value = data.nodes || []
      edges.value = data.edges || []
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao carregar mapa de topologia'
    } finally {
      loading.value = false
    }
  }

  async function addLink(payload: {
    sourceDeviceId: number
    targetDeviceId: number
    sourceInterfaceId?: number
    targetInterfaceId?: number
  }): Promise<boolean> {
    try {
      await apiService.post('/topology/links', payload)
      await fetchTopology()
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao adicionar conexão na topologia'
      return false
    }
  }

  async function deleteLink(linkId: number): Promise<boolean> {
    try {
      await apiService.delete(`/topology/links/${linkId}`)
      edges.value = edges.value.filter((e) => e.id !== linkId)
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao remover link da topologia'
      return false
    }
  }

  async function recalculateTopology(): Promise<boolean> {
    recalculating.value = true
    try {
      await apiService.post('/topology/recalculate')
      await fetchTopology()
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao recalcular ligações de topologia'
      return false
    } finally {
      recalculating.value = false
    }
  }

  return {
    nodes,
    edges,
    loading,
    recalculating,
    error,
    fetchTopology,
    addLink,
    deleteLink,
    recalculateTopology,
  }
})
