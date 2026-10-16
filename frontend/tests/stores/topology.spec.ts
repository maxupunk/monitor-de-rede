import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { ApiError, apiService } from '@/services/apiService'
import { parsePluginActionMap, useTopologyStore } from '@/stores/topology'
import { useLinkTestRequestsStore } from '@/stores/linkTestRequests'
import { edge, node } from '../utils/topology/fixtures'

describe('store da topologia', () => {
  beforeEach(() => setActivePinia(createPinia()))
  afterEach(() => vi.restoreAllMocks())

  it('carrega o grafo no site pedido e normaliza ip/isSystem', async () => {
    const get = vi.spyOn(apiService, 'get').mockResolvedValue({
      nodes: [{ ...node(1), ip: '10.0.0.1', status: '' }],
      edges: [{ ...edge(5, 1, 2), sourceDeviceId: undefined, inBps: 1500 }],
    })
    const store = useTopologyStore()
    await store.fetchTopology(4)
    expect(get).toHaveBeenCalledWith('/topology?siteId=4')
    expect(store.siteId).toBe(4)
    expect(store.nodes[0].ipAddress).toBe('10.0.0.1')
    expect(store.nodes[0].status).toBe('unknown')
    expect(store.nodes[0].isSystem).toBe(false)
    expect(store.edges[0].sourceDeviceId).toBe(1)
    expect(store.edges[0].inBpsLabel).toBe('1.5 Kbps')
    expect(store.loaded).toBe(true)
    expect(store.revision).toBe(1)
  })

  it('recarga por SSE só acontece para quem já abriu o mapa', async () => {
    const get = vi.spyOn(apiService, 'get').mockResolvedValue({ nodes: [], edges: [] })
    const store = useTopologyStore()
    store.refreshIfLoaded()
    expect(get).not.toHaveBeenCalled()
    await store.fetchTopology(2)
    store.refreshIfLoaded()
    expect(get).toHaveBeenLastCalledWith('/topology?siteId=2')
  })

  it('aplica status, tráfego e mudança de interface em memória, sem consultar a API', () => {
    const get = vi.spyOn(apiService, 'get')
    const store = useTopologyStore()
    store.nodes = [node(1), node(2)]
    store.edges = [
      edge(5, 1, 2, {
        sourceInterfaceId: 10,
        targetInterfaceId: 20,
        sourceInterfaceSpeed: 100_000_000,
      }),
    ]
    store.applyRealtimeStatus({ id: 2, status: 'offline' })
    expect(store.nodes[1].status).toBe('offline')

    store.applyRealtimeTraffic({
      deviceId: 1,
      interfaces: [
        { id: 10, name: 'ge-0/0/1', inBps: 80_000_000, outBps: 5_000_000, operStatus: 'up' },
      ],
    })
    expect(store.edges[0].inBps).toBe(80_000_000)
    expect(store.edges[0].trafficLabel).toBe('85 Mbps')
    expect(store.edges[0].utilization).toBeCloseTo(0.8)
    expect(store.edges[0].status).toBe('up')

    store.applyInterfaceChange('interface:status_change', {
      interfaceId: 20,
      currentStatus: 'down',
    })
    expect(store.edges[0].targetInterfaceStatus).toBe('down')
    expect(store.edges[0].status).toBe('down')
    expect(get).not.toHaveBeenCalled()
  })

  it('grava só as posições movidas (PATCH com o site) e mescla no layout local', async () => {
    const patch = vi.spyOn(apiService, 'patch').mockResolvedValue({})
    const store = useTopologyStore()
    store.siteId = 3
    store.layout = { 1: { x: 0, y: 0 } }
    await store.patchLayout([{ deviceId: 2, x: 10, y: 20 }])
    expect(patch).toHaveBeenCalledWith('/topology/layout', {
      siteId: 3,
      nodes: [{ deviceId: 2, x: 10, y: 20 }],
    })
    expect(store.layout).toEqual({ 1: { x: 0, y: 0 }, 2: { x: 10, y: 20 } })
  })

  it('sem PATCH no backend (405), cai no PUT com o layout inteiro', async () => {
    vi.spyOn(apiService, 'patch').mockRejectedValue(new ApiError('Method Not Allowed', 405))
    const put = vi.spyOn(apiService, 'put').mockResolvedValue({})
    const store = useTopologyStore()
    store.layout = { 1: { x: 0, y: 0 } }
    await store.patchLayout([{ deviceId: 2, x: 10, y: 20 }])
    expect(put).toHaveBeenCalledWith('/topology/layout', {
      nodes: [
        { deviceId: 1, x: 0, y: 0 },
        { deviceId: 2, x: 10, y: 20 },
      ],
    })
  })

  it('outra falha de gravação sobe para quem chamou (o aviso oferece nova tentativa)', async () => {
    vi.spyOn(apiService, 'patch').mockRejectedValue(new ApiError('boom', 500))
    const store = useTopologyStore()
    await expect(store.patchLayout([{ deviceId: 1, x: 1, y: 1 }])).rejects.toThrow('boom')
  })

  it('layout salvo em outra aba: só conta se for do site em exibição', () => {
    const store = useTopologyStore()
    store.siteId = 2
    store.applyLayoutUpdated({ siteId: 5 })
    expect(store.remoteLayoutRevision).toBe(0)
    store.applyLayoutUpdated({ siteId: 2 })
    expect(store.remoteLayoutRevision).toBe(1)
    store.siteId = null
    store.applyLayoutUpdated({ siteId: null })
    expect(store.remoteLayoutRevision).toBe(2)
  })

  it('confirmar enlace regrava os mesmos dados (vira do operador)', async () => {
    const put = vi.spyOn(apiService, 'put').mockResolvedValue({})
    vi.spyOn(apiService, 'get').mockResolvedValue({ nodes: [], edges: [] })
    const store = useTopologyStore()
    const ok = await store.confirmLink(
      edge(9, 1, 2, { linkType: 'lldp', sourceInterfaceId: 3, targetInterfaceId: null })
    )
    expect(ok).toBe(true)
    expect(put).toHaveBeenCalledWith('/topology/links/9', {
      sourceInterfaceId: 3,
      targetInterfaceId: null,
      linkType: 'lldp',
    })
  })

  it('erro de ação fica em actionError (os diálogos mostram)', async () => {
    vi.spyOn(apiService, 'post').mockRejectedValue(new Error('Portas já ocupadas'))
    const store = useTopologyStore()
    expect(await store.addLink({ sourceDeviceId: 1, targetDeviceId: 2 })).toBe(false)
    expect(store.actionError).toBe('Portas já ocupadas')
  })

  it('valida o mapa de ações de plugin', () => {
    expect(
      parsePluginActionMap({
        '5': [
          { pluginId: 1, action: 'reboot', title: 'Reiniciar', effect: 'write', hasParams: 0 },
          { lixo: true },
        ],
        x: [],
      })
    ).toEqual({
      5: [
        {
          pluginId: 1,
          pluginSlug: '',
          pluginName: '',
          action: 'reboot',
          title: 'Reiniciar',
          effect: 'write',
          hasParams: false,
        },
      ],
    })
    expect(parsePluginActionMap(null)).toEqual({})
  })
})

describe('pedidos de teste de enlace', () => {
  beforeEach(() => setActivePinia(createPinia()))

  it('registra o pedido; o assistente assume com take()', () => {
    const store = useLinkTestRequestsStore()
    const first = store.requestLinkTest({ kind: 'link', linkId: 4 })
    const second = store.requestLinkTest({ kind: 'server', targetDeviceId: 9 })
    expect(second.seq).toBe(first.seq + 1)
    expect(store.history).toHaveLength(2)
    expect(store.take()).toMatchObject({ kind: 'server', targetDeviceId: 9 })
    expect(store.pending).toBeNull()
  })
})
