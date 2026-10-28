import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { apiService } from '@/services/apiService'
import { useEventsStore } from '@/stores/events'
import { useWifiClientsStore } from '@/stores/wifiClients'
import type { WifiDeviceSnapshot } from '@/bindings/WifiDeviceSnapshot'
import type { WifiWatchedClient } from '@/bindings/WifiWatchedClient'
import { snapshot, station } from '../utils/wifiFixtures'

class FakeEventSource {
  static latest: FakeEventSource | null = null
  onopen: (() => void) | null = null
  onmessage: ((event: MessageEvent<string>) => void) | null = null
  onerror: (() => void) | null = null

  constructor(_url: string) {
    FakeEventSource.latest = this
  }

  close() {}
}

function emit(type: string, data: unknown) {
  FakeEventSource.latest?.onmessage?.({
    data: JSON.stringify({ type, timestamp: '2026-10-26T12:00:00Z', data }),
  } as MessageEvent<string>)
}

describe('SSE dos clientes Wi-Fi', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.stubGlobal('EventSource', FakeEventSource)
    vi.stubGlobal('localStorage', {
      getItem: vi.fn(() => null),
      setItem: vi.fn(),
      removeItem: vi.fn(),
      clear: vi.fn(),
    })
  })

  afterEach(() => {
    vi.unstubAllGlobals()
    vi.restoreAllMocks()
  })

  it('aplica o snapshot de cada roteador sem consultar a API e fora do feed', () => {
    const get = vi.spyOn(apiService, 'get')
    const post = vi.spyOn(apiService, 'post')
    const events = useEventsStore()
    const store = useWifiClientsStore()
    events.connect()

    emit('wifi:snapshot', snapshot(1, 'AP Sala', [station('aa:bb:cc:dd:ee:01', -82, 'critical')]))
    emit('wifi:snapshot', snapshot(2, 'AP Depósito', [station('aa:bb:cc:dd:ee:02', -50)]))
    // A leitura seguinte do AP Sala substitui a anterior.
    const lendo: WifiDeviceSnapshot = { ...snapshot(1, 'AP Sala', []), status: 'reading' }
    emit('wifi:snapshot', lendo)
    // Payload fora do formato: ignorado.
    emit('wifi:snapshot', { deviceId: 'um', stations: [] })

    expect(store.devices.map((device) => device.deviceName)).toEqual(['AP Depósito', 'AP Sala'])
    expect(store.snapshotFor(1)?.status).toBe('reading')
    expect(store.reading).toBe(true)
    expect(store.rows.map((row) => row.mac)).toEqual(['aa:bb:cc:dd:ee:02'])
    expect(get).not.toHaveBeenCalled()
    expect(post).not.toHaveBeenCalled()
    expect(events.recentEvents).toHaveLength(0)
  })

  it('leitura de todos tira da tela quem perdeu o plugin Wi-Fi', async () => {
    const store = useWifiClientsStore()
    store.applySnapshot(snapshot(1, 'AP Sala', []))
    store.applySnapshot(snapshot(2, 'AP Antigo', []))
    const post = vi.spyOn(apiService, 'post').mockResolvedValue({ deviceIds: [1] })

    expect(await store.refresh()).toBe(true)

    expect(post).toHaveBeenCalledWith('/wifi/refresh', {})
    expect(store.devices.map((device) => device.deviceId)).toEqual([1])
    expect(store.lastFleetRefresh).toEqual({ deviceIds: [1] })
  })

  it('o ping guarda resultado e erro por estação', async () => {
    const store = useWifiClientsStore()
    store.applySnapshot(snapshot(1, 'AP Sala', [station('aa:bb:cc:dd:ee:01', -60)]))
    const row = store.rows[0]!
    const result = {
      deviceId: 1,
      target: '192.168.1.10',
      sent: 3,
      received: 3,
      lossPct: 0,
      avgMs: 4.2,
      grade: 'excellent',
    }
    vi.spyOn(apiService, 'post').mockResolvedValueOnce(result)
    await store.ping(row)
    expect(store.pingOf(1, row.mac)).toEqual({ loading: false, result, error: null })

    vi.spyOn(apiService, 'post').mockRejectedValueOnce(new Error('roteador fora do ar'))
    await store.ping(row)
    expect(store.pingOf(1, row.mac)?.error).toBe('roteador fora do ar')
  })

  it('aplica a lista dos vigiados pelo SSE sem consultar a API', () => {
    const get = vi.spyOn(apiService, 'get')
    const events = useEventsStore()
    const store = useWifiClientsStore()
    events.connect()

    const camera: WifiWatchedClient = {
      id: 1,
      mac: 'aa:bb:cc:dd:ee:01',
      name: 'Câmera da portaria',
      randomizedMac: false,
      state: 'offline',
      deviceId: 2,
      deviceName: 'AP Portaria',
      lastSeenAt: '2026-10-26T11:40:00Z',
    }
    emit('wifi:watched', { clients: [camera] })
    emit('wifi:watched', { clients: 'nada' })

    expect(store.watched).toEqual([camera])
    expect(store.watchedByMac.get('aa:bb:cc:dd:ee:01')?.name).toBe('Câmera da portaria')
    expect(get).not.toHaveBeenCalled()
    expect(events.recentEvents).toHaveLength(0)
  })

  it('vigiar e deixar de vigiar atualizam a lista na hora', async () => {
    const store = useWifiClientsStore()
    const camera: WifiWatchedClient = {
      id: 3,
      mac: 'aa:bb:cc:dd:ee:03',
      name: 'Câmera',
      randomizedMac: false,
      state: 'pending',
      deviceId: null,
      deviceName: null,
      lastSeenAt: null,
    }
    const post = vi.spyOn(apiService, 'post').mockResolvedValueOnce(camera)
    expect(await store.watch(camera.mac, 'Câmera')).toBe(true)
    expect(post).toHaveBeenCalledWith('/wifi/watched', { mac: camera.mac, name: 'Câmera' })
    expect(store.watched).toEqual([camera])

    vi.spyOn(apiService, 'delete').mockResolvedValueOnce(undefined)
    expect(await store.unwatch(3)).toBe(true)
    expect(store.watched).toEqual([])

    vi.spyOn(apiService, 'post').mockRejectedValueOnce(new Error('Este aparelho já é vigiado'))
    expect(await store.watch(camera.mac, 'Outra')).toBe(false)
    expect(store.error).toBe('Este aparelho já é vigiado')
  })
})
