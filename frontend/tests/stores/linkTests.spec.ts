import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { ApiError, apiService } from '@/services/apiService'
import { SAMPLE_LIMIT, useLinkTestsStore } from '@/stores/linkTests'
import { leg, plan, test } from '../utils/linkTests/fixtures'

describe('store do teste de enlace', () => {
  beforeEach(() => setActivePinia(createPinia()))
  afterEach(() => vi.restoreAllMocks())

  it('plano: POST sem o confirm', async () => {
    const post = vi.spyOn(apiService, 'post').mockResolvedValue(plan())
    const store = useLinkTestsStore()
    const result = await store.plan({ kind: 'link', linkId: 4, confirm: true })
    expect(result.legs).toHaveLength(1)
    expect(post).toHaveBeenCalledWith(
      '/link-tests/plan',
      { kind: 'link', linkId: 4 },
      {
        timeoutMs: 30_000,
      }
    )
  })

  it('iniciar: POST com confirm: true e devolve o id', async () => {
    const post = vi.spyOn(apiService, 'post').mockResolvedValue({ id: 42 })
    const store = useLinkTestsStore()
    expect(await store.start({ kind: 'server', targetDeviceId: 9 })).toBe(42)
    expect(post).toHaveBeenCalledWith(
      '/link-tests',
      { kind: 'server', targetDeviceId: 9, confirm: true },
      { timeoutMs: 30_000 }
    )
  })

  it('iniciar: resposta com o teste inteiro já entra no cache', async () => {
    vi.spyOn(apiService, 'post').mockResolvedValue(test(7, { status: 'queued' }))
    const store = useLinkTestsStore()
    expect(await store.start({ kind: 'link', linkId: 101 })).toBe(7)
    expect(store.active.map((item) => item.id)).toEqual([7])
  })

  it('cancelar: POST; teste já terminado (404/409) não é erro', async () => {
    const post = vi.spyOn(apiService, 'post').mockResolvedValueOnce({})
    const store = useLinkTestsStore()
    await store.cancel(3)
    expect(post).toHaveBeenCalledWith('/link-tests/3/cancel', {})
    post.mockRejectedValueOnce(new ApiError('Já terminou', 409))
    await expect(store.cancel(3)).resolves.toBeUndefined()
    post.mockRejectedValueOnce(new ApiError('Sem permissão', 403))
    await expect(store.cancel(3)).rejects.toThrow('Sem permissão')
  })

  it('histórico: filtros viram query e os itens entram no cache', async () => {
    const get = vi.spyOn(apiService, 'get').mockResolvedValue({
      items: [test(1, { status: 'succeeded' }), test(2, { status: 'failed' })],
      total: 2,
      page: 1,
      perPage: 20,
    })
    const store = useLinkTestsStore()
    await store.fetchPage({ page: 1, deviceId: 5, status: 'failed', kind: 'path' })
    expect(get).toHaveBeenCalledWith(
      '/link-tests?page=1&perPage=20&deviceId=5&status=failed&kind=path'
    )
    expect(store.page?.total).toBe(2)
    expect(store.byId(2)?.status).toBe('failed')
  })

  it('detalhe: semeia as amostras gravadas e guarda o teste sem elas', async () => {
    const detail = test(5, {
      status: 'succeeded',
      legs: [
        leg(1, {
          status: 'succeeded',
          samples: [
            { flow: 'forward', bps: 2, tMs: 2000 },
            { flow: 'forward', bps: 1, tMs: 1000 },
            { flow: 'reverse', bps: 3, tMs: 1000 },
          ],
          health: [
            { deviceId: 2, atMs: 2000, cpu: 80, memory: 50 },
            { deviceId: 2, atMs: 1000, cpu: 20, memory: 50 },
          ],
        }),
      ],
    })
    vi.spyOn(apiService, 'get').mockResolvedValue(detail)
    const store = useLinkTestsStore()
    await store.fetchDetail(5)
    expect(store.samplesFor(5, 1, 'forward').map((s) => s.bps)).toEqual([1, 2])
    expect(store.samplesFor(5, 1, 'reverse')).toHaveLength(1)
    expect(store.byId(5)?.legs[0].samples).toBeUndefined()
    // CPU/RAM gravados semeiam o gráfico, em ordem; o cache fica sem eles.
    expect(store.healthFor(5, 1).map((h) => h.cpu)).toEqual([20, 80])
    expect(store.byId(5)?.legs[0].health).toBeUndefined()
  })

  it('SSE: link_test:updated atualiza o estado SEM consultar a API de novo', () => {
    const get = vi.spyOn(apiService, 'get')
    const post = vi.spyOn(apiService, 'post')
    const store = useLinkTestsStore()

    store.applyUpdated(test(9, { status: 'running' }))
    expect(store.active.map((item) => item.id)).toEqual([9])

    store.applyUpdated(
      test(9, {
        status: 'succeeded',
        legs: [leg(1, { status: 'succeeded', forwardBps: 9e8 })],
      })
    )
    expect(store.active).toEqual([])
    expect(store.byId(9)?.legs[0].forwardBps).toBe(9e8)
    expect(store.latestForEdge(101)?.id).toBe(9)
    expect(store.latestForDevice(2)?.id).toBe(9)
    expect(get).not.toHaveBeenCalled()
    expect(post).not.toHaveBeenCalled()
  })

  it('SSE: snapshot atrasado não reabre teste já terminado; lixo é ignorado', () => {
    const store = useLinkTestsStore()
    store.applyUpdated(test(1, { status: 'cancelled' }))
    store.applyUpdated(test(1, { status: 'running' }))
    expect(store.byId(1)?.status).toBe('cancelled')
    store.applyUpdated({ id: 'x' })
    store.applyUpdated(null)
    expect(Object.keys(store.tests)).toEqual(['1'])
  })

  it('SSE: teste novo entra na primeira página do histórico se casar com o filtro', async () => {
    vi.spyOn(apiService, 'get').mockResolvedValue({ items: [], total: 0, page: 1, perPage: 20 })
    const store = useLinkTestsStore()
    await store.fetchPage({ page: 1, kind: 'link' })
    store.applyUpdated(test(3, { kind: 'path' }))
    expect(store.page?.items).toHaveLength(0)
    store.applyUpdated(test(4))
    expect(store.page?.items.map((item) => item.id)).toEqual([4])
    expect(store.page?.total).toBe(1)
    store.applyUpdated(test(4, { status: 'succeeded' }))
    expect(store.page?.items[0].status).toBe('succeeded')
    expect(store.page?.total).toBe(1)
  })

  it('SSE: amostras num anel limitado por perna e sentido, sem repetidas', () => {
    const store = useLinkTestsStore()
    for (let t = 1; t <= SAMPLE_LIMIT + 30; t += 1) {
      store.applySample({ testId: 1, legSeq: 1, flow: 'forward', bps: t * 1000, tMs: t * 1000 })
    }
    store.applySample({ testId: 1, legSeq: 1, flow: 'forward', bps: 1, tMs: 5000 })
    store.applySample({ testId: 1, legSeq: 1, flow: 'sideways', bps: 1, tMs: 999_999 })
    const list = store.samplesFor(1, 1, 'forward')
    expect(list).toHaveLength(SAMPLE_LIMIT)
    expect(list[list.length - 1].bps).toBe((SAMPLE_LIMIT + 30) * 1000)
    expect(store.liveBps(1, 1, 'forward')).toBe((SAMPLE_LIMIT + 30) * 1000)
    expect(store.liveBps(1, 1, 'reverse')).toBeNull()
    expect(store.lastActivityAt(1)).not.toBeNull()
  })

  it('agendamento atualizado: relê só se a lista já foi aberta', async () => {
    const get = vi.spyOn(apiService, 'get').mockResolvedValue([])
    const store = useLinkTestsStore()
    store.applyScheduleUpdated({ id: 1 })
    expect(get).not.toHaveBeenCalled()
    await store.fetchSchedules()
    store.applyScheduleUpdated({ id: 1 })
    expect(get).toHaveBeenCalledTimes(2)
  })

  it('agendamentos: CRUD e "rodar agora" com recuo para teste avulso', async () => {
    const saved = {
      id: 3,
      name: 'Torre',
      kind: 'link' as const,
      linkId: 101,
      sourceDeviceId: 1,
      targetDeviceId: 2,
      spec: { durationS: 5, direction: 'both' as const, streams: 1, limitMbps: null },
      intervalMinutes: 60,
      windowStart: null,
      windowEnd: null,
      enabled: true,
      lastRunAt: null,
      nextRunAt: null,
      lastTestId: null,
      lastStatus: null,
      createdAt: '2026-10-16T12:00:00Z',
      updatedAt: '2026-10-16T12:00:00Z',
    }
    const post = vi.spyOn(apiService, 'post').mockResolvedValueOnce(saved)
    const put = vi.spyOn(apiService, 'put').mockResolvedValue({ ...saved, enabled: false })
    const del = vi.spyOn(apiService, 'delete').mockResolvedValue({})
    const store = useLinkTestsStore()

    await store.createSchedule({ ...saved })
    expect(store.schedules).toHaveLength(1)
    await store.updateSchedule(3, { ...saved, enabled: false })
    expect(put).toHaveBeenCalledWith('/link-test-schedules/3', expect.anything())
    expect(store.schedules[0].enabled).toBe(false)

    post.mockResolvedValueOnce({ id: 50 })
    expect(await store.runScheduleNow(3)).toBe(50)
    expect(post).toHaveBeenLastCalledWith('/link-test-schedules/3/run', {})

    post.mockRejectedValueOnce(new ApiError('Not Found', 404)).mockResolvedValueOnce({ id: 51 })
    expect(await store.runScheduleNow(3)).toBe(51)
    expect(post).toHaveBeenLastCalledWith(
      '/link-tests',
      expect.objectContaining({ kind: 'link', linkId: 101, confirm: true }),
      { timeoutMs: 30_000 }
    )

    await store.deleteSchedule(3)
    expect(del).toHaveBeenCalledWith('/link-test-schedules/3')
    expect(store.schedules).toEqual([])
  })

  it('foco: o teste acompanhado na tela', () => {
    const store = useLinkTestsStore()
    store.applyUpdated(test(2))
    store.focus(2)
    expect(store.focused?.id).toBe(2)
    store.focus(null)
    expect(store.focused).toBeNull()
  })
})
