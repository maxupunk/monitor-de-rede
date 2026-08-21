import { describe, it, expect, vi, beforeEach } from 'vitest'
import { setActivePinia, createPinia } from 'pinia'
import {
  usePreferencesStore,
  defaultPreferences,
  MIN_PING_INTERVAL_SECONDS,
  MAX_PING_INTERVAL_SECONDS,
} from '../../src/stores/preferences.ts'

const getMock = vi.fn()
const putMock = vi.fn()
vi.mock('@/services/apiService', () => ({
  apiService: {
    get: (...args: unknown[]) => getMock(...args),
    put: (...args: unknown[]) => putMock(...args),
  },
}))

describe('preferences store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    getMock.mockReset()
    putMock.mockReset()
  })

  it('expõe valores padrão ao inicializar', () => {
    const store = usePreferencesStore()

    expect(store.preferences).toEqual(defaultPreferences())
    expect(store.loading).toBe(false)
    expect(store.saving).toBe(false)
  })

  it('carrega preferências do servidor', async () => {
    const remote = {
      defaultPingIntervalSeconds: 120,
      defaultSnmpCommunity: 'private',
      autoDiscoveryEnabled: false,
    }
    getMock.mockResolvedValueOnce(remote)

    const store = usePreferencesStore()
    await store.fetchAll()

    expect(getMock).toHaveBeenCalledWith('/settings')
    expect(store.preferences).toEqual(remote)
    expect(store.loaded).toBe(true)
    expect(store.error).toBe('')
  })

  it('não recarrega quando já está carregado', async () => {
    getMock.mockResolvedValueOnce(defaultPreferences())

    const store = usePreferencesStore()
    await store.fetchAll()
    await store.fetchAll()

    expect(getMock).toHaveBeenCalledTimes(1)
  })

  it('salva e adota o documento devolvido pelo servidor', async () => {
    const sent = { ...defaultPreferences(), defaultPingIntervalSeconds: 90 }
    const returned = { ...sent, defaultSnmpCommunity: 'public' }
    putMock.mockResolvedValueOnce(returned)

    const store = usePreferencesStore()
    const ok = await store.save(sent)

    expect(ok).toBe(true)
    expect(putMock).toHaveBeenCalledWith('/settings', sent)
    expect(store.preferences).toEqual(returned)
  })

  it('reporta erro quando o servidor falha', async () => {
    putMock.mockRejectedValueOnce(new Error('timeout'))

    const store = usePreferencesStore()
    const ok = await store.save(defaultPreferences())

    expect(ok).toBe(false)
    expect(store.error).toBe('timeout')
  })

  it('exporta constantes de validação', () => {
    expect(MIN_PING_INTERVAL_SECONDS).toBe(10)
    expect(MAX_PING_INTERVAL_SECONDS).toBe(86_400)
  })
})
