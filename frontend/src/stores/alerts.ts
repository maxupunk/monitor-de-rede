import { defineStore } from 'pinia'
import { ref } from 'vue'

export const useAlertsStore = defineStore('alerts', () => {
  const alerts = ref<Array<Record<string, unknown>>>([])
  const loading = ref(false)

  async function fetchAlerts() {
    loading.value = true
    try {
      const res = await fetch('/api/alerts')
      if (res.ok) {
        const data = (await res.json()) as Array<Record<string, unknown>>
        alerts.value = data
      }
    } finally {
      loading.value = false
    }
  }

  return { alerts, loading, fetchAlerts }
})
