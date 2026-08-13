import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'
import type { SnmpVersion } from '@/utils/monitorTypes'

export interface SnmpTestResult {
  responded: boolean
  version?: SnmpVersion
  community?: string
  sysName?: string
  sysDescr?: string
  sysUpTime?: number
  /** Mensagem do backend — explica a recusa melhor do que um texto genérico */
  message?: string
}

/** `SnmpSystemInfo` do backend (`services/snmp/collectors.rs`) */
interface SnmpSystemPayload {
  sysDescr?: string | null
  sysName?: string | null
  sysUpTime?: number | null
}

/** `SnmpTestResult` do backend, devolvido por `POST /snmp/test` */
interface TestPayload {
  success?: boolean
  system?: SnmpSystemPayload | null
  message?: string
}

/** `SnmpDetectResult` do backend, devolvido quando `autoDetect` é pedido */
interface DetectPayload {
  detected?: boolean
  version?: SnmpVersion | null
  community?: string | null
  result?: TestPayload | null
}

/**
 * O endpoint tem duas respostas — teste direto e detecção automática — e
 * nenhuma delas é plana: o que a tela chama de `responded` é `success` num
 * caso e `detected` no outro, e os dados do agente vivem dentro de `system`.
 * Sem esta tradução, todo teste bem-sucedido aparecia como "não respondeu".
 */
function normalize(payload: TestPayload & DetectPayload): SnmpTestResult {
  const test = payload.result ?? payload
  const system = test.system ?? {}

  return {
    responded: Boolean(payload.detected ?? test.success),
    version: payload.version ?? undefined,
    community: payload.community ?? undefined,
    sysName: system.sysName ?? undefined,
    sysDescr: system.sysDescr ?? undefined,
    sysUpTime: system.sysUpTime ?? undefined,
    message: test.message,
  }
}

export const useSnmpTestStore = defineStore('snmpTest', () => {
  const testing = ref(false)
  const error = ref<string | null>(null)

  async function testConnection(payload: {
    host: string
    port?: number
    version?: SnmpVersion
    community?: string
    autoDetect?: boolean
  }): Promise<SnmpTestResult | null> {
    testing.value = true
    error.value = null
    try {
      return normalize(await apiService.post<TestPayload & DetectPayload>('/snmp/test', payload))
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao testar conexão SNMP'
      return null
    } finally {
      testing.value = false
    }
  }

  return { testing, error, testConnection }
})
