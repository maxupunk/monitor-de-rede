/**
 * Regras puras do assistente de teste de enlace (e do diálogo de
 * agendamento, que escolhe o mesmo alvo): rascunho do alvo, validação, o
 * pedido enviado à API, o que impede rodar, os passos que faltam e a dica
 * de cada problema do plano.
 */
import type {
  LinkTestEndpoint,
  LinkTestFix,
  LinkTestKind,
  LinkTestPlan,
  LinkTestPlanLeg,
  LinkTestRequest,
  LinkTestSpecInput,
} from '@/types/linkTests'
import { transportLabel } from '@/utils/deviceCredentials'
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
    reasons.push(
      plan.fixes.length > 0
        ? 'Faça o que falta (no topo do plano) para liberar o teste.'
        : 'Nenhuma perna do plano pode rodar — veja os problemas de cada uma abaixo.'
    )
  }
  return reasons
}

/** Identidade de um passo, estável entre um plano e o seguinte. */
export function fixKey(fix: LinkTestFix): string {
  return fix.kind === 'install_plugin'
    ? `install:${fix.deviceId}:${fix.pluginId}`
    : `access:${fix.deviceId}:${fix.transport}`
}

/** O passo, como item da lista "Antes de testar, falta". */
export function fixLabel(fix: LinkTestFix): string {
  return fix.kind === 'install_plugin'
    ? `Instalar o plugin «${fix.pluginName}» em ${fix.deviceName}`
    : `Cadastrar o acesso ${transportLabel(fix.transport)} de ${fix.deviceName}`
}

/** Os problemas da perna que nenhum passo resolve ali mesmo. */
export function legOpenIssues(leg: Pick<LinkTestPlanLeg, 'issues' | 'fixes'>): string[] {
  return leg.issues.filter((issue) => !leg.fixes.some((fix) => fix.message === issue))
}

/** A perna só espera um passo (instalar, cadastrar o acesso) — não é erro. */
export function legNeedsSteps(leg: LinkTestPlanLeg): boolean {
  return !leg.runnable && leg.fixes.length > 0 && legOpenIssues(leg).length === 0
}

export interface IssueHint {
  text: string
  /** A ficha do equipamento citado no problema. */
  to: string | null
  actionLabel: string | null
}

/**
 * Dica para um problema do plano que não tem passo ali mesmo (sem IP, por
 * exemplo): se o texto cita um equipamento do plano, aponta para a ficha
 * dele. Plugin e acesso que faltam não chegam aqui — viram `fixes`.
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
