import { defineStore } from 'pinia'
import { ref } from 'vue'

export const useProbesStore = defineStore('probes', () => {
  const probes = ref<Array<Record<string, unknown>>>([])
  const loading = ref(false)

  async function fetchProbes() {
    loading.value = true
    try {
      const res = await fetch('/api/probes')
      if (res.ok) {
        const data = (await res.json()) as Array<Record<string, unknown>>
        probes.value = data
      }
    } finally {
      loading.value = false
    }
  }

  return { probes, loading, fetchProbes }
})
