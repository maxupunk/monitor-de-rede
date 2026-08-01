import { defineStore } from 'pinia'
import { ref } from 'vue'

export interface Network {
  id: number
  siteId: number
  name: string
  cidr: string
  active: boolean
}

export const useNetworksStore = defineStore('networks', () => {
  const networks = ref<Network[]>([])
  const loading = ref(false)

  async function fetchNetworks() {
    loading.value = true
    try {
      const res = await fetch('/api/networks')
      if (res.ok) {
        const data = (await res.json()) as Network[]
        networks.value = data
      }
    } finally {
      loading.value = false
    }
  }

  return { networks, loading, fetchNetworks }
})
