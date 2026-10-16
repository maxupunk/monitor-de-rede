import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { ApiError, apiService } from '@/services/apiService'
import { useDevicesStore } from './devices'
import type { TopologyActionsByDevice, TopologyNodeAction } from './plugins'
import { formatBps } from '@/utils/formatters'
import type { TopologyEdge as TopologyEdgeBinding } from '@/bindings/TopologyEdge'
import type { TopologyGraph } from '@/bindings/TopologyGraph'
import type { TopologyNode as TopologyNodeBinding } from '@/bindings/TopologyNode'
import type { TopologyPath } from '@/bindings/TopologyPath'

/*
 * Tipos do grafo de topologia: os bindings do `ts-rs` (`@/bindings/Topology*`)
 * são a fonte; aqui só se acrescentam os campos calculados no cliente. O resto
 * da tela importa daqui.
 */

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

export type TopologyNodeStatus = 'online' | 'offline' | 'warning' | 'unknown'
export type TopologyEdgeStatus = 'up' | 'down' | 'unknown'

export type TopologyNode = TopologyNodeBinding

export interface TopologyEdge extends TopologyEdgeBinding {
  /** Rótulos de tráfego calculados no cliente (tooltip, inspetor). */
  inBpsLabel?: string | null
  outBpsLabel?: string | null
}

export type { TopologyPath }

/** Ação de plugin oferecida no nó (`GET /api/plugins/topology-actions`). */
export type TopologyPluginAction = TopologyNodeAction

export type TopologyPluginActionMap = TopologyActionsByDevice

export type TopologyData = TopologyGraph

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

export interface TopologyPosition {
  x: number
  y: number
}

export interface LinkPayload {
  sourceDeviceId: number
  targetDeviceId: number
  sourceInterfaceId?: number | null
  targetInterfaceId?: number | null
  linkType?: string
}

export interface LinkUpdatePayload {
  sourceInterfaceId?: number | null
  targetInterfaceId?: number | null
  linkType?: string
}

function describe(err: unknown, fallback: string): string {
  return err instanceof Error && err.message ? err.message : fallback
}

function siteQuery(siteId: number | null): string {
  return siteId ? `?siteId=${siteId}` : ''
}

/** Normaliza uma aresta: ids de ponta, rótulos de tráfego e status. */
export function normalizeEdge(edge: TopologyEdge): TopologyEdge {
  const inB = edge.inBps
  const outB = edge.outBps
  return {
    ...edge,
    sourceDeviceId: edge.sourceDeviceId ?? edge.source,
    targetDeviceId: edge.targetDeviceId ?? edge.target,
    inBpsLabel: edge.inBpsLabel ?? (inB !== null && inB !== undefined ? formatBps(inB) : null),
    outBpsLabel: edge.outBpsLabel ?? (outB !== null && outB !== undefined ? formatBps(outB) : null),
  }
}

/** Recalcula total, rótulos e status da aresta depois de um patch em tempo real. */
function refreshEdgeDerived(edge: TopologyEdge, trafficChanged: boolean) {
  if (trafficChanged) {
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
    const speed =
      edge.speedBps ??
      minPositive(edge.sourceInterfaceSpeed ?? null, edge.targetInterfaceSpeed ?? null)
    if (speed && speed > 0) {
      edge.utilization = Math.min(1, Math.max(inB ?? 0, outB ?? 0) / speed)
    }
  }
  edge.status =
    edge.sourceInterfaceStatus === 'down' || edge.targetInterfaceStatus === 'down' ? 'down' : 'up'
}

function minPositive(a: number | null, b: number | null): number | null {
  const values = [a, b].filter((v): v is number => v !== null && v > 0)
  return values.length > 0 ? Math.min(...values) : null
}

export const useTopologyStore = defineStore('topology', () => {
  const nodes = ref<TopologyNode[]>([])
  const edges = ref<TopologyEdge[]>([])
  const loading = ref(false)
  const loaded = ref(false)
  const recalculating = ref(false)
  /** Erro ao carregar o mapa (estado da tela). */
  const error = ref<string | null>(null)
  /** Erro da última ação (salvar enlace, excluir…) — os diálogos mostram. */
  const actionError = ref<string | null>(null)
  /** Site em exibição (`null` = todos). Recargas por SSE respeitam o recorte. */
  const siteId = ref<number | null>(null)
  /** Posições persistidas do site em exibição. */
  const layout = ref<Record<number, TopologyPosition>>({})
  /** Sobe a cada recarga do grafo — a tela posiciona os nós novos. */
  const revision = ref(0)
  /** Sobe quando outra aba salvou o layout deste site. */
  const remoteLayoutRevision = ref(0)
  const pluginActions = ref<TopologyPluginActionMap>({})
  const interfaceCache = ref<Map<number, DeviceInterfaceItem[]>>(new Map())

  const nodesById = computed(() => new Map(nodes.value.map((node) => [node.id, node])))
  const edgesById = computed(() => new Map(edges.value.map((edge) => [edge.id, edge])))

  async function fetchTopology(site: number | null = siteId.value, setGlobalLoading = true) {
    siteId.value = site
    if (setGlobalLoading) loading.value = true
    error.value = null
    try {
      const data = await apiService.get<TopologyData>(`/topology${siteQuery(site)}`)
      nodes.value = (data.nodes || []).map((node) => ({
        ...node,
        status: node.status || 'unknown',
      }))
      edges.value = (data.edges || []).map(normalizeEdge)
      loaded.value = true
      revision.value += 1
    } catch (err: unknown) {
      error.value = describe(err, 'Erro ao carregar mapa de topologia')
    } finally {
      if (setGlobalLoading) loading.value = false
    }
  }

  /** Recarga pedida pelo SSE — só para quem já abriu o mapa. */
  function refreshIfLoaded() {
    if (!loaded.value) return
    void fetchTopology(siteId.value, false)
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

  async function mutate(action: () => Promise<unknown>, fallback: string): Promise<boolean> {
    actionError.value = null
    try {
      await action()
      return true
    } catch (err: unknown) {
      actionError.value = describe(err, fallback)
      return false
    }
  }

  async function addLink(payload: LinkPayload): Promise<boolean> {
    const ok = await mutate(
      () => apiService.post('/topology/links', payload),
      'Erro ao adicionar conexão na topologia'
    )
    if (ok) await fetchTopology(siteId.value, false)
    return ok
  }

  async function updateLink(linkId: number, payload: LinkUpdatePayload): Promise<boolean> {
    const ok = await mutate(
      () => apiService.put(`/topology/links/${linkId}`, payload),
      'Erro ao atualizar conexão na topologia'
    )
    if (ok) await fetchTopology(siteId.value, false)
    return ok
  }

  /** Confirma um enlace descoberto/inferido: regravar os mesmos dados o torna do operador. */
  function confirmLink(edge: TopologyEdge): Promise<boolean> {
    return updateLink(edge.id, {
      sourceInterfaceId: edge.sourceInterfaceId ?? null,
      targetInterfaceId: edge.targetInterfaceId ?? null,
      linkType: edge.linkType,
    })
  }

  async function createUnmanagedSwitch(payload: UnmanagedSwitchPayload): Promise<boolean> {
    const ok = await mutate(
      () => apiService.post('/topology/unmanaged-switch', payload),
      'Erro ao cadastrar switch'
    )
    if (ok) {
      await fetchTopology(siteId.value, false)
      void useDevicesStore().fetchDevices()
    }
    return ok
  }

  async function deleteDevice(deviceId: number): Promise<boolean> {
    const ok = await mutate(
      () => apiService.delete(`/devices/${deviceId}`),
      'Erro ao remover dispositivo'
    )
    if (ok) {
      nodes.value = nodes.value.filter((n) => n.id !== deviceId)
      edges.value = edges.value.filter(
        (e) => e.sourceDeviceId !== deviceId && e.targetDeviceId !== deviceId
      )
      const rest = { ...layout.value }
      delete rest[deviceId]
      layout.value = rest
    }
    return ok
  }

  async function deleteLink(linkId: number): Promise<boolean> {
    const ok = await mutate(
      () => apiService.delete(`/topology/links/${linkId}`),
      'Erro ao remover enlace da topologia'
    )
    if (ok) {
      edges.value = edges.value.filter((e) => e.id !== linkId)
      await fetchTopology(siteId.value, false)
      void useDevicesStore().fetchDevices()
    }
    return ok
  }

  async function recalculateTopology(): Promise<boolean> {
    recalculating.value = true
    try {
      const ok = await mutate(
        () => apiService.post('/topology/recalculate'),
        'Erro ao recalcular ligações de topologia'
      )
      if (ok) await fetchTopology(siteId.value, false)
      return ok
    } finally {
      recalculating.value = false
    }
  }

  // --- Layout ---------------------------------------------------------------

  async function fetchLayout(): Promise<Record<number, TopologyPosition>> {
    try {
      const data = await apiService.get<TopologyLayout>(
        `/topology/layout${siteQuery(siteId.value)}`
      )
      const next: Record<number, TopologyPosition> = {}
      for (const item of data?.nodes ?? []) {
        if (Number.isFinite(item.x) && Number.isFinite(item.y)) {
          next[item.deviceId] = { x: item.x, y: item.y }
        }
      }
      layout.value = next
    } catch {
      // Sem layout salvo: a tela organiza os nós.
    }
    return layout.value
  }

  /** Mescla posições só na memória (arranjo local de quem não grava, ou antes do envio). */
  function mergeLayout(positions: Record<number, TopologyPosition>) {
    if (Object.keys(positions).length === 0) return
    layout.value = { ...layout.value, ...positions }
  }

  /**
   * Grava só as posições que mudaram (`PATCH`). Enquanto o backend não tem o
   * `PATCH`, cai no `PUT` com o layout inteiro já mesclado.
   */
  async function patchLayout(changes: TopologyLayoutNode[]): Promise<boolean> {
    if (changes.length === 0) return true
    const merged = { ...layout.value }
    for (const item of changes) merged[item.deviceId] = { x: item.x, y: item.y }
    layout.value = merged
    try {
      // `siteId` do momento do envio: quem troca de site grava o pendente antes.
      await apiService.patch('/topology/layout', { siteId: siteId.value, nodes: changes })
      return true
    } catch (err: unknown) {
      if (err instanceof ApiError && (err.status === 404 || err.status === 405)) {
        const nodesPayload = Object.entries(merged).map(([id, pos]) => ({
          deviceId: Number(id),
          x: pos.x,
          y: pos.y,
        }))
        await apiService.put(`/topology/layout${siteQuery(siteId.value)}`, {
          nodes: nodesPayload,
        })
        return true
      }
      throw err
    }
  }

  /** `topology:layout_updated` — outra aba salvou; a tela relê se não estiver arrastando. */
  function applyLayoutUpdated(data: Record<string, unknown>) {
    const raw = data.siteId
    const eventSite = raw === null || raw === undefined ? null : Number(raw)
    if (eventSite !== siteId.value) return
    remoteLayoutRevision.value += 1
  }

  // --- Consultas sob demanda ------------------------------------------------

  async function fetchPath(from: number, to: number): Promise<TopologyPath | null> {
    actionError.value = null
    try {
      const path = await apiService.get<TopologyPath>(`/topology/path?from=${from}&to=${to}`)
      return {
        ...path,
        deviceIds: Array.isArray(path?.deviceIds) ? path.deviceIds : [],
        edgeIds: Array.isArray(path?.edgeIds) ? path.edgeIds : [],
      }
    } catch (err: unknown) {
      actionError.value = describe(err, 'Não foi possível calcular o caminho')
      return null
    }
  }

  function setPluginActions(map: TopologyPluginActionMap) {
    pluginActions.value = map
  }

  // --- Tempo real (SSE) -----------------------------------------------------

  /** Repinta o nó no mapa quando o dispositivo muda de estado. */
  function applyRealtimeStatus(data: Record<string, unknown>) {
    const id = Number(data.id ?? data.deviceId)
    if (!id) return
    const node = nodesById.value.get(id)
    if (!node) return
    if (typeof data.status === 'string' && data.status) node.status = data.status
    if (typeof data.name === 'string' && data.name) node.name = data.name
    if (typeof data.ipAddress === 'string' && data.ipAddress) node.ipAddress = data.ipAddress
  }

  /** Atualiza tráfego e estado operacional das interfaces e enlaces em tempo real. */
  function applyRealtimeTraffic(data: Record<string, unknown>) {
    const deviceId = Number(data.deviceId)
    if (!deviceId) return

    const rawInterfaces = Array.isArray(data.interfaces)
      ? (data.interfaces as Array<Record<string, unknown>>)
      : []

    if (interfaceCache.value.has(deviceId)) {
      const cachedList = interfaceCache.value.get(deviceId) || []
      for (const item of rawInterfaces) {
        const found = cachedList.find((c) => c.id === Number(item.id))
        if (!found) continue
        if (item.operStatus !== undefined) found.operStatus = String(item.operStatus)
        if (item.adminStatus !== undefined) found.adminStatus = String(item.adminStatus)
        if (item.speed !== undefined && item.speed !== null) found.speed = Number(item.speed)
      }
    }

    const node = nodesById.value.get(deviceId)
    if (node && rawInterfaces.length > 0) {
      node.interfaceCount = Math.max(node.interfaceCount, rawInterfaces.length)
    }

    for (const edge of edges.value) {
      const srcDevId = edge.sourceDeviceId ?? edge.source
      const tgtDevId = edge.targetDeviceId ?? edge.target
      if (srcDevId !== deviceId && tgtDevId !== deviceId) continue
      let updated = false

      if (srcDevId === deviceId) {
        const matched = findInterface(
          rawInterfaces,
          edge.sourceInterfaceId,
          edge.sourceInterfaceName
        )
        if (matched) {
          edge.sourceInterfaceId ??= Number(matched.id)
          if (!edge.sourceInterfaceName && matched.name)
            edge.sourceInterfaceName = String(matched.name)
          if (isNumber(matched.inBps)) edge.inBps = Number(matched.inBps)
          if (isNumber(matched.outBps)) edge.outBps = Number(matched.outBps)
          if (matched.operStatus !== undefined)
            edge.sourceInterfaceStatus = String(matched.operStatus)
          if (isNumber(matched.speed)) edge.sourceInterfaceSpeed = Number(matched.speed)
          updated = true
        }
      }

      if (tgtDevId === deviceId) {
        const matched = findInterface(
          rawInterfaces,
          edge.targetInterfaceId,
          edge.targetInterfaceName
        )
        if (matched) {
          edge.targetInterfaceId ??= Number(matched.id)
          if (!edge.targetInterfaceName && matched.name)
            edge.targetInterfaceName = String(matched.name)
          // O tráfego do enlace é medido pela ponta de origem; a de destino só completa.
          if ((edge.inBps === null || edge.inBps === undefined) && isNumber(matched.inBps)) {
            edge.inBps = Number(matched.inBps)
          }
          if ((edge.outBps === null || edge.outBps === undefined) && isNumber(matched.outBps)) {
            edge.outBps = Number(matched.outBps)
          }
          if (matched.operStatus !== undefined)
            edge.targetInterfaceStatus = String(matched.operStatus)
          if (isNumber(matched.speed)) edge.targetInterfaceSpeed = Number(matched.speed)
          updated = true
        }
      }

      // Enlace sem porta definida num dispositivo de interface única.
      if (
        !updated &&
        rawInterfaces.length === 1 &&
        !edge.sourceInterfaceId &&
        !edge.targetInterfaceId
      ) {
        const single = rawInterfaces[0]
        if (isNumber(single.inBps)) edge.inBps = Number(single.inBps)
        if (isNumber(single.outBps)) edge.outBps = Number(single.outBps)
        if (single.operStatus) {
          if (srcDevId === deviceId) edge.sourceInterfaceStatus = String(single.operStatus)
          else edge.targetInterfaceStatus = String(single.operStatus)
        }
        updated = true
      }

      if (updated) refreshEdgeDerived(edge, true)
    }
  }

  /** Atualiza estado e velocidade de interface nas arestas correspondentes. */
  function applyInterfaceChange(type: string, data: Record<string, unknown>) {
    const interfaceId = Number(data.interfaceId)
    if (!interfaceId) return

    const status =
      type === 'interface:status_change' && data.currentStatus ? String(data.currentStatus) : null
    const speed = isNumber(data.currentSpeed) ? Number(data.currentSpeed) : null
    for (const edge of edges.value) {
      let updated = false
      if (edge.sourceInterfaceId === interfaceId) {
        if (status) edge.sourceInterfaceStatus = status
        if (speed !== null) edge.sourceInterfaceSpeed = speed
        updated = true
      }
      if (edge.targetInterfaceId === interfaceId) {
        if (status) edge.targetInterfaceStatus = status
        if (speed !== null) edge.targetInterfaceSpeed = speed
        updated = true
      }
      if (updated) refreshEdgeDerived(edge, false)
    }
  }

  return {
    nodes,
    edges,
    nodesById,
    edgesById,
    loading,
    loaded,
    recalculating,
    error,
    actionError,
    siteId,
    layout,
    revision,
    remoteLayoutRevision,
    pluginActions,
    interfaceCache,
    fetchTopology,
    refreshIfLoaded,
    fetchDeviceInterfaces,
    addLink,
    updateLink,
    confirmLink,
    createUnmanagedSwitch,
    deleteDevice,
    deleteLink,
    recalculateTopology,
    fetchLayout,
    mergeLayout,
    patchLayout,
    applyLayoutUpdated,
    fetchPath,
    setPluginActions,
    applyRealtimeStatus,
    applyRealtimeTraffic,
    applyInterfaceChange,
  }
})

function isNumber(value: unknown): boolean {
  return value !== undefined && value !== null && Number.isFinite(Number(value))
}

function findInterface(
  list: Array<Record<string, unknown>>,
  id: number | null | undefined,
  name: string | null | undefined
): Record<string, unknown> | undefined {
  return list.find(
    (item) =>
      (id && Number(item.id) === id) ||
      (name && String(item.name).toLowerCase() === name.toLowerCase())
  )
}
