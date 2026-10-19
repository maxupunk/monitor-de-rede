import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises } from '@vue/test-utils'
import type { Pinia } from 'pinia'
import type { QuickPingReport } from '@/bindings/QuickPingReport'
import QuickPingDialog from '@/components/diagnostics/QuickPingDialog.vue'
import QuickPingResult from '@/components/diagnostics/QuickPingResult.vue'
import { quickPingActions } from '@/components/topology/quickPing/registerQuickPing'
import { apiService } from '@/services/apiService'
import { useDiagnosticsStore } from '@/stores/diagnostics'
import { useLinkTestRequestsStore } from '@/stores/linkTestRequests'
import {
  canQuickPing,
  capacityOutcome,
  capacityRangeLabel,
  confidenceLabel,
  lossColor,
  queueingBadge,
} from '@/utils/diagnostics/quickPing'
import type { TopologyHost } from '@/utils/topology/inspectorRegistry'
import { node } from '../utils/topology/fixtures'
import { bodyText, button, mountWith, setRole, setupDom, teardownDom } from './domHarness'

function report(patch: Partial<QuickPingReport> = {}): QuickPingReport {
  return {
    target: '10.0.0.242',
    sent: 160,
    samples: 160,
    lossPct: 0,
    small: { bytes: 56, sent: 80, received: 80, minMs: 0.81, p50Ms: 0.95, p95Ms: 1.2 },
    large: { bytes: 1472, sent: 80, received: 80, minMs: 0.97, p50Ms: 1.4, p95Ms: 4.9 },
    jitterMs: 0.12,
    queueing: { inflationMs: 3.93, level: 'moderado' },
    deltaMinMs: 0.16,
    capacity: { lowMbps: 82, highMbps: 170, confidence: 'baixa' },
    capacityNote: 'Ordem de grandeza do caminho todo, da central até o equipamento.',
    verdict: 'Sinais moderados de fila ou perda.',
    ...patch,
  }
}

describe('apresentação do diagnóstico rápido', () => {
  it('cores semânticas para fila, perda e confiança', () => {
    expect(queueingBadge('ok').color).toBe('success')
    expect(queueingBadge('moderado').color).toBe('warning')
    expect(queueingBadge('alto').color).toBe('error')
    expect(lossColor(0)).toBe('success')
    expect(lossColor(1.25)).toBe('warning')
    expect(lossColor(6)).toBe('error')
    expect(confidenceLabel('muito_baixa')).toBe('Confiança muito baixa')
  })

  it('faixa em Mbit/s pelo formatador central e os três desfechos', () => {
    expect(capacityRangeLabel({ lowMbps: 82, highMbps: 170, confidence: 'baixa' })).toBe(
      '82 Mbps – 170 Mbps'
    )
    expect(capacityOutcome(report())).toBe('range')
    expect(capacityOutcome(report({ capacity: null }))).toBe('not_estimated')
    expect(capacityOutcome(report({ capacity: null, samples: 0 }))).toBe('no_reply')
  })

  it('só oferece para nó com IP', () => {
    expect(canQuickPing({ ipAddress: '10.0.0.2' })).toBe(true)
    expect(canQuickPing({ ipAddress: null })).toBe(false)
    expect(canQuickPing({ ipAddress: '  ' })).toBe(false)
  })
})

describe('ação do mapa', () => {
  const host = { systemNodeId: 1 } as TopologyHost

  it('aparece em qualquer nó com IP (até desconhecido) e abre o diálogo', () => {
    const open = vi.fn()
    const unknown = node(7, { type: 'unknown', ipAddress: '10.0.0.7' })
    const [action] = quickPingActions(open)({ kind: 'node', node: unknown }, host)
    expect(action.title).toBe('Diagnóstico rápido (ping)')
    expect(action.group).toBe('test')
    expect(action.requiresWrite).toBe(true)
    action.run()
    expect(open).toHaveBeenCalledWith(unknown)
  })

  it('some sem IP, na própria central e fora de nó', () => {
    const provider = quickPingActions(vi.fn())
    expect(provider({ kind: 'node', node: node(8, { ipAddress: null }) }, host)).toEqual([])
    expect(provider({ kind: 'node', node: node(1, { ipAddress: '10.0.0.1' }) }, host)).toEqual([])
    expect(provider({ kind: 'background' }, host)).toEqual([])
  })
})

describe('QuickPingResult.vue e QuickPingDialog.vue', () => {
  let pinia: Pinia
  beforeEach(() => {
    pinia = setupDom()
  })
  afterEach(teardownDom)

  it('com faixa: latência, perda, fila e capacidade com a confiança', () => {
    const wrapper = mountWith(QuickPingResult, { report: report() }, pinia)
    const text = wrapper.text()
    expect(text).toContain('0.8 ms')
    expect(text).toContain('p95 4.9 ms')
    expect(text).toContain('Fila moderada')
    expect(wrapper.find('[data-test="capacity-range"]').text()).toBe('82 Mbps – 170 Mbps')
    expect(text).toContain('Confiança baixa')
    wrapper.unmount()
  })

  it('rápido demais: sem número, com a nota do backend', () => {
    const note = 'Acima de ~190 Mbit/s — rápido demais para estimar por ping.'
    const wrapper = mountWith(
      QuickPingResult,
      { report: report({ capacity: null, capacityNote: note }) },
      pinia
    )
    expect(wrapper.find('[data-test="capacity-range"]').exists()).toBe(false)
    expect(wrapper.find('[data-test="capacity-none"]').text()).toBe('Não estimada')
    expect(wrapper.text()).toContain(note)
    wrapper.unmount()
  })

  it('sem resposta: só o veredito e a nota, sem blocos de latência', () => {
    const wrapper = mountWith(
      QuickPingResult,
      {
        report: report({
          samples: 0,
          lossPct: 100,
          small: null,
          large: null,
          jitterMs: null,
          queueing: null,
          capacity: null,
          capacityNote: 'Sem resposta ao ping — nada a estimar.',
          verdict: 'O equipamento não respondeu a nenhum ping.',
        }),
      },
      pinia
    )
    const text = wrapper.text()
    expect(text).toContain('não respondeu')
    expect(text).not.toContain('Jitter')
    expect(text).toContain('Sem resposta ao ping')
    wrapper.unmount()
  })

  it('o diálogo roda ao abrir e o CTA pede o teste completo do servidor', async () => {
    setRole('operator')
    const post = vi.spyOn(apiService, 'post').mockResolvedValue(report())
    const wrapper = mountWith(
      QuickPingDialog,
      { modelValue: true, deviceId: 42, deviceName: 'RB922', offerFullTest: true },
      pinia
    )
    await flushPromises()
    expect(post).toHaveBeenCalledWith(
      '/diagnostics/quick-ping',
      { deviceId: 42 },
      expect.objectContaining({ timeoutMs: expect.any(Number) })
    )
    expect(bodyText()).toContain('estimativa de baixa precisão')
    expect(bodyText()).toContain('82 Mbps – 170 Mbps')

    button('Fazer o teste de velocidade completo')?.click()
    expect(useLinkTestRequestsStore().pending).toMatchObject({
      kind: 'server',
      targetDeviceId: 42,
    })
    expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual([false])
    wrapper.unmount()
  })

  it('perfil leitura não vê o CTA; erro da API aparece no diálogo', async () => {
    setRole('viewer')
    vi.spyOn(apiService, 'post').mockRejectedValue(new Error('Ping indisponível: sysctl'))
    const wrapper = mountWith(
      QuickPingDialog,
      { modelValue: true, deviceId: 42, offerFullTest: true },
      pinia
    )
    await flushPromises()
    expect(button('Fazer o teste de velocidade completo')).toBeUndefined()
    expect(bodyText()).toContain('Ping indisponível')
    expect(useDiagnosticsStore().quickPingError).toContain('Ping indisponível')
    wrapper.unmount()
  })
})
