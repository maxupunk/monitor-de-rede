import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'
import { useDevicesStore } from './devices'
import { formatBps } from '@/utils/formatters'

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
  inBpsLabel?: string | null
  outBpsLabel?: string | null
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
      edges.value = (data.edges || []).map((edge) => {
        const inB = edge.inBps
        const outB = edge.outBps
        return {
          ...edge,
          sourceDeviceId: edge.sourceDeviceId ?? edge.source,
          targetDeviceId: edge.targetDeviceId ?? edge.target,
          inBpsLabel:
            edge.inBpsLabel ?? (inB !== null && inB !== undefined ? formatBps(inB) : null),
          outBpsLabel:
            edge.outBpsLabel ?? (outB !== null && outB !== undefined ? formatBps(outB) : null),
        }
      })
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
      const devicesStore = useDevicesStore()
      void devicesStore.fetchDevices()
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
      await fetchTopology(null, false)
      const devicesStore = useDevicesStore()
      void devicesStore.fetchDevices()
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

  /** Atualiza o tráfego e estado operacional das interfaces e enlaces em tempo real */
  function applyRealtimeTraffic(data: Record<string, unknown>) {
    const deviceId = Number(data.deviceId)
    if (!deviceId) return

    const rawInterfaces = Array.isArray(data.interfaces)
      ? (data.interfaces as Array<Record<string, unknown>>)
      : []

    // 1. Atualiza cache de interfaces se existir
    if (interfaceCache.value.has(deviceId)) {
      const cachedList = interfaceCache.value.get(deviceId) || []
      for (const item of rawInterfaces) {
        const ifId = Number(item.id)
        const found = cachedList.find((c) => c.id === ifId)
        if (found) {
          if (item.operStatus !== undefined) found.operStatus = String(item.operStatus)
          if (item.adminStatus !== undefined) found.adminStatus = String(item.adminStatus)
          if (item.speed !== undefined && item.speed !== null) found.speed = Number(item.speed)
        }
      }
    }

    // 2. Atualiza contagem de interfaces no nó
    const node = nodes.value.find((n) => n.id === deviceId)
    if (node && rawInterfaces.length > 0) {
      node.interfaceCount = Math.max(node.interfaceCount, rawInterfaces.length)
    }

    // 3. Atualiza arestas / enlaces
    for (const edge of edges.value) {
      let updated = false
      const srcDevId = edge.sourceDeviceId ?? edge.source
      const tgtDevId = edge.targetDeviceId ?? edge.target

      if (srcDevId === deviceId) {
        const matched = rawInterfaces.find(
          (i) =>
            (edge.sourceInterfaceId && Number(i.id) === edge.sourceInterfaceId) ||
            (edge.sourceInterfaceName &&
              String(i.name).toLowerCase() === edge.sourceInterfaceName.toLowerCase())
        )
        if (matched) {
          if (!edge.sourceInterfaceId) {
            edge.sourceInterfaceId = Number(matched.id)
          }
          if (!edge.sourceInterfaceName && matched.name) {
            edge.sourceInterfaceName = String(matched.name)
          }
          if (matched.inBps !== undefined && matched.inBps !== null) {
            edge.inBps = Number(matched.inBps)
          }
          if (matched.outBps !== undefined && matched.outBps !== null) {
            edge.outBps = Number(matched.outBps)
          }
          if (matched.operStatus !== undefined) {
            edge.sourceInterfaceStatus = String(matched.operStatus)
          }
          if (matched.speed !== undefined && matched.speed !== null) {
            edge.sourceInterfaceSpeed = Number(matched.speed)
          }
          updated = true
        }
      }

      if (tgtDevId === deviceId) {
        const matched = rawInterfaces.find(
          (i) =>
            (edge.targetInterfaceId && Number(i.id) === edge.targetInterfaceId) ||
            (edge.targetInterfaceName &&
              String(i.name).toLowerCase() === edge.targetInterfaceName.toLowerCase())
        )
        if (matched) {
          if (!edge.targetInterfaceId) {
            edge.targetInterfaceId = Number(matched.id)
          }
          if (!edge.targetInterfaceName && matched.name) {
            edge.targetInterfaceName = String(matched.name)
          }
          if (edge.inBps === null || edge.inBps === undefined) {
            if (matched.inBps !== undefined && matched.inBps !== null) {
              edge.inBps = Number(matched.inBps)
            }
          }
          if (edge.outBps === null || edge.outBps === undefined) {
            if (matched.outBps !== undefined && matched.outBps !== null) {
              edge.outBps = Number(matched.outBps)
            }
          }
          if (matched.operStatus !== undefined) {
            edge.targetInterfaceStatus = String(matched.operStatus)
          }
          if (matched.speed !== undefined && matched.speed !== null) {
            edge.targetInterfaceSpeed = Number(matched.speed)
          }
          updated = true
        }
      }

      // Fallback para enlaces onde uma das pontas é o dispositivo com interface única
      if (!updated && (srcDevId === deviceId || tgtDevId === deviceId)) {
        if (rawInterfaces.length === 1 && !edge.sourceInterfaceId && !edge.targetInterfaceId) {
          const single = rawInterfaces[0]
          if (single.inBps !== undefined && single.inBps !== null) {
            edge.inBps = Number(single.inBps)
          }
          if (single.outBps !== undefined && single.outBps !== null) {
            edge.outBps = Number(single.outBps)
          }
          if (single.operStatus) {
            if (srcDevId === deviceId) {
              edge.sourceInterfaceStatus = String(single.operStatus)
            } else {
              edge.targetInterfaceStatus = String(single.operStatus)
            }
          }
          updated = true
        }
      }

      if (updated) {
        const inB = edge.inBps
        const outB = edge.outBps
        const total =
          inB !== null && inB !== undefined && outB !== null && outB !== undefined
            ? inB + outB
            : (inB ?? outB ?? null)

        edge.trafficBps = total
        edge.trafficLabel = total !== null ? formatBps(total) : '0 bps'
        edge.inBpsLabel = inB !== null && inB !== undefined ? formatBps(inB) : '0 bps'
        edge.outBpsLabel = outB !== null && outB !== undefined ? formatBps(outB) : '0 bps'

        if (edge.sourceInterfaceStatus === 'down' || edge.targetInterfaceStatus === 'down') {
          edge.status = 'down'
        } else {
          edge.status = 'up'
        }
      }
    }
  }

  /** Atualiza estado e velocidade de interface nas arestas correspondentes */
  function applyInterfaceChange(type: string, data: Record<string, unknown>) {
    const interfaceId = Number(data.interfaceId)
    if (!interfaceId) return

    for (const edge of edges.value) {
      let updated = false
      if (edge.sourceInterfaceId === interfaceId) {
        if (type === 'interface:status_change' && data.currentStatus) {
          edge.sourceInterfaceStatus = String(data.currentStatus)
        }
        if (data.currentSpeed !== undefined && data.currentSpeed !== null) {
          edge.sourceInterfaceSpeed = Number(data.currentSpeed)
        }
        updated = true
      }
      if (edge.targetInterfaceId === interfaceId) {
        if (type === 'interface:status_change' && data.currentStatus) {
          edge.targetInterfaceStatus = String(data.currentStatus)
        }
        if (data.currentSpeed !== undefined && data.currentSpeed !== null) {
          edge.targetInterfaceSpeed = Number(data.currentSpeed)
        }
        updated = true
      }
      if (updated) {
        if (edge.sourceInterfaceStatus === 'down' || edge.targetInterfaceStatus === 'down') {
          edge.status = 'down'
        } else {
          edge.status = 'up'
        }
      }
    }
  }

  /** Aplica amostras numéricas de inBps/outBps de monitores de interface */
  function applyRealtimeMetric(data: Record<string, unknown>) {
    const samples = (data.metrics as Array<Record<string, unknown>>) || []
    for (const sample of samples) {
      const ifaceId = sample.interfaceId ? Number(sample.interfaceId) : null
      if (!ifaceId) continue
      const name = String(sample.name)
      const val = Number(sample.value)
      if (Number.isNaN(val)) continue

      for (const edge of edges.value) {
        let updated = false
        if (edge.sourceInterfaceId === ifaceId || edge.targetInterfaceId === ifaceId) {
          if (name === 'inBps' || name === 'traffic_rx_bytes') {
            edge.inBps = val
            updated = true
          } else if (name === 'outBps' || name === 'traffic_tx_bytes') {
            edge.outBps = val
            updated = true
          }
        }
        if (updated) {
          const inB = edge.inBps
          const outB = edge.outBps
          const total =
            inB !== null && inB !== undefined && outB !== null && outB !== undefined
              ? inB + outB
              : (inB ?? outB ?? null)

          edge.trafficBps = total
          edge.trafficLabel = total !== null ? formatBps(total) : '0 bps'
          edge.inBpsLabel = inB !== null && inB !== undefined ? formatBps(inB) : '0 bps'
          edge.outBpsLabel = outB !== null && outB !== undefined ? formatBps(outB) : '0 bps'
        }
      }
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
    applyRealtimeTraffic,
    applyInterfaceChange,
    applyRealtimeMetric,
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
