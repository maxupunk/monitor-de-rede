/**
 * Regras puras do assistente de teste de enlace (e do diálogo de
 * agendamento, que escolhe o mesmo alvo): rascunho do alvo, validação, o
 * pedido enviado à API, o que impede rodar e a dica acionável de cada
 * problema do plano.
 */
import type {
  LinkTestEndpoint,
  LinkTestKind,
  LinkTestPlan,
  LinkTestRequest,
  LinkTestSpecInput,
} from '@/types/linkTests'
import { normalizeSpec } from './spec'

export interface LinkTestTargetDraft {
  kind: LinkTestKind
  linkId: number | null
  sourceDeviceId: number | null
  targetDeviceId: number | null
}

export const READ_ONLY_TEST_REASON =
  'Seu perfil é somente leitura: só um operador ou administrador planeja e inicia testes de velocidade.'

function id(value: number | null | undefined): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null
}

/** Rascunho a partir de um pedido (menu da topologia, "repetir", agendamento). */
export function draftFromRequest(
  request?: Partial<
    Pick<LinkTestRequest, 'kind' | 'linkId' | 'sourceDeviceId' | 'targetDeviceId'>
  > | null
): LinkTestTargetDraft {
  return {
    kind: request?.kind ?? 'link',
    linkId: id(request?.linkId),
    sourceDeviceId: id(request?.sourceDeviceId),
    targetDeviceId: id(request?.targetDeviceId),
  }
}

/** O que falta escolher no passo "O que testar". Vazio = pode seguir. */
export function targetErrors(draft: LinkTestTargetDraft): string[] {
  if (draft.kind === 'link') {
    if (draft.linkId === null) return ['Escolha o enlace a medir.']
    return []
  }
  if (draft.kind === 'path') {
    const errors: string[] = []
    if (draft.sourceDeviceId === null) errors.push('Escolha a origem do caminho.')
    if (draft.targetDeviceId === null) errors.push('Escolha o destino do caminho.')
    if (draft.sourceDeviceId !== null && draft.sourceDeviceId === draft.targetDeviceId) {
      errors.push('Origem e destino precisam ser equipamentos diferentes.')
    }
    return errors
  }
  return draft.targetDeviceId === null ? ['Escolha até qual equipamento medir.'] : []
}

/** O pedido enviado à API: só os campos que o tipo usa. */
export function requestFromDraft(
  draft: LinkTestTargetDraft,
  spec?: LinkTestSpecInput,
  confirm = false
): LinkTestRequest {
  const request: LinkTestRequest = { kind: draft.kind, spec: normalizeSpec(spec) }
  if (draft.kind === 'link') {
    if (draft.linkId !== null) request.linkId = draft.linkId
    if (draft.sourceDeviceId !== null) request.sourceDeviceId = draft.sourceDeviceId
    if (draft.targetDeviceId !== null) request.targetDeviceId = draft.targetDeviceId
  } else {
    if (draft.kind === 'path' && draft.sourceDeviceId !== null) {
      request.sourceDeviceId = draft.sourceDeviceId
    }
    if (draft.targetDeviceId !== null) request.targetDeviceId = draft.targetDeviceId
  }
  if (confirm) request.confirm = true
  return request
}

/**
 * Por que o plano não pode rodar (vazio = pode). Perfil somente leitura,
 * bloqueios do backend e plano sem nenhuma perna executável.
 */
export function runBlockers(plan: LinkTestPlan | null, canWrite: boolean): string[] {
  if (!plan) return ['Calcule o plano antes de iniciar.']
  const reasons: string[] = []
  if (!canWrite) reasons.push(READ_ONLY_TEST_REASON)
  reasons.push(...plan.blocking)
  if (plan.blocking.length === 0 && !plan.legs.some((leg) => leg.runnable)) {
    reasons.push('Nenhuma perna do plano pode rodar — veja os problemas de cada uma abaixo.')
  }
  return reasons
}

export interface IssueHint {
  text: string
  /** Rota que resolve o problema (ficha do equipamento na seção certa). */
  to: string | null
  actionLabel: string | null
}

const CREDENTIAL_PATTERN = /credencia|ssh|senha|login|usu[aá]rio|autentica/i
const PLUGIN_PATTERN = /plugin|instal|pacote|ativ/i

/**
 * Dica acionável para um problema do plano: se o texto cita um equipamento
 * do plano e fala de credencial ou plugin, aponta para a seção da ficha dele.
 */
export function issueHint(
  text: string,
  endpoints: ReadonlyArray<Pick<LinkTestEndpoint, 'deviceId' | 'name'>>
): IssueHint {
  const lowered = text.toLowerCase()
  const device = endpoints
    .filter((endpoint) => endpoint.deviceId !== null && endpoint.name)
    .sort((a, b) => b.name.length - a.name.length)
    .find((endpoint) => lowered.includes(endpoint.name.toLowerCase()))
  const deviceId = device?.deviceId ?? (endpoints.length === 1 ? endpoints[0].deviceId : null)
  if (deviceId === null || deviceId === undefined) return { text, to: null, actionLabel: null }
  if (CREDENTIAL_PATTERN.test(text)) {
    return {
      text,
      to: `/devices/${deviceId}?tab=plugins&section=credentials`,
      actionLabel: 'Cadastrar credencial',
    }
  }
  if (PLUGIN_PATTERN.test(text)) {
    return { text, to: `/devices/${deviceId}?tab=plugins`, actionLabel: 'Abrir plugins' }
  }
  return { text, to: `/devices/${deviceId}`, actionLabel: 'Abrir equipamento' }
}

/** Todas as pontas do plano (para achar o equipamento citado num problema). */
export function planEndpoints(plan: Pick<LinkTestPlan, 'legs'>): LinkTestEndpoint[] {
  return plan.legs.flatMap((leg) => [leg.a, leg.b])
}

/**
 * Subtítulo do enlace na lista do assistente: as portas (quando o mapa as
 * conhece) e a velocidade. Enlace manual, sem nenhum dos dois, diz isso em vez
 * de "? ↔ ?".
 */
export function linkOptionSubtitle(
  sourcePort: string | null | undefined,
  targetPort: string | null | undefined,
  speed: string | null
): string {
  const ports = sourcePort || targetPort ? `${sourcePort || '?'} ↔ ${targetPort || '?'}` : null
  const parts = [ports, speed].filter((part): part is string => Boolean(part))
  return parts.length > 0 ? parts.join(' · ') : 'Portas não identificadas'
}
