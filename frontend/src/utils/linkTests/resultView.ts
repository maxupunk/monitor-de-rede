/**
 * Regras puras do resultado "resumo primeiro" (`LinkTestResultView`): o que
 * não rodou e por quê, e quando oferecer "Medir enlace por enlace" depois de
 * uma medição ponta a ponta.
 */
import type { LinkTest, LinkTestLeg, LinkTestRequest } from '@/types/linkTests'
import { legNotes, legTitle } from './presentation'

/** Uma perna que não deu número: o título e o primeiro motivo. */
export interface MissedLeg {
  seq: number
  title: string
  reason: string
  failed: boolean
}

export function missedLegs(test: Pick<LinkTest, 'legs'>): MissedLeg[] {
  return test.legs
    .filter((leg) => leg.status === 'failed' || leg.status === 'skipped')
    .map((leg) => ({
      seq: leg.seq,
      title: legTitle(leg),
      reason: legNotes(leg)[0]?.text ?? 'Sem detalhes — veja os detalhes técnicos.',
      failed: leg.status === 'failed',
    }))
}

type DrillLeg = Pick<LinkTestLeg, 'kind' | 'a' | 'b' | 'viaDeviceIds'>

/**
 * "Medir enlace por enlace" para um teste ponta a ponta que terminou: o mesmo
 * par, agora em `path`. Só quando há outro equipamento com IP no meio — com
 * só switches sem IP, enlace por enlace mediria a mesma perna.
 */
export function perHopRequest(
  test: Pick<LinkTest, 'kind' | 'legs' | 'spec'>,
  hasIp: (deviceId: number) => boolean
): Omit<LinkTestRequest, 'confirm'> | null {
  if (test.kind !== 'end_to_end' && test.kind !== 'server') return null
  const leg: DrillLeg | undefined = test.legs.find((item) => item.kind === 'end_to_end')
  if (!leg || leg.a.deviceId === null || leg.b.deviceId === null) return null
  if (!(leg.viaDeviceIds ?? []).some(hasIp)) return null
  return {
    kind: 'path',
    sourceDeviceId: leg.a.deviceId,
    targetDeviceId: leg.b.deviceId,
    spec: test.spec,
  }
}

/** O pedido que refaz o mesmo teste, com os mesmos parâmetros. */
export function repeatRequest(
  test: Pick<LinkTest, 'kind' | 'linkId' | 'sourceDeviceId' | 'targetDeviceId' | 'spec'>
): Omit<LinkTestRequest, 'confirm'> {
  return {
    kind: test.kind,
    ...(test.linkId !== null ? { linkId: test.linkId } : {}),
    ...(test.sourceDeviceId !== null ? { sourceDeviceId: test.sourceDeviceId } : {}),
    ...(test.targetDeviceId !== null ? { targetDeviceId: test.targetDeviceId } : {}),
    spec: test.spec,
  }
}
