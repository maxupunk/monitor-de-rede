import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useLogsStore } from '@/stores/logs'
import { apiService } from '@/services/apiService'

vi.mock('@/services/apiService', () => ({
  apiService: {
    get: vi.fn(),
    post: vi.fn(),
    download: vi.fn(),
  },
  ApiError: class extends Error {},
  NetworkError: class extends Error {},
}))

const mocked = vi.mocked(apiService)

describe('logs store: exportação e download de logs', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.clearAllMocks()
    mocked.download.mockResolvedValue(new Blob(['log content'], { type: 'text/plain' }))

    // Mock URL.createObjectURL e revokeObjectURL
    globalThis.URL.createObjectURL = vi.fn(() => 'blob:http://localhost/mock-blob')
    globalThis.URL.revokeObjectURL = vi.fn()
  })

  it('downloadExport chama apiService.download com os filtros de dispositivo, severidade, período e busca', async () => {
    const store = useLogsStore()
    const success = await store.downloadExport(
      {
        deviceId: 2,
        severity: 3,
        hours: 24,
        search: 'falha de login',
      },
      'Roteador-Borda'
    )

    expect(success).toBe(true)
    expect(mocked.download).toHaveBeenCalledTimes(1)

    const urlChamada = mocked.download.mock.calls[0][0]
    expect(urlChamada).toContain('/logs/export?')
    expect(urlChamada).toContain('deviceId=2')
    expect(urlChamada).toContain('severity=3')
    expect(urlChamada).toContain('from=')
    expect(urlChamada).toContain('q=falha+de+login')

    expect(globalThis.URL.createObjectURL).toHaveBeenCalled()
    expect(globalThis.URL.revokeObjectURL).toHaveBeenCalledWith('blob:http://localhost/mock-blob')
  })

  it('downloadExport respeita formato csv quando especificado', async () => {
    const store = useLogsStore()
    await store.downloadExport({ deviceId: 2 }, 'Roteador-Borda', 'csv')

    const urlChamada = mocked.download.mock.calls[0][0]
    expect(urlChamada).toContain('format=csv')
  })
})
