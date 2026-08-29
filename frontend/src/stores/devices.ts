import { defineStore } from 'pinia'
import { computed } from 'vue'
import { apiService } from '@/services/apiService'
import { useCrudResource } from './crudResource'
import type { VpnDeviceProfile, VpnConnectionStatus } from './vpn'

export interface DeviceVpnPeer {
  id: number
  vpnServerId: number
  deviceId: number
  publicKey: string
  deviceProfile: VpnDeviceProfile
  persistentKeepalive: number
  lastHandshakeAt: string | null
  bytesRx: number
  bytesTx: number
  enabled: boolean
  connectionStatus: VpnConnectionStatus
}

export interface Device {
  id: number
  siteId?: number | null
  networkId?: number | null
  parentId?: number | null
  name: string
  type: string
  vendor?: string
  model?: string
  ipAddress?: string
  macAddress?: string
  isMonitored?: boolean
  status: 'online' | 'offline' | 'warning' | 'unknown'
  snmpEnabled?: boolean
  snmpCommunity?: string
  snmpVersion?: 'v1' | 'v2c' | 'v3'
  snmpPollIntervalSeconds?: number
  /** O que o operador declarou. `null` = automático; ver `utils/accessMode`. */
  accessMode?: 'local' | 'vpn' | 'remote' | null
  /** O que o sistema vai usar de fato — declarado ou deduzido. */
  effectiveAccessMode?: 'local' | 'vpn' | 'remote'
  /** Por que essa conclusão. */
  accessModeReason?: string
  accessModeDeclared?: boolean
  /** Sistema declarado no cadastro. `null` = automático. */
  operatingSystem?: string | null
  /** O que vale hoje — declarado ou deduzido. Id do catálogo do servidor. */
  effectiveOperatingSystem?: string
  /** `declarado` | `snmp` | `cadastro` | `padrão`. */
  operatingSystemSource?: string
  /**
   * Chave técnica de um dispositivo **do próprio sistema** (`netmonitor`).
   * `null` em todo equipamento comum.
   */
  systemKey?: string | null
  /**
   * Este é o dispositivo que representa esta instalação.
   *
   * Respondido pelo backend, e é a **única** forma legítima de a tela saber
   * disso: deduzir por nome, por posição na lista ou por ID fixo é proibido —
   * o nome é editável e o ID varia por instalação.
   */
  isSystem?: boolean
  site?: { id: number; name: string } | null
  parent?: { id: number; name: string } | null
  network?: { id: number; name: string; cidr: string }
  vpnPeer?: DeviceVpnPeer | null
  linkInterfaceId?: number | null
  linkInterfaceName?: string | null
  createdAt?: string
  updatedAt?: string
  clearHistory?: boolean
}

export interface BandwidthLatencyPoint {
  time: string
  timestamp: number
  bwBps: number
  latency: number
}

export interface BandwidthLatencyResponse {
  timeframe: string
  samples: BandwidthLatencyPoint[]
  currentBw: number
  peakBw: number
  currentLatency: number
  avgLatency: number
  correlationScore: number
  hasSaturationCorrelation: boolean
}

export const useDevicesStore = defineStore('devices', () => {
  const resource = useCrudResource<Device>('/devices', {
    fetch: 'Erro ao carregar dispositivos',
    create: 'Erro ao criar dispositivo',
    update: 'Erro ao atualizar dispositivo',
    delete: 'Erro ao excluir dispositivo',
  })
  const devices = resource.items

  const totalCount = computed(() => devices.value.length)
  const onlineCount = computed(() => devices.value.filter((d) => d.status === 'online').length)
  const offlineCount = computed(() => devices.value.filter((d) => d.status === 'offline').length)
  const warningCount = computed(() => devices.value.filter((d) => d.status === 'warning').length)

  async function fetchDevices() {
    await resource.fetchAll()
  }

  async function createDevice(payload: Partial<Device>): Promise<Device | null> {
    return resource.create(payload)
  }

  async function updateDevice(id: number, payload: Partial<Device>): Promise<Device | null> {
    return resource.update(id, payload)
  }

  async function deleteDevice(id: number): Promise<boolean> {
    return resource.remove(id)
  }

  function updateDeviceStatus(id: number, status: Device['status']) {
    const dev = devices.value.find((d) => d.id === id)
    if (dev) {
      dev.status = status
    }
  }

  /** Aplica o payload SSE `device:status` sem recarregar a lista inteira */
  function applyRealtimeStatus(data: Record<string, unknown>) {
    const id = Number(data.id ?? data.deviceId)
    if (!id) return
    const dev = devices.value.find((d) => d.id === id)
    if (!dev) return

    if (data.status) dev.status = data.status as Device['status']
    if (data.name) dev.name = String(data.name)
    if (data.ipAddress) dev.ipAddress = String(data.ipAddress)
  }

  async function fetchBandwidthLatencySeries(params?: {
    deviceId?: number | 'all' | string
    pingTarget?: number | 'all' | string
    timeframe?: '5m' | '15m' | '1h' | '24h'
  }): Promise<BandwidthLatencyResponse | null> {
    try {
      const q = new URLSearchParams()
      if (params?.deviceId && params.deviceId !== 'all') {
        q.set('deviceId', String(params.deviceId))
      }
      if (params?.pingTarget && params.pingTarget !== 'all') {
        q.set('pingTarget', String(params.pingTarget))
      }
      if (params?.timeframe) {
        q.set('timeframe', params.timeframe)
      }
      const qs = q.toString()
      return await apiService.get<BandwidthLatencyResponse>(
        qs ? `/devices/bandwidth-latency-series?${qs}` : '/devices/bandwidth-latency-series'
      )
    } catch {
      return null
    }
  }

  return {
    devices,
    loading: resource.loading,
    error: resource.error,
    totalCount,
    onlineCount,
    offlineCount,
    warningCount,
    fetchDevices,
    createDevice,
    updateDevice,
    deleteDevice,
    updateDeviceStatus,
    applyRealtimeStatus,
    fetchBandwidthLatencySeries,
  }
})
