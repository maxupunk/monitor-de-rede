import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { apiService } from '@/services/apiService'
import { usePluginAppsStore } from '@/stores/pluginApps'
import type { FleetView } from '@/bindings/FleetView'
import type { PluginBatchView } from '@/bindings/PluginBatchView'

function fleet(): FleetView {
  return {
    plugin: {
      id: 3,
      slug: 'openwrt-wifi',
      name: 'Rede Wi-Fi (OpenWrt)',
      version: '1.0.0',
      description: null,
      scope: 'model',
      deviceId: null,
      source: 'builtin',
      status: 'active',
      transports: ['ssh'],
      actions: [],
      matcher: {},
      list: null,
      surfaces: ['device', 'fleet'],
      settings: null,
      fleet: { title: 'Rede Wi-Fi', statusAction: 'status' },
      wifi: true,
      risk: null,
      lastTestAt: null,
      lastTestOk: null,
      validatedCount: 0,
      updatedAt: '2026-10-01T12:00:00Z',
    },
    settings: {},
    members: [],
    candidates: [],
    batches: [],
  }
}

function batch(overrides: Partial<PluginBatchView> = {}): PluginBatchView {
  return {
    id: 21,
    pluginId: 3,
    action: 'status',
    title: 'Atualizar estado',
    status: 'running',
    devices: [
      {
        deviceId: 7,
        deviceName: 'AP Loja',
        runId: null,
        status: 'pending',
        error: null,
        output: null,
      },
    ],
    result: null,
    hasPatch: false,
    error: null,
    createdAt: '2026-10-01T12:00:00Z',
    finishedAt: null,
    ...overrides,
  }
}

describe('pluginApps store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('lote acompanha o SSE sem consultar a API de novo', async () => {
    const get = vi.spyOn(apiService, 'get').mockResolvedValue(fleet())
    const store = usePluginAppsStore()
    await store.loadFleet(3)
    expect(get).toHaveBeenCalledTimes(1)

    store.applyBatchUpdated({ batch: batch() })
    expect(store.batchById(3, 21)?.status).toBe('running')
    expect(store.latestBatch(3, 'status')).toBeNull()

    store.applyBatchUpdated({
      batch: batch({
        status: 'succeeded',
        finishedAt: '2026-10-01T12:00:05Z',
        devices: [
          {
            deviceId: 7,
            deviceName: 'AP Loja',
            runId: 40,
            status: 'succeeded',
            error: null,
            output: { state: 'sincronizado' },
          },
        ],
      }),
    })

    expect(store.fleets[3]?.batches).toHaveLength(1)
    expect(store.latestBatch(3, 'status')?.devices[0]?.output).toEqual({ state: 'sincronizado' })
    expect(get).toHaveBeenCalledTimes(1)
  })

  it('parâmetros de cada equipamento vão junto com os comuns', async () => {
    const post = vi.spyOn(apiService, 'post').mockResolvedValue({ batchId: 30 })
    const store = usePluginAppsStore()
    const id = await store.runAction(
      3,
      'radios',
      [7, 8],
      true,
      {},
      { 7: { radio_2g_channel: '6' } }
    )
    expect(id).toBe(30)
    expect(post).toHaveBeenCalledWith('/plugins/3/fleet/actions/radios', {
      deviceIds: [7, 8],
      params: {},
      confirmWrite: true,
      deviceParams: { 7: { radio_2g_channel: '6' } },
    })
  })

  it('lote lançado daqui roda de novo num equipamento com os mesmos valores, uma vez', async () => {
    const post = vi
      .spyOn(apiService, 'post')
      .mockResolvedValueOnce({ batchId: 30 })
      .mockResolvedValueOnce({ batchId: 31 })
    const store = usePluginAppsStore()
    await store.runAction(
      3,
      'set_wifi',
      [7, 8],
      true,
      { ssid: 'Loja', key: 'segredo' },
      { 7: { channel: '6' }, 8: { channel: '11' } }
    )
    expect(store.canRerun(30, 7)).toBe(true)
    // Os valores (com a senha) ficam fora do estado da store.
    expect(JSON.stringify(store.$state)).not.toContain('segredo')

    await expect(store.rerunDevice(30, 7, true)).resolves.toBe(31)
    expect(post).toHaveBeenLastCalledWith('/plugins/3/fleet/actions/set_wifi', {
      deviceIds: [7],
      params: { ssid: 'Loja', key: 'segredo' },
      confirmWrite: true,
      deviceParams: { 7: { channel: '6' } },
    })
    // O lote antigo esquece o equipamento; o novo guarda os seus.
    expect(store.canRerun(30, 7)).toBe(false)
    expect(store.canRerun(30, 8)).toBe(true)
    expect(store.canRerun(31, 7)).toBe(true)
    await expect(store.rerunDevice(30, 7, true)).resolves.toBeNull()
    expect(post).toHaveBeenCalledTimes(2)
  })

  it('sem os valores (outra aba, página recarregada, página fechada) não roda de novo', async () => {
    vi.spyOn(apiService, 'post').mockResolvedValue({ batchId: 30 })
    const store = usePluginAppsStore()
    expect(store.canRerun(30, 7)).toBe(false)
    await expect(store.rerunDevice(30, 7, false)).resolves.toBeNull()

    await store.runAction(3, 'set_wifi', [7], true, { ssid: 'Loja' })
    expect(store.canRerun(30, 7)).toBe(true)
    store.forgetLaunched(4)
    expect(store.canRerun(30, 7)).toBe(true)
    store.forgetLaunched(3)
    expect(store.canRerun(30, 7)).toBe(false)
  })

  it('lote que sai da lista leva junto os valores guardados', async () => {
    vi.spyOn(apiService, 'get').mockResolvedValue(fleet())
    vi.spyOn(apiService, 'post').mockResolvedValue({ batchId: 1 })
    const store = usePluginAppsStore()
    await store.loadFleet(3)
    await store.runAction(3, 'set_wifi', [7], true, { ssid: 'Loja' })
    store.applyBatchUpdated({ batch: batch({ id: 1 }) })
    for (let id = 2; id <= 16; id += 1) store.applyBatchUpdated({ batch: batch({ id }) })
    expect(store.batchById(3, 1)).toBeNull()
    expect(store.canRerun(1, 7)).toBe(false)
  })

  it('ignora lote de aplicativo não aberto e evento malformado', () => {
    const store = usePluginAppsStore()
    store.applyBatchUpdated({ batch: batch() })
    store.applyBatchUpdated({ batch: { id: 'x' } })
    expect(store.fleets).toEqual({})
  })
})
