import { defineStore } from 'pinia'
import { ref } from 'vue'

export const useDiscoveryStore = defineStore('discovery', () => {
  const results = ref<Array<Record<string, unknown>>>([])
  const loading = ref(false)

  async function fetchResults() {
    loading.value = true
    try {
      const res = await fetch('/api/discovery/results')
      if (res.ok) {
        const data = (await res.json()) as Array<Record<string, unknown>>
        results.value = data
      }
    } finally {
      loading.value = false
    }
  }

  return { results, loading, fetchResults }
})
