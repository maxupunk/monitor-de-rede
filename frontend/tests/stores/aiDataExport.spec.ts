import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { apiService } from '@/services/apiService'
import { useAiDataExportStore } from '@/stores/aiDataExport'
import type { AiDataExportSummary } from '@/bindings/AiDataExportSummary'
import { triggerDownload } from '@/utils/download'

vi.mock('@/utils/download', () => ({ triggerDownload: vi.fn() }))

const summary: AiDataExportSummary = {
  conversations: 12,
  messages: 80,
  users: 2,
  deviceRuns: 3,
  oldest: '2026-10-20T10:00:00Z',
  newest: '2026-11-12T10:00:00Z',
}

describe('aiDataExport store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('pede o resumo do período escolhido e "Tudo" vai sem filtro', async () => {
    const get = vi.spyOn(apiService, 'get').mockResolvedValue(summary)
    const store = useAiDataExportStore()

    await store.loadSummary()
    expect(get).toHaveBeenLastCalledWith('/ai/data-export/summary?days=30')
    expect(store.summary).toEqual(summary)

    await store.setPeriod(null)
    expect(get).toHaveBeenLastCalledWith('/ai/data-export/summary')
    expect(store.days).toBeNull()
  })

  it('baixa o pacote com nome carimbado e avisa a falha', async () => {
    const blob = new Blob(['PK'])
    const fetchZip = vi.spyOn(apiService, 'download').mockResolvedValue(blob)
    vi.spyOn(apiService, 'get').mockResolvedValue(summary)
    const store = useAiDataExportStore()

    await store.setPeriod(7)
    expect(await store.download()).toBe(true)
    expect(fetchZip).toHaveBeenCalledWith('/ai/data-export?days=7')
    expect(triggerDownload).toHaveBeenCalledWith(
      blob,
      expect.stringMatching(/^netmonitor-ia-dados-\d{8}-\d{6}\.zip$/)
    )

    fetchZip.mockRejectedValueOnce(new Error('Apenas administradores'))
    expect(await store.download()).toBe(false)
    expect(store.error).toBe('Apenas administradores')
    expect(store.downloading).toBe(false)
  })
})
