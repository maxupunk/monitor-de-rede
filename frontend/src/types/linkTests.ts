/**
 * Contrato de dados do teste de enlace (ADR 015 §5.1) — espelho exato dos
 * tipos que o backend exporta por `ts-rs` para `@/bindings/`. Enquanto os
 * bindings não chegam, este é o ÚNICO lugar com estes tipos: o resto do
 * frontend importa daqui. Quando chegarem, troque cada declaração da primeira
 * parte por `export type { X } from '@/bindings/X'`; os apelidos da segunda
 * parte são derivados e continuam valendo.
 */

// --- Espelho do §5.1 (nomes e formas dos bindings) ---------------------------

export type LinkTestKind = 'link' | 'path' | 'server'

export type LinkTestStatus = 'queued' | 'running' | 'succeeded' | 'partial' | 'failed' | 'cancelled'

export type LinkTestStepStatus =
  'pending' | 'running' | 'succeeded' | 'failed' | 'skipped' | 'cancelled'

export type LinkTestFlow = 'forward' | 'reverse'

export type LinkTestSpecInput = {
  /** Segundos por sentido: padrão 5, de 1 a 60. */
  durationS?: number
  direction?: 'both' | 'forward' | 'reverse'
  /** Fluxos paralelos (só iperf3): padrão 1, de 1 a 8. */
  streams?: number
  limitMbps?: number | null
}

export type LinkTestRequest = {
  kind: LinkTestKind
  linkId?: number
  sourceDeviceId?: number
  targetDeviceId?: number
  spec?: LinkTestSpecInput
  confirm?: boolean
}

export type LinkTestEndpoint = {
  kind: 'device' | 'central' | 'agent'
  deviceId: number | null
  agentId: number | null
  name: string
  ip: string | null
  platform: string | null
}

export type LinkTestPlan = {
  kind: LinkTestKind
  legs: LinkTestPlanLeg[]
  estimatedSeconds: number
  warnings: string[]
  /** Não vazio = não roda. */
  blocking: string[]
  willInstall: { deviceId: number; deviceName: string; packages: string[] }[]
}

export type LinkTestPlanLeg = {
  seq: number
  kind: 'link' | 'end_to_end'
  a: LinkTestEndpoint
  b: LinkTestEndpoint
  edgeId: number | null
  engine: 'iperf3' | 'btest' | null
  serverSide: 'a' | 'b' | null
  durationS: number
  runnable: boolean
  issues: string[]
}

export type LinkTestStage = {
  key: 'prepare_a' | 'prepare_b' | 'serve' | 'forward' | 'reverse' | 'stop'
  label: string
  status: LinkTestStepStatus
  message: string | null
  runId: number | null
  startedAt: string | null
  finishedAt: string | null
}

export type LinkTestLeg = LinkTestPlanLeg & {
  status: LinkTestStepStatus
  stages: LinkTestStage[]
  forwardBps: number | null
  reverseBps: number | null
  retransmits: number | null
  error: string | null
  startedAt: string | null
  finishedAt: string | null
  /** Só no GET de detalhe. */
  samples?: { flow: LinkTestFlow; bps: number; tMs: number }[]
}

export type LinkTest = {
  id: number
  kind: LinkTestKind
  status: LinkTestStatus
  origin: 'user' | 'schedule'
  scheduleId: number | null
  linkId: number | null
  sourceDeviceId: number | null
  targetDeviceId: number | null
  spec: Required<LinkTestSpecInput>
  legs: LinkTestLeg[]
  summary: {
    bottleneckLegSeq: number | null
    minForwardBps: number | null
    minReverseBps: number | null
  } | null
  error: string | null
  requestedBy: { id: number; name: string } | null
  createdAt: string
  startedAt: string | null
  finishedAt: string | null
}

export type LinkTestPage = { items: LinkTest[]; total: number; page: number; perPage: number }

export type LinkTestSchedule = {
  id: number
  name: string
  kind: LinkTestKind
  linkId: number | null
  sourceDeviceId: number | null
  targetDeviceId: number | null
  spec: Required<LinkTestSpecInput>
  /** ≥ 15. */
  intervalMinutes: number
  /** "HH:MM". */
  windowStart: string | null
  windowEnd: string | null
  enabled: boolean
  lastRunAt: string | null
  nextRunAt: string | null
  lastTestId: number | null
  lastStatus: LinkTestStatus | null
  createdAt: string
  updatedAt: string
}

export type LinkTestScheduleInput = Omit<
  LinkTestSchedule,
  | 'id'
  | 'lastRunAt'
  | 'nextRunAt'
  | 'lastTestId'
  | 'lastStatus'
  | 'createdAt'
  | 'updatedAt'
  | 'spec'
> & { spec?: LinkTestSpecInput }

// --- Apelidos derivados (sobrevivem à troca pelos bindings) ------------------

export type LinkTestSpec = Required<LinkTestSpecInput>
export type LinkTestDirection = LinkTestSpec['direction']
export type LinkTestEngine = NonNullable<LinkTestPlanLeg['engine']>
export type LinkTestStageKey = LinkTestStage['key']
export type LinkTestSummary = NonNullable<LinkTest['summary']>
export type LinkTestSample = NonNullable<LinkTestLeg['samples']>[number]

/** `link_test:sample` — efêmero, uma amostra por segundo por sentido. */
export type LinkTestSampleEvent = LinkTestSample & { testId: number; legSeq: number }
