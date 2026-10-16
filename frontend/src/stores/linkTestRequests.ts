import { defineStore } from 'pinia'
import { ref } from 'vue'

/**
 * Ponto único por onde a topologia pede um teste de enlace (ADR 015). O mapa
 * só registra o pedido; quem abre o assistente (plano → confirmação →
 * execução) observa `pending` e chama `take()` ao assumir o pedido.
 */

export type LinkTestKind = 'link' | 'path' | 'server'

export interface LinkTestRequest {
  kind: LinkTestKind
  /** Enlace a medir (`kind: 'link'`). */
  linkId?: number
  sourceDeviceId?: number
  targetDeviceId?: number
}

export interface RecordedLinkTestRequest extends LinkTestRequest {
  /** Sequencial da sessão — distingue dois pedidos iguais seguidos. */
  seq: number
  requestedAt: string
}

export const useLinkTestRequestsStore = defineStore('linkTestRequests', () => {
  const pending = ref<RecordedLinkTestRequest | null>(null)
  const history = ref<RecordedLinkTestRequest[]>([])
  let seq = 0

  function requestLinkTest(request: LinkTestRequest): RecordedLinkTestRequest {
    seq += 1
    const recorded: RecordedLinkTestRequest = {
      ...request,
      seq,
      requestedAt: new Date().toISOString(),
    }
    pending.value = recorded
    history.value = [recorded, ...history.value].slice(0, 20)
    return recorded
  }

  /** O assistente assume o pedido pendente (e ele deixa de estar pendente). */
  function take(): RecordedLinkTestRequest | null {
    const current = pending.value
    pending.value = null
    return current
  }

  return { pending, history, requestLinkTest, take }
})
