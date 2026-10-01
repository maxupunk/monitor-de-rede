import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { apiService } from '@/services/apiService'
import { usePluginsStore } from '@/stores/plugins'
import type { DevicePluginsView } from '@/bindings/DevicePluginsView'
import type { PluginRunView } from '@/bindings/PluginRunView'

function view(): DevicePluginsView {
  return {
    deviceId: 4,
    platform: 'openwrt',
    firmware: null,
    plugins: [],
    credentials: [],
    runs: [],
    agents: [],
  }
}

function run(overrides: Partial<PluginRunView> = {}): PluginRunView {
  return {
    id: 11,
    pluginId: 2,
    pluginName: 'OpenWrt – pacotes',
    deviceId: 4,
    action: 'detect',
    origin: 'user',
    status: 'running',
    params: {},
    output: null,
    transcript: [],
    error: null,
    createdAt: '2026-09-30T12:00:00Z',
    finishedAt: null,
    ...overrides,
  }
}

describe('plugins store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('execução acompanha o SSE sem consultar a API de novo', async () => {
    const get = vi.spyOn(apiService, 'get').mockResolvedValue(view())
    const store = usePluginsStore()
    await store.loadDevice(4)

    store.applyRunStarted({ run: run() })
    store.applyRunOutput({
      runId: 11,
      deviceId: 4,
      entry: {
        seq: 1,
        kind: 'ssh',
        request: 'cat /etc/openwrt_release',
        output: "DISTRIB_RELEASE='23.05.2'",
        durationMs: 12,
        origin: 'central',
        at: '2026-09-30T12:00:01Z',
      },
    })
    store.applyRunFinished({
      run: run({
        pluginName: null,
        status: 'succeeded',
        output: { firmware: '23.05.2' },
        finishedAt: '2026-09-30T12:00:02Z',
      }),
    })

    expect(get).toHaveBeenCalledOnce()
    const current = store.deviceViews[4]
    expect(current.runs).toHaveLength(1)
    expect(current.runs[0].status).toBe('succeeded')
    expect(current.runs[0].pluginName).toBe('OpenWrt – pacotes')
    expect(current.firmware).toBe('23.05.2')
  })

  it('pedido de aprovação chega pelo SSE e sai da fila ao ser resolvido', () => {
    const store = usePluginsStore()
    const pedido = {
      key: '11:1',
      runId: 11,
      deviceId: 4,
      deviceName: 'Roteador',
      pluginName: 'OpenWrt – pacotes',
      action: 'install_package',
      request: { transport: 'ssh', summary: 'opkg install luci', effect: 'write' },
      expiresAt: '2026-09-30T12:05:00Z',
    }
    store.applyApprovalRequired(pedido)
    store.applyApprovalRequired(pedido)
    expect(store.pendingApprovals).toHaveLength(1)
    expect(store.nextApproval?.request.effect).toBe('write')

    store.applyApprovalResolved({ key: '11:1', approved: true })
    expect(store.pendingApprovals).toHaveLength(0)
  })

  it('evento malformado é ignorado', async () => {
    vi.spyOn(apiService, 'get').mockResolvedValue(view())
    const store = usePluginsStore()
    await store.loadDevice(4)
    store.applyRunStarted({ run: { id: 'x' } })
    store.applyApprovalRequired({ key: 1 })
    expect(store.deviceViews[4].runs).toHaveLength(0)
    expect(store.pendingApprovals).toHaveLength(0)
  })
  it('instalar troca a visão e o resultado da lista chega pelo SSE', async () => {
    const instalado = { ...view(), firmware: '24.10.0' }
    const post = vi.spyOn(apiService, 'post').mockImplementation(async (path: string) => {
      if (path.endsWith('/install')) return instalado
      return { runId: 21 }
    })
    const get = vi.spyOn(apiService, 'get').mockResolvedValue(view())
    const store = usePluginsStore()
    await store.loadDevice(4)
    await store.installPlugin(4, 2)
    expect(store.deviceViews[4].firmware).toBe('24.10.0')

    const runId = await store.runAction(4, 2, 'list_packages', {}, false)
    expect(store.runById(4, runId)).toBeNull()
    store.applyRunStarted({ run: run({ id: 21, action: 'list_packages' }) })
    store.applyRunFinished({
      run: run({
        id: 21,
        action: 'list_packages',
        status: 'succeeded',
        output: [{ name: 'busybox', version: '1.36.1-1', installed: true }],
      }),
    })

    expect(store.runById(4, 21)?.output).toEqual([
      { name: 'busybox', version: '1.36.1-1', installed: true },
    ])
    expect(get).toHaveBeenCalledOnce()
    expect(post).toHaveBeenCalledTimes(2)
  })
})
