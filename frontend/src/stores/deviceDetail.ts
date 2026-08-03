import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'
import type { Device } from './devices'

export interface DeviceInterface {
  id: number
  deviceId: number
  snmpIndex?: number
  ifIndex?: number
  name?: string
  ifName?: string
  ifType?: string
  adminStatus?: string
  ifAdminStatus?: 'up' | 'down' | 'testing'
  operStatus?: string
  ifOperStatus?: 'up' | 'down' | 'testing'
  speed?: number
  ifSpeed?: number
  macAddress?: string
  ipAddress?: string
  inOctets?: number
  outOctets?: number
  inBps?: number
  outBps?: number
}

export interface DeviceMetric {
  id: number
  deviceId: number
  interfaceId?: number | null
  interfaceName?: string | null
  metricName: string
  metricValue: number
  unit?: string
  createdAt: string
}

export interface DeviceMonitor {
  id: number
  deviceId: number
  type: string
  name: string
  target: string
  port?: number
  intervalSeconds?: number
  enabled?: boolean
  status: 'online' | 'offline' | 'up' | 'down' | 'warning' | 'disabled' | 'unknown'
  lastCheckedAt?: string
  latencyMs?: number
}

export interface DeviceEvent {
  id: number
  deviceId: number
  eventType: string
  severity: 'info' | 'warning' | 'error' | 'critical'
  message: string
  createdAt: string
}

export interface ScanInterfaceItem {
  ifIndex: number
  ifName: string
  ifDescr?: string
  macAddress?: string
  ifSpeed?: number
  ifAdminStatus?: string
  ifOperStatus?: string
  isMonitored: boolean
}

export interface ScanResult {
  systemInfo: {
    sysName?: string
    sysDescr?: string
    sysUpTime?: number
  }
  cpuInfo: {
    usagePercent?: number
    userPercent?: number
    systemPercent?: number
    idlePercent?: number
    load1min?: number
    coresCount?: number
  }
  memoryInfo: {
    totalKb?: number
    usedKb?: number
    usedPercent?: number
  }
  interfaces: ScanInterfaceItem[]
  hasCpuMonitor: boolean
  hasMemoryMonitor: boolean
}

export const useDeviceDetailStore = defineStore('deviceDetail', () => {
  const device = ref<Device | null>(null)
  const interfaces = ref<DeviceInterface[]>([])
  const metrics = ref<DeviceMetric[]>([])
  const monitors = ref<DeviceMonitor[]>([])
  const events = ref<DeviceEvent[]>([])
  const loading = ref(false)
  const pollingSnmp = ref(false)
  const scanningSnmp = ref(false)
  const scanResult = ref<ScanResult | null>(null)
  const error = ref<string | null>(null)

  async function loadDeviceDetails(deviceId: number) {
    loading.value = true
    error.value = null
    try {
      const [devData, intfData, monData, metData, evtData] = await Promise.allSettled([
        apiService.get<Device>(`/devices/${deviceId}`),
        apiService.get<DeviceInterface[]>(`/devices/${deviceId}/interfaces`),
        apiService.get<DeviceMonitor[]>(`/devices/${deviceId}/monitors`),
        apiService.get<DeviceMetric[]>(`/devices/${deviceId}/metrics`),
        apiService.get<DeviceEvent[]>(`/devices/${deviceId}/events`),
      ])

      if (devData.status === 'fulfilled') device.value = devData.value
      if (intfData.status === 'fulfilled') {
        interfaces.value = Array.isArray(intfData.value) ? intfData.value : []
      }
      if (monData.status === 'fulfilled') {
        monitors.value = Array.isArray(monData.value) ? monData.value : []
      }
      if (metData.status === 'fulfilled') {
        metrics.value = Array.isArray(metData.value) ? metData.value : []
      }
      if (evtData.status === 'fulfilled') {
        events.value = Array.isArray(evtData.value) ? evtData.value : []
      }
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao carregar detalhes do dispositivo'
    } finally {
      loading.value = false
    }
  }

  async function triggerSnmpPoll(deviceId: number): Promise<boolean> {
    pollingSnmp.value = true
    try {
      await apiService.post(`/devices/${deviceId}/snmp/poll`)
      await loadDeviceDetails(deviceId)
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao executar poll SNMP'
      return false
    } finally {
      pollingSnmp.value = false
    }
  }

  async function scanDeviceSnmp(deviceId: number): Promise<ScanResult | null> {
    scanningSnmp.value = true
    error.value = null
    try {
      const res = await apiService.post<ScanResult>(`/devices/${deviceId}/snmp/scan`)
      scanResult.value = res
      return res
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao escanear dispositivo via SNMP'
      return null
    } finally {
      scanningSnmp.value = false
    }
  }

  async function applySnmpMonitors(
    deviceId: number,
    options: {
      enableCpuMonitor?: boolean
      enableMemoryMonitor?: boolean
      monitoredIfIndexes?: number[]
    }
  ): Promise<boolean> {
    loading.value = true
    try {
      await apiService.post(`/devices/${deviceId}/snmp/apply-monitors`, options)
      await loadDeviceDetails(deviceId)
      return true
    } catch (err: unknown) {
      error.value =
        err instanceof Error ? err.message : 'Erro ao aplicar configurações de monitoramento'
      return false
    } finally {
      loading.value = false
    }
  }

  return {
    device,
    interfaces,
    metrics,
    monitors,
    events,
    loading,
    pollingSnmp,
    scanningSnmp,
    scanResult,
    error,
    loadDeviceDetails,
    triggerSnmpPoll,
    scanDeviceSnmp,
    applySnmpMonitors,
  }
})
