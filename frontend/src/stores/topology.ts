import { defineStore } from 'pinia'
import { ref } from 'vue'

export const useTopologyStore = defineStore('topology', () => {
  const nodes = ref<Array<Record<string, unknown>>>([])
  const edges = ref<Array<Record<string, unknown>>>([])
  const loading = ref(false)

  async function fetchTopology() {
    loading.value = true
    try {
      const res = await fetch('/api/topology')
      if (res.ok) {
        const data = await res.json()
        nodes.value = data.nodes || []
        edges.value = data.edges || []
      }
    } finally {
      loading.value = false
    }
  }

  return { nodes, edges, loading, fetchTopology }
})
