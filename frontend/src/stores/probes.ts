import { defineStore } from 'pinia'
import { ref } from 'vue'

export const useProbesStore = defineStore('probes', () => {
  const probes = ref<Array<Record<string, unknown>>>([])
  const loading = ref(false)

  async function fetchProbes() {
    loading.value = true
    try {
      const res = await fetch('/api/probes')
      if (res.ok) probes.value = await res.json()
    } finally {
      loading.value = false
    }
  }

  return { probes, loading, fetchProbes }
})
