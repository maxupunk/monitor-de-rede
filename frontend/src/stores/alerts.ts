import { defineStore } from 'pinia'
import { ref } from 'vue'

export const useAlertsStore = defineStore('alerts', () => {
  const alerts = ref<Array<Record<string, unknown>>>([])
  const loading = ref(false)

  async function fetchAlerts() {
    loading.value = true
    try {
      const res = await fetch('/api/alerts')
      if (res.ok) alerts.value = await res.json()
    } finally {
      loading.value = false
    }
  }

  return { alerts, loading, fetchAlerts }
})
