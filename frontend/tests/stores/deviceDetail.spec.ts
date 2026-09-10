import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useDeviceDetailStore, SNMP_REQUEST_TIMEOUT_MS } from '@/stores/deviceDetail'
import { apiService } from '@/services/apiService'

vi.mock('@/services/apiService', () => ({
  apiService: {
    get: vi.fn(),
    post: vi.fn(),
    patch: vi.fn(),
  },
  ApiError: class extends Error {},
  NetworkError: class extends Error {},
}))

const mocked = vi.mocked(apiService)

/**
 * Toda rota `/snmp/*` espera o equipamento responder — `apply-monitors` faz dois
 * walks completos. Com o teto padrão de 15 s do `apiService`, o navegador
 * abortava a requisição com o backend ainda escrevendo: a tela ficava
 * carregando "sem acontecer nada" e a gravação seguia sem ninguém saber.
 */
describe('deviceDetail: chamadas SNMP declaram o próprio timeout', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.clearAllMocks()
    mocked.get.mockResolvedValue([])
    mocked.post.mockResolvedValue({})
    mocked.patch.mockResolvedValue({})
  })

  it('o teto declarado é maior que o padrão global de 15 s', () => {
    expect(SNMP_REQUEST_TIMEOUT_MS).toBeGreaterThan(15_000)
  })

  it('aplicar configurações de monitoramento não usa o teto padrão', async () => {
    const store = useDeviceDetailStore()
    await store.applySnmpMonitors(2, { monitoredIfIndexes: [55] })

    const chamada = mocked.post.mock.calls.find(([rota]) =>
      String(rota).endsWith('/snmp/apply-monitors')
    )
    expect(chamada?.[2]).toEqual({ timeoutMs: SNMP_REQUEST_TIMEOUT_MS })
  })

  it('escanear e coletar também declaram o teto', async () => {
    const store = useDeviceDetailStore()
    await store.scanDeviceSnmp(2)
    await store.triggerSnmpPoll(2)

    for (const sufixo of ['/snmp/scan', '/snmp/poll']) {
      const chamada = mocked.post.mock.calls.find(([rota]) => String(rota).endsWith(sufixo))
      expect(chamada, `faltou chamada para ${sufixo}`).toBeDefined()
      expect(chamada?.[2]).toEqual({ timeoutMs: SNMP_REQUEST_TIMEOUT_MS })
    }
  })

  it('ligar uma interface pelo PATCH também declara o teto', async () => {
    const store = useDeviceDetailStore()
    await store.setInterfaceMonitoring(2, 21, true)

    expect(mocked.patch).toHaveBeenCalledWith(
      '/devices/2/interfaces/21/monitoring',
      { enabled: true },
      { timeoutMs: SNMP_REQUEST_TIMEOUT_MS }
    )
  })
})
