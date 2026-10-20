/**
 * Apresentação do teste de enlace (ADR 015) — fonte única de rótulos, ícones
 * e cores de estado, tipo, etapa e motor. Assistente, painel ao vivo, aba da
 * topologia e página de histórico leem daqui (DRY). Cores são tokens
 * semânticos do tema, nunca cinza.
 */
import { formatBps } from '@/utils/formatters'
import type { StatusPresentation, ThemeColorToken } from '@/utils/topology/presentation'
import type {
  LinkTest,
  LinkTestDirection,
  LinkTestEndpoint,
  LinkTestEngine,
  LinkTestKind,
  LinkTestLeg,
  LinkTestPlanLeg,
  LinkTestStageKey,
  LinkTestStatus,
  LinkTestStepStatus,
} from '@/types/linkTests'

export type { StatusPresentation }

export const TEST_STATUS: Record<LinkTestStatus, StatusPresentation> = {
  queued: { label: 'Na fila', color: 'info', icon: 'mdi-timer-sand' },
  running: { label: 'Em andamento', color: 'primary', icon: 'mdi-speedometer' },
  succeeded: { label: 'Concluído', color: 'success', icon: 'mdi-check-circle' },
  partial: { label: 'Parcial', color: 'warning', icon: 'mdi-alert-circle' },
  failed: { label: 'Falhou', color: 'error', icon: 'mdi-close-circle' },
  cancelled: { label: 'Cancelado', color: 'secondary', icon: 'mdi-cancel' },
}

export const STEP_STATUS: Record<LinkTestStepStatus, StatusPresentation> = {
  pending: { label: 'Aguardando', color: 'info', icon: 'mdi-circle-outline' },
  running: { label: 'Em execução', color: 'primary', icon: 'mdi-progress-clock' },
  succeeded: { label: 'Concluída', color: 'success', icon: 'mdi-check-circle' },
  failed: { label: 'Falhou', color: 'error', icon: 'mdi-close-circle' },
  skipped: { label: 'Pulada', color: 'warning', icon: 'mdi-debug-step-over' },
  cancelled: { label: 'Cancelada', color: 'secondary', icon: 'mdi-cancel' },
}

export function testStatusPresentation(status: string): StatusPresentation {
  return TEST_STATUS[status as LinkTestStatus] ?? TEST_STATUS.queued
}

export function stepStatusPresentation(status: string): StatusPresentation {
  return STEP_STATUS[status as LinkTestStepStatus] ?? STEP_STATUS.pending
}

export interface KindPresentation {
  label: string
  icon: string
  color: ThemeColorToken
  description: string
}

export const TEST_KINDS: Record<LinkTestKind, KindPresentation> = {
  link: {
    label: 'Enlace',
    icon: 'mdi-swap-horizontal-bold',
    color: 'primary',
    description: 'Mede só o enlace entre dois equipamentos vizinhos.',
  },
  end_to_end: {
    label: 'Ponta a ponta',
    icon: 'mdi-arrow-expand-horizontal',
    color: 'info',
    description:
      'Uma medição de um equipamento até o outro, atravessando o caminho inteiro — a velocidade que quem usa a rede sente.',
  },
  path: {
    label: 'Enlace por enlace',
    icon: 'mdi-transit-connection-variant',
    color: 'secondary',
    description:
      'Mede, um de cada vez e em ordem, cada enlace do caminho — mostra onde está o gargalo.',
  },
  server: {
    label: 'Do servidor',
    icon: 'mdi-server-network',
    color: 'info',
    description: 'Ponta a ponta do servidor NetMonitor até o equipamento.',
  },
}

export const KIND_OPTIONS: ReadonlyArray<{ value: LinkTestKind } & KindPresentation> = (
  Object.keys(TEST_KINDS) as LinkTestKind[]
).map((value) => ({ value, ...TEST_KINDS[value] }))

export const STAGES: Record<LinkTestStageKey, { label: string; icon: string }> = {
  prepare_a: { label: 'Conferir A', icon: 'mdi-clipboard-check-outline' },
  prepare_b: { label: 'Conferir B', icon: 'mdi-clipboard-check-outline' },
  serve: { label: 'Subir servidor', icon: 'mdi-server' },
  forward: { label: 'Ida (A → B)', icon: 'mdi-arrow-right-bold' },
  reverse: { label: 'Volta (B → A)', icon: 'mdi-arrow-left-bold' },
  stop: { label: 'Encerrar', icon: 'mdi-stop-circle-outline' },
}

export const ENGINES: Record<LinkTestEngine, { label: string; color: ThemeColorToken }> = {
  iperf3: { label: 'iperf3', color: 'primary' },
  btest: { label: 'MikroTik btest', color: 'secondary' },
}

export function engineLabel(engine: LinkTestEngine | null): string {
  return engine ? ENGINES[engine].label : 'Sem motor'
}

export const DIRECTION_OPTIONS: ReadonlyArray<{
  value: LinkTestDirection
  title: string
  icon: string
}> = [
  { value: 'both', title: 'Ambos', icon: 'mdi-swap-horizontal' },
  { value: 'forward', title: 'Só ida', icon: 'mdi-arrow-right' },
  { value: 'reverse', title: 'Só volta', icon: 'mdi-arrow-left' },
]

export function directionLabel(direction: LinkTestDirection): string {
  if (direction === 'forward') return 'só ida (A → B)'
  if (direction === 'reverse') return 'só volta (B → A)'
  return 'ida e volta'
}

export function isActiveStatus(status: LinkTestStatus): boolean {
  return status === 'queued' || status === 'running'
}

export function endpointLabel(endpoint: Pick<LinkTestEndpoint, 'kind' | 'name'>): string {
  if (endpoint.name) return endpoint.name
  if (endpoint.kind === 'central') return 'Servidor NetMonitor'
  if (endpoint.kind === 'agent') return 'Agente remoto'
  return 'Equipamento'
}

export function endpointIcon(endpoint: Pick<LinkTestEndpoint, 'kind'>): string {
  if (endpoint.kind === 'central') return 'mdi-monitor-dashboard'
  if (endpoint.kind === 'agent') return 'mdi-robot-outline'
  return 'mdi-router-network'
}

/**
 * As arestas da perna, em ordem de A a B (várias quando ela atravessa
 * equipamentos sem IP). Perna de um backend anterior só traz o `edgeId`.
 */
export function legEdgeIds(leg: Pick<LinkTestPlanLeg, 'edgeId' | 'edgeIds'>): number[] {
  if (leg.edgeIds && leg.edgeIds.length > 0) return leg.edgeIds
  return leg.edgeId !== null ? [leg.edgeId] : []
}

/** "switch-a › switch-b": os equipamentos sem IP atravessados, em ordem. */
export function viaText(names: ReadonlyArray<string> | null | undefined): string | null {
  return names && names.length > 0 ? names.join(' › ') : null
}

/** "via switch-a › switch-b" — a perna atravessa equipamentos sem IP. */
export function legViaLabel(leg: Pick<LinkTestPlanLeg, 'viaNames'>): string | null {
  const via = viaText(leg.viaNames)
  return via ? `via ${via}` : null
}

/** A explicação completa do chip "via" (dica e leitor de tela). */
export function legViaDescription(
  leg: Pick<LinkTestPlanLeg, 'viaNames'> & Partial<Pick<LinkTestPlanLeg, 'kind'>>
): string | null {
  const via = viaText(leg.viaNames)
  if (!via) return null
  return leg.kind === 'end_to_end'
    ? `Passa por ${via}: mede de uma ponta à outra, atravessando todos eles`
    : `Passa por ${via} (sem IP): a perna mede os cabos entre as duas pontas`
}

/** "OpenWrt-Sala → MikroTik-Torre". */
export function legTitle(leg: Pick<LinkTestPlanLeg, 'a' | 'b'>): string {
  return `${endpointLabel(leg.a)} → ${endpointLabel(leg.b)}`
}

export function legKindLabel(leg: Pick<LinkTestPlanLeg, 'kind'>): string {
  return leg.kind === 'end_to_end' ? 'Ponta a ponta' : 'Enlace'
}

/** Quem hospeda o servidor de teste na perna ("iperf3 em OpenWrt-Sala"). */
export function serverSideLabel(
  leg: Pick<LinkTestPlanLeg, 'a' | 'b' | 'serverSide'>
): string | null {
  if (!leg.serverSide) return null
  return `servidor em ${endpointLabel(leg.serverSide === 'a' ? leg.a : leg.b)}`
}

/**
 * Em qual ponta a etapa roda: preparar A/B em cada uma, servidor e
 * encerramento em quem serve, ida e volta em quem dirige (a outra ponta).
 */
export function stageEndpoint(
  leg: Pick<LinkTestPlanLeg, 'a' | 'b' | 'serverSide'>,
  key: LinkTestStageKey
): LinkTestEndpoint {
  if (key === 'prepare_a') return leg.a
  if (key === 'prepare_b') return leg.b
  const server = leg.serverSide === 'a' ? leg.a : leg.b
  const driver = leg.serverSide === 'a' ? leg.b : leg.a
  return key === 'serve' || key === 'stop' ? server : driver
}

/**
 * Alvo pedido (agendamento, filtro), sem as pernas: "A ↔ B" (enlace),
 * "A → B" (caminho), "Servidor → B". `nameOf` resolve o nome do equipamento.
 */
export function requestTargetLabel(
  target: {
    kind: LinkTestKind
    sourceDeviceId: number | null
    targetDeviceId: number | null
  },
  nameOf: (deviceId: number) => string
): string {
  const source = target.sourceDeviceId !== null ? nameOf(target.sourceDeviceId) : '?'
  const destination = target.targetDeviceId !== null ? nameOf(target.targetDeviceId) : '?'
  if (target.kind === 'link') return `${source} ↔ ${destination}`
  if (target.kind === 'server') return `Servidor → ${destination}`
  return `${source} → ${destination}`
}

/** Título curto do teste: "Enlace · A → B", "Ponta a ponta · A → Z". */
export function testTitle(test: Pick<LinkTest, 'id' | 'kind' | 'legs'>): string {
  const kind = TEST_KINDS[test.kind]?.label ?? 'Teste'
  if (test.legs.length === 0) return `${kind} · teste #${test.id}`
  const first = test.legs[0]
  const last = test.legs[test.legs.length - 1]
  return `${kind} · ${endpointLabel(first.a)} → ${endpointLabel(last.b)}`
}

/** As pontas do teste, sem o tipo: "AP-assistencia → BORDA". */
export function testRouteTitle(test: Pick<LinkTest, 'id' | 'legs'>): string {
  if (test.legs.length === 0) return `Teste #${test.id}`
  return `${endpointLabel(test.legs[0].a)} → ${endpointLabel(test.legs[test.legs.length - 1].b)}`
}

/**
 * Taxa medida em forma compacta: três algarismos significativos bastam
 * ("487 Mbps", "94.2 Mbps", "1.2 Gbps") — a medição não tem mais resolução.
 */
export function compactBps(bps: number | null | undefined): string {
  if (typeof bps !== 'number' || !Number.isFinite(bps)) return '—'
  if (bps <= 0) return formatBps(0)
  const mantissa = bps / 1000 ** Math.floor(Math.log10(bps) / 3)
  return formatBps(bps, { fractionDigits: mantissa >= 100 ? 0 : 1 })
}

/**
 * Par de sentidos em uma linha: "↓ 487 Mbps ↑ 471 Mbps" (↓ ida A→B, ↑ volta
 * B→A). `null` quando nenhum dos dois tem valor.
 */
export function flowPairLabel(
  forwardBps: number | null | undefined,
  reverseBps: number | null | undefined
): string | null {
  const parts: string[] = []
  if (typeof forwardBps === 'number') parts.push(`↓ ${compactBps(forwardBps)}`)
  if (typeof reverseBps === 'number') parts.push(`↑ ${compactBps(reverseBps)}`)
  return parts.length > 0 ? parts.join(' ') : null
}

export interface LegNote {
  text: string
  icon: string
  color: ThemeColorToken
}

/**
 * Problemas da perna, sem repetição: o erro, os motivos do plano e as
 * mensagens das etapas que falharam ou foram puladas. Uma perna pulada leva o
 * mesmo motivo em todas as etapas — ele aparece uma vez só.
 */
export function legNotes(leg: Pick<LinkTestLeg, 'error' | 'issues' | 'stages'>): LegNote[] {
  const notes: LegNote[] = []
  const seen = new Set<string>()
  const add = (message: string, note: LegNote) => {
    if (seen.has(message)) return
    seen.add(message)
    notes.push(note)
  }
  if (leg.error) add(leg.error, { text: leg.error, icon: 'mdi-close-circle', color: 'error' })
  for (const issue of leg.issues) add(issue, { text: issue, icon: 'mdi-alert', color: 'warning' })
  for (const stage of leg.stages) {
    if (!stage.message || (stage.status !== 'failed' && stage.status !== 'skipped')) continue
    const failed = stage.status === 'failed'
    add(stage.message, {
      text: `${stage.label || STAGES[stage.key].label}: ${stage.message}`,
      icon: failed ? 'mdi-close-circle' : 'mdi-debug-step-over',
      color: failed ? 'error' : 'warning',
    })
  }
  return notes
}
