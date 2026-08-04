import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { apiService } from '@/services/apiService'
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
  site?: { id: number; name: string } | null
  parent?: { id: number; name: string } | null
  network?: { id: number; name: string; cidr: string }
  vpnPeer?: DeviceVpnPeer | null
  createdAt?: string
  updatedAt?: string
}

export const useDevicesStore = defineStore('devices', () => {
  const devices = ref<Device[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)

  const totalCount = computed(() => devices.value.length)
  const onlineCount = computed(() => devices.value.filter((d) => d.status === 'online').length)
  const offlineCount = computed(() => devices.value.filter((d) => d.status === 'offline').length)
  const warningCount = computed(() => devices.value.filter((d) => d.status === 'warning').length)

  async function fetchDevices() {
    loading.value = true
    error.value = null
    try {
      devices.value = await apiService.get<Device[]>('/devices')
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao carregar dispositivos'
    } finally {
      loading.value = false
    }
  }

  async function createDevice(payload: Partial<Device>): Promise<boolean> {
    try {
      const created = await apiService.post<Device>('/devices', payload)
      devices.value.push(created)
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao criar dispositivo'
      return false
    }
  }

  async function updateDevice(id: number, payload: Partial<Device>): Promise<boolean> {
    try {
      const updated = await apiService.put<Device>(`/devices/${id}`, payload)
      const index = devices.value.findIndex((d) => d.id === id)
      if (index !== -1) {
        devices.value[index] = updated
      }
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao atualizar dispositivo'
      return false
    }
  }

  async function deleteDevice(id: number): Promise<boolean> {
    try {
      await apiService.delete(`/devices/${id}`)
      devices.value = devices.value.filter((d) => d.id !== id)
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao excluir dispositivo'
      return false
    }
  }

  function updateDeviceStatus(id: number, status: Device['status']) {
    const dev = devices.value.find((d) => d.id === id)
    if (dev) {
      dev.status = status
    }
  }

  return {
    devices,
    loading,
    error,
    totalCount,
    onlineCount,
    offlineCount,
    warningCount,
    fetchDevices,
    createDevice,
    updateDevice,
    deleteDevice,
    updateDeviceStatus,
  }
})
