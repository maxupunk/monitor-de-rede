import { defineStore } from 'pinia'
import { ref } from 'vue'

export interface Site {
  id: number
  name: string
  description?: string
  location?: string
  active: boolean
}

export const useSitesStore = defineStore('sites', () => {
  const sites = ref<Site[]>([])
  const loading = ref(false)

  async function fetchSites() {
    loading.value = true
    try {
      const res = await fetch('/api/sites')
      if (res.ok) {
        const data = (await res.json()) as Site[]
        sites.value = data
      }
    } finally {
      loading.value = false
    }
  }

  return { sites, loading, fetchSites }
})
