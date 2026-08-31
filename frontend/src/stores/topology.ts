import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'

export interface DeviceInterfaceItem {
  id: number
  deviceId: number
  snmpIndex?: number
  name: string
  description?: string
  alias?: string
  macAddress?: string
  type?: string
  speed?: number
  adminStatus?: 'up' | 'down' | string
  operStatus?: 'up' | 'down' | string
  isMonitored?: boolean
}

export interface TopologyNode {
  id: number
  name: string
  type: string
  vendor?: string
  model?: string
  status: 'online' | 'offline' | 'warning' | 'unknown' | string
  siteId?: number | null
  interfaceCount: number
  ipAddress?: string
  snmpEnabled?: boolean
  parentId?: number | null
  x?: number
  y?: number
}

export interface TopologyEdge {
  id: number
  source: number
  target: number
  sourceDeviceId?: number
  targetDeviceId?: number
  sourceDeviceName?: string
  targetDeviceName?: string
  sourceInterfaceId?: number | null
  targetInterfaceId?: number | null
  sourceInterfaceName?: string | null
  targetInterfaceName?: string | null
  sourceInterfaceSpeed?: number | null
  targetInterfaceSpeed?: number | null
  sourceInterfaceStatus?: string | null
  targetInterfaceStatus?: string | null
  inBps?: number | null
  outBps?: number | null
  trafficBps?: number | null
  trafficLabel?: string | null
  linkType: string
  discoveryMethod: string
  confidence?: number
  confirmed?: boolean
  status?: string
}

export interface TopologyData {
  nodes: TopologyNode[]
  edges: TopologyEdge[]
}

export interface UnmanagedSwitchPayload {
  name: string
  vendor?: string
  model?: string
  portCount: number
  siteId?: number | null
  networkId?: number | null
}

export interface TopologyLayoutNode {
  deviceId: number
  x: number
  y: number
}

export interface TopologyLayout {
  nodes: TopologyLayoutNode[]
}

export const useTopologyStore = defineStore('topology', () => {
  const nodes = ref<TopologyNode[]>([])
  const edges = ref<TopologyEdge[]>([])
  const loading = ref(false)
  const recalculating = ref(false)
  const error = ref<string | null>(null)
  const interfaceCache = ref<Map<number, DeviceInterfaceItem[]>>(new Map())

  async function fetchTopology(siteId?: number | null, setGlobalLoading = true, live = true) {
    if (setGlobalLoading) {
      loading.value = true
    }
    error.value = null
    try {
      const params = new URLSearchParams()
      if (siteId) params.set('siteId', siteId.toString())
      if (live) params.set('live', 'true')
      const query = params.toString() ? `?${params.toString()}` : ''
      const data = await apiService.get<TopologyData>(`/topology${query}`)
      nodes.value = (data.nodes || []).map((node) => ({
        ...node,
        status: node.status || 'unknown',
      }))
      edges.value = (data.edges || []).map((edge) => ({
        ...edge,
        sourceDeviceId: edge.sourceDeviceId ?? edge.source,
        targetDeviceId: edge.targetDeviceId ?? edge.target,
      }))
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao carregar mapa de topologia'
    } finally {
      if (setGlobalLoading) {
        loading.value = false
      }
    }
  }

  async function fetchDeviceInterfaces(
    deviceId: number,
    forceRefresh = false
  ): Promise<DeviceInterfaceItem[]> {
    if (!forceRefresh && interfaceCache.value.has(deviceId)) {
      return interfaceCache.value.get(deviceId) || []
    }
    try {
      const ifaces = await apiService.get<DeviceInterfaceItem[]>(`/devices/${deviceId}/interfaces`)
      interfaceCache.value.set(deviceId, ifaces || [])
      return ifaces || []
    } catch {
      return []
    }
  }

  async function addLink(payload: {
    sourceDeviceId: number
    targetDeviceId: number
    sourceInterfaceId?: number | null
    targetInterfaceId?: number | null
    linkType?: string
  }): Promise<boolean> {
    try {
      await apiService.post('/topology/links', payload)
      await fetchTopology(null, false)
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao adicionar conexão na topologia'
      return false
    }
  }

  async function createUnmanagedSwitch(payload: UnmanagedSwitchPayload): Promise<boolean> {
    try {
      await apiService.post('/topology/unmanaged-switch', payload)
      await fetchTopology(null, false)
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao cadastrar switch'
      return false
    }
  }

  async function updateLink(
    linkId: number,
    payload: {
      sourceInterfaceId?: number | null
      targetInterfaceId?: number | null
      linkType?: string
    }
  ): Promise<boolean> {
    try {
      await apiService.put(`/topology/links/${linkId}`, payload)
      await fetchTopology(null, false)
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao atualizar conexão na topologia'
      return false
    }
  }

  async function deleteDevice(deviceId: number): Promise<boolean> {
    try {
      await apiService.delete(`/devices/${deviceId}`)
      nodes.value = nodes.value.filter((n) => n.id !== deviceId)
      edges.value = edges.value.filter(
        (e) => e.sourceDeviceId !== deviceId && e.targetDeviceId !== deviceId
      )
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao remover dispositivo'
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
      await fetchTopology(null, false)
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao recalcular ligações de topologia'
      return false
    } finally {
      recalculating.value = false
    }
  }

  /** Repinta o nó no mapa quando o dispositivo muda de estado */
  function applyRealtimeStatus(data: Record<string, unknown>) {
    const id = Number(data.id ?? data.deviceId)
    if (!id) return
    const node = nodes.value.find((n) => n.id === id)
    if (node && data.status) {
      node.status = data.status as TopologyNode['status']
    }
  }

  async function fetchTopologyLayout(siteId?: number | null): Promise<TopologyLayout | null> {
    try {
      const params = new URLSearchParams()
      if (siteId) params.set('siteId', siteId.toString())
      const query = params.toString() ? `?${params.toString()}` : ''
      return await apiService.get<TopologyLayout>(`/topology/layout${query}`)
    } catch {
      return null
    }
  }

  async function saveTopologyLayout(
    positions: Map<number, { x: number; y: number }>,
    siteId?: number | null
  ): Promise<boolean> {
    try {
      const nodes: TopologyLayoutNode[] = []
      positions.forEach((pos, id) => {
        nodes.push({ deviceId: id, x: pos.x, y: pos.y })
      })
      const params = new URLSearchParams()
      if (siteId) params.set('siteId', siteId.toString())
      const query = params.toString() ? `?${params.toString()}` : ''
      await apiService.put(`/topology/layout${query}`, { nodes })
      return true
    } catch {
      return false
    }
  }

  return {
    nodes,
    edges,
    loading,
    recalculating,
    error,
    interfaceCache,
    applyRealtimeStatus,
    fetchTopology,
    fetchTopologyLayout,
    fetchDeviceInterfaces,
    addLink,
    updateLink,
    createUnmanagedSwitch,
    deleteDevice,
    deleteLink,
    recalculateTopology,
    saveTopologyLayout,
  }
})
