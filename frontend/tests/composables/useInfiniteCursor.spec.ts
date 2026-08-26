import { describe, it, expect, vi, beforeEach } from 'vitest'
import { useInfiniteCursor } from '../../src/composables/useInfiniteCursor.ts'

const getMock = vi.fn()
vi.mock('@/services/apiService', () => ({
  apiService: { get: (...args: unknown[]) => getMock(...args) },
}))

interface Log {
  id: string
  message: string
}

describe('useInfiniteCursor', () => {
  beforeEach(() => {
    getMock.mockReset()
  })

  it('carrega a primeira página e avisa "ok" quando há mais dados', async () => {
    getMock.mockResolvedValueOnce({
      data: [{ id: 'a', message: 'one' }],
      meta: { nextCursor: 'cursor-1', hasMore: true, limit: 50, from: 'now-1h', to: 'now' },
    })

    const list = useInfiniteCursor<Log>(() => '/logs')
    const done = vi.fn()

    await list.load({ done })

    expect(getMock).toHaveBeenCalledWith('/logs?limit=50')
    expect(list.items.value).toEqual([{ id: 'a', message: 'one' }])
    expect(list.window.value).toEqual({ from: 'now-1h', to: 'now' })
    expect(done).toHaveBeenCalledWith('ok')
  })

  it('usa o cursor na próxima requisição e avisa "empty" no fim', async () => {
    getMock
      .mockResolvedValueOnce({
        data: [{ id: 'a', message: 'one' }],
        meta: { nextCursor: 'cursor-1', hasMore: true, limit: 50, from: 'now-2h', to: 'now-1h' },
      })
      .mockResolvedValueOnce({
        data: [{ id: 'b', message: 'two' }],
        meta: { nextCursor: null, hasMore: false, limit: 50, from: 'now-3h', to: 'now-2h' },
      })

    const list = useInfiniteCursor<Log>(() => '/logs')
    await list.load({ done: vi.fn() })
    const done = vi.fn()

    await list.load({ done })

    expect(getMock).toHaveBeenLastCalledWith('/logs?limit=50&cursor=cursor-1')
    expect(list.items.value).toEqual([
      { id: 'a', message: 'one' },
      { id: 'b', message: 'two' },
    ])
    expect(done).toHaveBeenCalledWith('empty')
  })

  it('evita requisições concorrentes com o mesmo cursor', async () => {
    let resolve: (value: unknown) => void = () => {}
    getMock.mockImplementation(
      () =>
        new Promise((res) => {
          resolve = res
        })
    )

    const list = useInfiniteCursor<Log>(() => '/logs')
    const first = list.load({ done: vi.fn() })
    const second = list.load({ done: vi.fn() })

    resolve({
      data: [{ id: 'a', message: 'one' }],
      meta: { nextCursor: null, hasMore: false, limit: 50, from: '', to: '' },
    })

    await Promise.all([first, second])

    expect(getMock).toHaveBeenCalledTimes(1)
  })

  it('reinicia a lista do começo', async () => {
    getMock.mockResolvedValue({
      data: [{ id: 'a', message: 'one' }],
      meta: { nextCursor: null, hasMore: false, limit: 50, from: '', to: '' },
    })

    const list = useInfiniteCursor<Log>(() => '/logs')
    await list.load({ done: vi.fn() })
    list.reset()
    await list.load({ done: vi.fn() })

    expect(list.items.value).toEqual([{ id: 'a', message: 'one' }])
    expect(list.scrollKey.value).toBe(1)
  })

  it('ignora a resposta anterior quando o filtro muda durante o carregamento', async () => {
    let resolveOld: (value: unknown) => void = () => {}
    let resolveCurrent: (value: unknown) => void = () => {}
    getMock
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolveOld = resolve
          })
      )
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolveCurrent = resolve
          })
      )

    let endpoint = '/logs?deviceId=1'
    const list = useInfiniteCursor<Log>(() => endpoint)
    const oldLoad = list.load({ done: vi.fn() })

    endpoint = '/logs?deviceId=2'
    list.reset()
    const currentLoad = list.load({ done: vi.fn() })

    resolveCurrent({
      data: [{ id: 'device-2', message: 'current' }],
      meta: { nextCursor: null, hasMore: false, limit: 50, from: '', to: '' },
    })
    await currentLoad

    resolveOld({
      data: [{ id: 'device-1', message: 'stale' }],
      meta: { nextCursor: null, hasMore: false, limit: 50, from: '', to: '' },
    })
    await oldLoad

    expect(getMock).toHaveBeenNthCalledWith(1, '/logs?deviceId=1&limit=50')
    expect(getMock).toHaveBeenNthCalledWith(2, '/logs?deviceId=2&limit=50')
    expect(list.items.value).toEqual([{ id: 'device-2', message: 'current' }])
  })

  it('empilha novas entradas no topo sem duplicar', () => {
    const list = useInfiniteCursor<Log>(() => '/logs')
    list.items.value = [
      { id: 'a', message: 'one' },
      { id: 'b', message: 'two' },
    ]

    list.prepend(
      [
        { id: 'c', message: 'three' },
        { id: 'a', message: 'one' },
      ],
      (entry) => entry.id
    )

    expect(list.items.value).toEqual([
      { id: 'c', message: 'three' },
      { id: 'a', message: 'one' },
      { id: 'b', message: 'two' },
    ])
  })
})
