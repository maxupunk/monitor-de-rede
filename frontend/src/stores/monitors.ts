import { defineStore } from 'pinia'
import { ref } from 'vue'

export interface Monitor {
  id: number
  deviceId: number
  type: string
  name: string
  enabled: boolean
  status: string
}

export const useMonitorsStore = defineStore('monitors', () => {
  const monitors = ref<Monitor[]>([])
  const loading = ref(false)

  async function fetchMonitors() {
    loading.value = true
    try {
      const res = await fetch('/api/monitors')
      if (res.ok) {
        const data = (await res.json()) as Monitor[]
        monitors.value = data
      }
    } finally {
      loading.value = false
    }
  }

  return { monitors, loading, fetchMonitors }
})
