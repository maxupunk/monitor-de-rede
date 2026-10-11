import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { apiService } from '@/services/apiService'
import type { AiPendingApproval } from '@/bindings/AiPendingApproval'
import type { ExecuteToolResponse } from '@/bindings/ExecuteToolResponse'
import { isAiToolRisk } from '@/utils/aiRisk'
import { useAuthStore } from './auth'

export type { AiPendingApproval }

/** Como o usuário decidiu um pedido (evento `ai:approval_resolved`). */
export type AiApprovalDecision = 'confirmed' | 'dismissed'

export interface AiApprovalResolved {
  callId: string
  decision: AiApprovalDecision
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}

export function isPendingApproval(value: unknown): value is AiPendingApproval {
  return (
    isRecord(value) &&
    typeof value.callId === 'string' &&
    typeof value.userId === 'number' &&
    typeof value.tool === 'string' &&
    typeof value.summary === 'string' &&
    isAiToolRisk(value.risk) &&
    typeof value.reason === 'string'
  )
}

export function asApprovalResolved(value: unknown): AiApprovalResolved | null {
  if (!isRecord(value) || typeof value.callId !== 'string') return null
  const decision = value.decision === 'dismissed' ? 'dismissed' : 'confirmed'
  return { callId: value.callId, decision }
}

/**
 * Pedidos de aprovação da IA esperando o usuário. Chegam pelo SSE global
 * (snapshot ao conectar), então aparecem em qualquer tela — inclusive numa
 * aba aberta pela notificação no celular. Nada de polling (AGENTS §9).
 */
export const useAiApprovalsStore = defineStore('aiApprovals', () => {
  const auth = useAuthStore()
  const pending = ref<AiPendingApproval[]>([])

  /** Só os do usuário logado: o SSE é o mesmo para todos. */
  const mine = computed(() => pending.value.filter((item) => item.userId === auth.user?.id))

  function remove(callId: string) {
    pending.value = pending.value.filter((item) => item.callId !== callId)
  }

  function applyPending(data: unknown) {
    if (!isPendingApproval(data)) return
    if (pending.value.some((item) => item.callId === data.callId)) return
    pending.value = [...pending.value, data]
  }

  function applyResolved(resolved: AiApprovalResolved) {
    remove(resolved.callId)
  }

  /** Executa a proposta guardada no servidor (confirmação fora do chat). */
  async function confirm(callId: string): Promise<ExecuteToolResponse> {
    try {
      return await apiService.post<ExecuteToolResponse>(
        `/ai/approvals/${encodeURIComponent(callId)}/confirm`,
        {},
        // Acesso a equipamento pode esperar aprovações no meio da execução.
        { timeoutMs: 15 * 60_000 }
      )
    } finally {
      // Executado, expirado ou já decidido em outra tela: sai da fila.
      remove(callId)
    }
  }

  /** O usuário cancelou: o pedido sai de todas as telas. */
  async function dismiss(callId: string) {
    remove(callId)
    await apiService.post(`/ai/approvals/${encodeURIComponent(callId)}/dismiss`, {})
  }

  return { pending, mine, applyPending, applyResolved, confirm, dismiss }
})
