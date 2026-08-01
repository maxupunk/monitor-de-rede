import { defineStore } from 'pinia'
import { ref } from 'vue'

export interface Device {
  id: number
  siteId: number
  networkId?: number
  name: string
  type: string
  vendor?: string
  model?: string
  status: 'online' | 'offline' | 'warning' | 'unknown'
}

export const useDevicesStore = defineStore('devices', () => {
  const devices = ref<Device[]>([])
  const loading = ref(false)

  async function fetchDevices() {
    loading.value = true
    try {
      const res = await fetch('/api/devices')
      if (res.ok) devices.value = await res.json()
    } finally {
      loading.value = false
    }
  }

  return { devices, loading, fetchDevices }
})
