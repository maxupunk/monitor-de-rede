import type {
  LinkTest,
  LinkTestEndpoint,
  LinkTestLeg,
  LinkTestPlan,
  LinkTestStage,
} from '@/types/linkTests'

export function endpoint(
  deviceId: number | null,
  name: string,
  patch: Partial<LinkTestEndpoint> = {}
): LinkTestEndpoint {
  return { kind: 'device', deviceId, agentId: null, name, ip: null, platform: null, ...patch }
}

export function stage(
  key: LinkTestStage['key'],
  patch: Partial<LinkTestStage> = {}
): LinkTestStage {
  return {
    key,
    label: '',
    status: 'pending',
    message: null,
    runId: null,
    startedAt: null,
    finishedAt: null,
    ...patch,
  }
}

export function leg(seq: number, patch: Partial<LinkTestLeg> = {}): LinkTestLeg {
  return {
    seq,
    kind: 'link',
    a: endpoint(seq, `Dev-${seq}`),
    b: endpoint(seq + 1, `Dev-${seq + 1}`),
    edgeId: 100 + seq,
    engine: 'iperf3',
    serverSide: 'b',
    durationS: 5,
    runnable: true,
    issues: [],
    status: 'pending',
    stages: [stage('serve'), stage('forward'), stage('reverse'), stage('stop')],
    forwardBps: null,
    reverseBps: null,
    retransmits: null,
    error: null,
    startedAt: null,
    finishedAt: null,
    ...patch,
  }
}

export function test(id: number, patch: Partial<LinkTest> = {}): LinkTest {
  return {
    id,
    kind: 'link',
    status: 'running',
    origin: 'user',
    scheduleId: null,
    linkId: 101,
    sourceDeviceId: 1,
    targetDeviceId: 2,
    spec: { durationS: 5, direction: 'both', streams: 1, limitMbps: null },
    legs: [leg(1)],
    summary: null,
    error: null,
    requestedBy: { id: 1, name: 'Ana' },
    createdAt: '2026-10-16T12:00:00Z',
    startedAt: '2026-10-16T12:00:01Z',
    finishedAt: null,
    ...patch,
  }
}

export function plan(patch: Partial<LinkTestPlan> = {}): LinkTestPlan {
  const {
    status: _s,
    stages: _st,
    forwardBps: _f,
    reverseBps: _r,
    retransmits: _x,
    error: _e,
    startedAt: _a,
    finishedAt: _b,
    ...planLeg
  } = leg(1)
  return {
    kind: 'link',
    legs: [planLeg],
    estimatedSeconds: 16,
    warnings: [],
    blocking: [],
    willInstall: [],
    ...patch,
  }
}
