import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { apiService } from '@/services/apiService'
import { useLogsStore, type LogEntry, type LogTemplateInfo } from '@/stores/logs'
import { isLogTemplateInfo } from '@/utils/logCategories'

function template(overrides: Partial<LogTemplateInfo> = {}): LogTemplateInfo {
  return {
    templateHash: '5946f5852770d51e',
    template: 'ether# link down',
    example: 'ether3 link down',
    category: 'link_change',
    confidence: 88,
    model: 'laya:multilingual',
    confirmed: false,
    alertTemplate: 'log_pppoe_flapping',
    alertRegex: 'ether\\S* link down',
    ...overrides,
  }
}

function entry(templateHash: string | null): LogEntry {
  return {
    id: 1,
    deviceId: 1,
    deviceName: 'Borda',
    sourceIp: '10.0.0.1',
    receivedAt: '2026-09-30T12:00:00Z',
    deviceTime: null,
    facility: null,
    severity: 4,
    severityLabel: 'aviso',
    hostname: null,
    appName: null,
    pid: null,
    topics: [],
    message: 'ether3 link down',
    templateHash,
  }
}

describe('categorias de padrão de log', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('o evento SSE dá a categoria à linha sem consultar a API', () => {
    const get = vi.spyOn(apiService, 'get')
    const store = useLogsStore()
    const line = entry('5946f5852770d51e')
    expect(store.templateOf(line)).toBeNull()

    const payload: unknown = template()
    expect(isLogTemplateInfo(payload)).toBe(true)
    if (isLogTemplateInfo(payload)) store.applyTemplateClassified(payload)

    expect(store.templateOf(line)?.category).toBe('link_change')
    expect(store.templateOf(entry(null))).toBeNull()
    expect(get).not.toHaveBeenCalled()
  })

  it('a correção do operador substitui o palpite', async () => {
    vi.spyOn(apiService, 'put').mockResolvedValue(
      template({ category: 'hardware', confirmed: true, confidence: null, model: null })
    )
    const store = useLogsStore()
    store.applyTemplateClassified(template())

    expect(await store.confirmTemplate('5946f5852770d51e', 'hardware')).toBeNull()
    const info = store.templateOf(entry('5946f5852770d51e'))
    expect(info?.category).toBe('hardware')
    expect(info?.confirmed).toBe(true)
  })

  it('payload estranho não passa pelo guard', () => {
    expect(isLogTemplateInfo({ templateHash: 7 })).toBe(false)
    expect(isLogTemplateInfo(null)).toBe(false)
  })
})
