import { describe, it, expect, vi, beforeEach } from 'vitest'
import { setActivePinia, createPinia } from 'pinia'
import { useAlertsStore, type AlertEvent } from '../../src/stores/alerts.ts'

const getMock = vi.fn()
const postMock = vi.fn()
const putMock = vi.fn()
const deleteMock = vi.fn()
vi.mock('@/services/apiService', () => ({
  apiService: {
    get: (...args: unknown[]) => getMock(...args),
    post: (...args: unknown[]) => postMock(...args),
    put: (...args: unknown[]) => putMock(...args),
    delete: (...args: unknown[]) => deleteMock(...args),
  },
}))

function makeAlert(overrides: Partial<AlertEvent> = {}): AlertEvent {
  return {
    id: 1,
    severity: 'warning',
    status: 'active',
    title: 'Alerta',
    message: 'Mensagem',
    createdAt: new Date().toISOString(),
    ...overrides,
  }
}

describe('alerts store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    getMock.mockReset()
    postMock.mockReset()
    putMock.mockReset()
    deleteMock.mockReset()
  })

  it('separa eventos ativos, pendentes, reconhecidos e resolvidos', () => {
    const store = useAlertsStore()
    store.alertEvents = [
      makeAlert({ id: 1, status: 'active' }),
      makeAlert({ id: 2, status: 'acknowledged' }),
      makeAlert({ id: 3, status: 'resolved' }),
      makeAlert({ id: 4, status: 'active', severity: 'critical' }),
    ]

    expect(store.activeAlerts).toHaveLength(3)
    expect(store.pendingAlerts).toHaveLength(2)
    expect(store.acknowledgedAlerts).toHaveLength(1)
    expect(store.resolvedAlerts).toHaveLength(1)
    expect(store.criticalCount).toBe(1)
  })

  it('insere novo evento no topo quando não existe', () => {
    const store = useAlertsStore()
    const event = makeAlert({ id: 5 })
    store.upsertAlertEvent(event)

    expect(store.alertEvents[0]).toEqual(event)
    expect(store.lastRealtimeUpdateAt).not.toBeNull()
  })

  it('atualiza evento existente sobrescrevendo com os dados novos', () => {
    const store = useAlertsStore()
    store.alertEvents = [makeAlert({ id: 1, status: 'active', title: 'Antigo' })]

    store.upsertAlertEvent(makeAlert({ id: 1, status: 'resolved', title: 'Novo' }))

    expect(store.alertEvents[0].status).toBe('resolved')
    expect(store.alertEvents[0].title).toBe('Novo')
  })

  it('aplica patch parcial em evento existente', () => {
    const store = useAlertsStore()
    store.alertEvents = [makeAlert({ id: 1, status: 'active' })]

    store.patchAlertEvent(1, { status: 'acknowledged' })

    expect(store.alertEvents[0].status).toBe('acknowledged')
  })

  it('normaliza isEnabled ao inserir regra', () => {
    const store = useAlertsStore()
    store.upsertAlertRule({
      id: 1,
      name: 'Regra',
      type: 'device_offline',
      condition: { field: 'status', operator: 'eq', value: 'offline' },
      severity: 'warning',
      durationSeconds: 60,
      enabled: true,
    } as ReturnType<typeof useAlertsStore>['alertRules'][number])

    expect(store.alertRules[0].isEnabled).toBe(true)
  })

  it('remove regra pelo id', () => {
    const store = useAlertsStore()
    store.alertRules = [
      {
        id: 1,
        name: 'Regra',
        type: 'device_offline',
        condition: { field: 'status', operator: 'eq', value: 'offline' },
        severity: 'warning',
        durationSeconds: 60,
        enabled: true,
        isEnabled: true,
      },
    ]

    store.removeAlertRule(1)

    expect(store.alertRules).toHaveLength(0)
  })

  it('obtem diagnostico global de causa raiz (RCA) e preenche rcaSummary e activeClusters', async () => {
    const store = useAlertsStore()
    const mockSummary = {
      activeClusters: [
        {
          id: 'cluster-1',
          rootCauseEvent: makeAlert({ id: 10, title: 'Gateway Inacessível' }),
          rootCauseDeviceId: 1,
          rootCauseDeviceName: 'Gateway Principal',
          causalCategory: 'gateway',
          causalCategoryLabel: 'Gateway da Rede',
          confidence: 90,
          explanation:
            '17 dispositivos ficaram inacessíveis após 192.168.1.1 parar de responder — causa provável: Gateway da Rede',
          impactedDevicesCount: 17,
          totalAlertsCount: 18,
          events: [makeAlert({ id: 10 }), makeAlert({ id: 11 })],
          startedAt: new Date().toISOString(),
          maxSeverity: 'critical',
        },
      ],
      totalActiveIncidents: 1,
      totalCorrelatedAlerts: 18,
    }

    getMock.mockResolvedValueOnce(mockSummary)

    const result = await store.fetchRootCauseAnalysis()

    expect(getMock).toHaveBeenCalledWith('/alerts/root-cause-analysis')
    expect(result).toEqual(mockSummary)
    expect(store.rcaSummary).toEqual(mockSummary)
    expect(store.activeClusters).toHaveLength(1)
    expect(store.activeClusters[0].causalCategory).toBe('gateway')
  })

  it('obtem correlacao pontual de um alerta com cadeia de dependencia', async () => {
    const store = useAlertsStore()
    const mockCorrelation = {
      windowSeconds: 60,
      primaryCause: makeAlert({ id: 10, title: 'Roteador Offline' }),
      causalCategory: 'router',
      causalCategoryLabel: 'Roteador Principal',
      confidence: 85,
      explanation: 'Dispositivo inacessível após queda do roteador',
      impactedDevicesCount: 2,
      impactedDevices: [{ id: 2, name: 'Servidor', type: 'server', status: 'offline' }],
      dependencyChain: [
        {
          id: 1,
          name: 'Router',
          type: 'router',
          status: 'offline',
          isRootCause: true,
          isTarget: false,
        },
        {
          id: 2,
          name: 'Servidor',
          type: 'server',
          status: 'offline',
          isRootCause: false,
          isTarget: true,
        },
      ],
      relatedEvents: [],
      correlationCount: 1,
    }

    getMock.mockResolvedValueOnce(mockCorrelation)

    const result = await store.fetchCorrelation(20)

    expect(getMock).toHaveBeenCalledWith('/alerts/20/correlation')
    expect(result).toEqual(mockCorrelation)
    expect(result?.confidence).toBe(85)
    expect(result?.dependencyChain).toHaveLength(2)
  })
})
