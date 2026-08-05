import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'

export type PortProtocol = 'tcp' | 'udp'
export type PortStatus = 'open' | 'closed' | 'open|filtered'

export interface PortScanItem {
  port: number
  protocol: PortProtocol
  status: PortStatus
  service?: string
  latencyMs: number
}

export interface PortScanResponse {
  host: string
  protocol: PortProtocol
  results: PortScanItem[]
}

export const usePortScanStore = defineStore('portScan', () => {
  const scanning = ref(false)
  const error = ref<string | null>(null)

  async function scanPorts(payload: {
    host: string
    protocol: PortProtocol
    ports: number[]
    timeoutMs?: number
  }): Promise<PortScanItem[] | null> {
    scanning.value = true
    error.value = null
    try {
      const res = await apiService.post<PortScanResponse>('/port-scan', payload)
      return res.results
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao executar varredura de portas'
      return null
    } finally {
      scanning.value = false
    }
  }

  return { scanning, error, scanPorts }
})
