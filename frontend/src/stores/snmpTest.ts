import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'

export interface SnmpTestResult {
  responded: boolean
  version?: 'v1' | 'v2c'
  community?: string
  sysName?: string
  sysDescr?: string
  sysUpTime?: number
}

export const useSnmpTestStore = defineStore('snmpTest', () => {
  const testing = ref(false)
  const error = ref<string | null>(null)

  async function testConnection(payload: {
    host: string
    port?: number
    version?: 'v1' | 'v2c' | 'v3'
    community?: string
    autoDetect?: boolean
  }): Promise<SnmpTestResult | null> {
    testing.value = true
    error.value = null
    try {
      return await apiService.post<SnmpTestResult>('/snmp/test', payload)
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao testar conexão SNMP'
      return null
    } finally {
      testing.value = false
    }
  }

  return { testing, error, testConnection }
})
