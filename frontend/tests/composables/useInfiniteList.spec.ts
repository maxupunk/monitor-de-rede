import { describe, it, expect, vi, beforeEach } from 'vitest'
import { useInfiniteList } from '../../src/composables/useInfiniteList.ts'

const getMock = vi.fn()
vi.mock('@/services/apiService', () => ({
  apiService: { get: (...args: unknown[]) => getMock(...args) },
}))

interface Item {
  id: number
}

describe('useInfiniteList', () => {
  beforeEach(() => {
    getMock.mockReset()
  })

  it('carrega a primeira página e empilha os itens', async () => {
    getMock.mockResolvedValueOnce({
      data: [{ id: 1 }, { id: 2 }],
      meta: { currentPage: 1, lastPage: 2, total: 4 },
    })

    const list = useInfiniteList<Item>(() => '/items')
    const done = vi.fn()

    await list.load({ done })

    expect(getMock).toHaveBeenCalledWith('/items?page=1&limit=20')
    expect(list.items.value).toEqual([{ id: 1 }, { id: 2 }])
    expect(list.total.value).toBe(4)
    expect(done).toHaveBeenCalledWith('ok')
  })

  it('avisa "empty" quando chega na última página', async () => {
    getMock
      .mockResolvedValueOnce({
        data: [{ id: 1 }, { id: 2 }],
        meta: { currentPage: 1, lastPage: 2, total: 3 },
      })
      .mockResolvedValueOnce({
        data: [{ id: 3 }],
        meta: { currentPage: 2, lastPage: 2, total: 3 },
      })

    const list = useInfiniteList<Item>(() => '/items')
    await list.load({ done: vi.fn() })
    const done = vi.fn()

    await list.load({ done })

    expect(getMock).toHaveBeenLastCalledWith('/items?page=2&limit=20')
    expect(done).toHaveBeenCalledWith('empty')
    expect(list.items.value).toEqual([{ id: 1 }, { id: 2 }, { id: 3 }])
  })

  it('continua funcionando quando a resposta não tem meta', async () => {
    getMock.mockResolvedValueOnce({
      data: [{ id: 1 }],
    })

    const list = useInfiniteList<Item>(() => '/items')
    const done = vi.fn()

    await list.load({ done })

    expect(done).toHaveBeenCalledWith('empty')
    expect(list.total.value).toBe(0)
  })

  it('reinicia a lista do começo', async () => {
    getMock
      .mockResolvedValueOnce({
        data: [{ id: 1 }],
        meta: { currentPage: 1, lastPage: 1, total: 1 },
      })
      .mockResolvedValueOnce({
        data: [{ id: 2 }],
        meta: { currentPage: 1, lastPage: 1, total: 1 },
      })

    const list = useInfiniteList<Item>(() => '/items')
    await list.load({ done: vi.fn() })
    list.reset()
    await list.load({ done: vi.fn() })

    expect(list.items.value).toEqual([{ id: 2 }])
    expect(list.scrollKey.value).toBe(1)
  })
})
