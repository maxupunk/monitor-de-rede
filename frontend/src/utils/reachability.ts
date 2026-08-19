/**
 * **Monitor de alcance** — a mesma definição que o backend usa.
 *
 * O domínio mora em `backend/src/services/monitoring/reachability.rs`: é lá
 * que a regra é aplicada, e nenhuma tela pode contradizê-la. O que existe aqui
 * é só o vocabulário de apresentação — o rótulo que a interface usa para
 * explicar *por que* um monitor não vai nascer.
 *
 * Quando o dispositivo já existe, o motivo autoritativo vem do backend em
 * `capabilities.reachMonitorBlockedReason`, e é ele que a tela mostra. O texto
 * abaixo cobre o único caso em que não há dispositivo para consultar: o
 * formulário de cadastro, antes do primeiro salvamento.
 */

/** Tipos que medem "este equipamento responde pela rede?". */
export const REACH_MONITOR_TYPES = ['ping', 'tcp', 'http', 'https', 'dns'] as const

export type ReachMonitorType = (typeof REACH_MONITOR_TYPES)[number]

/** Verdadeiro para um tipo que mede alcance pela rede. */
export function isReachMonitorType(type: string | null | undefined): boolean {
  if (!type) return false
  return (REACH_MONITOR_TYPES as readonly string[]).includes(type.trim().toLowerCase())
}

/**
 * Por que um cadastro sem endereço não gera monitor automático.
 *
 * Usado só no formulário de criação — ver a nota do módulo.
 */
export const SEM_ALVO_DE_ALCANCE =
  'Sem endereço IP não há alvo para checar: o monitor de alcance só é criado depois que o endereço for informado.'
