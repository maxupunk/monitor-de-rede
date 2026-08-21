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
})
