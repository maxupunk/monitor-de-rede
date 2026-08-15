import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'
import { drainNdjson } from '@/services/ndjson'

export type PortProtocol = 'tcp' | 'udp'
export type PortScanProfile = 'fast' | 'reliable' | 'complete'
export type PortStatus = 'open' | 'closed' | 'open|filtered' | 'filtered' | 'unreachable' | 'error'

export interface PortScanItem {
  port: number
  protocol: PortProtocol
  status: PortStatus
  service?: string
  latencyMs: number
  attempts: number
  error?: string
}

export const usePortScanStore = defineStore('portScan', () => {
  const scanning = ref(false)
  const error = ref<string | null>(null)
  let activeController: AbortController | null = null

  /**
   * Inicia a varredura e transmite cada porta verificada via `onResult` assim que o backend
   * a resolve, em vez de esperar a varredura inteira terminar.
   * Retorna `true` se o stream terminou normalmente, `false` em caso de erro ou cancelamento.
   */
  async function scanPorts(
    payload: {
      host: string
      protocol: PortProtocol
      ports: number[]
      timeoutMs?: number
      profile?: PortScanProfile
    },
    onResult: (item: PortScanItem) => void
  ): Promise<boolean> {
    scanning.value = true
    error.value = null
    const controller = new AbortController()
    activeController = controller

    try {
      const response = await apiService.postStream('/port-scan', payload, controller.signal)
      const reader = response.body?.getReader()
      if (!reader) return false

      const decoder = new TextDecoder()
      let buffer = ''
      let completed = false

      while (true) {
        const { done, value } = await reader.read()
        if (done) break
        buffer += decoder.decode(value, { stream: true })

        const parsedChunk = drainNdjson<{ type?: string; message?: string } & PortScanItem>(buffer)
        buffer = parsedChunk.remainder
        for (const parsed of parsedChunk.events) {
          if (parsed.type === 'result') {
            delete parsed.type
            onResult(parsed as PortScanItem)
          } else if (parsed.type === 'error') {
            error.value = parsed.message || 'Erro durante a varredura de portas'
          } else if (parsed.type === 'done') {
            completed = true
          }
        }
      }

      const finalChunk = drainNdjson<{ type?: string; message?: string } & PortScanItem>(buffer, {
        final: true,
      })
      for (const parsed of finalChunk.events) {
        if (parsed.type === 'result') {
          delete parsed.type
          onResult(parsed as PortScanItem)
        } else if (parsed.type === 'error') {
          error.value = parsed.message || 'Erro durante a varredura de portas'
        } else if (parsed.type === 'done') {
          completed = true
        }
      }

      return completed && !error.value
    } catch (err: unknown) {
      if (err instanceof DOMException && err.name === 'AbortError') {
        return false
      }
      error.value = err instanceof Error ? err.message : 'Erro ao executar varredura de portas'
      return false
    } finally {
      scanning.value = false
      activeController = null
    }
  }

  function cancelScan() {
    activeController?.abort()
  }

  return { scanning, error, scanPorts, cancelScan }
})
