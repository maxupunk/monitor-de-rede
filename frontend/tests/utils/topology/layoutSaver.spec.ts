import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createLayoutSaver } from '@/utils/topology/layoutSaver'

describe('gravação do layout em lote (PATCH com debounce)', () => {
  beforeEach(() => vi.useFakeTimers())
  afterEach(() => vi.useRealTimers())

  it('junta vários arrastes num único envio depois do intervalo', async () => {
    const save = vi.fn().mockResolvedValue(undefined)
    const saver = createLayoutSaver({ save, delayMs: 800 })
    saver.queue({ 1: { x: 10.4, y: 20.6 } })
    await vi.advanceTimersByTimeAsync(500)
    saver.queue({ 2: { x: 30, y: 40 } })
    saver.queue({ 1: { x: 11, y: 21 } })
    await vi.advanceTimersByTimeAsync(799)
    expect(save).not.toHaveBeenCalled()
    await vi.advanceTimersByTimeAsync(1)
    expect(save).toHaveBeenCalledTimes(1)
    expect(save.mock.calls[0][0]).toEqual([
      { deviceId: 1, x: 11, y: 21 },
      { deviceId: 2, x: 30, y: 40 },
    ])
    expect(saver.pendingCount).toBe(0)
  })

  it('falha mantém as posições pendentes e avisa; "tentar de novo" reenvia', async () => {
    const save = vi.fn().mockRejectedValueOnce(new Error('offline')).mockResolvedValue(undefined)
    const onError = vi.fn()
    const onSaved = vi.fn()
    const saver = createLayoutSaver({ save, onError, onSaved, delayMs: 100 })
    saver.queue({ 7: { x: 1, y: 2 } })
    await vi.advanceTimersByTimeAsync(100)
    expect(onError).toHaveBeenCalledTimes(1)
    expect(saver.pendingCount).toBe(1)
    expect(await saver.flush()).toBe(true)
    expect(save).toHaveBeenLastCalledWith([{ deviceId: 7, x: 1, y: 2 }])
    expect(onSaved).toHaveBeenCalledTimes(1)
  })

  it('o que é movido durante o envio não é atropelado pela devolução do lote', async () => {
    let reject: (err: Error) => void = () => undefined
    const save = vi
      .fn()
      .mockImplementationOnce(() => new Promise((_, r) => (reject = r)))
      .mockResolvedValue(undefined)
    const saver = createLayoutSaver({ save, delayMs: 10 })
    saver.queue({ 1: { x: 0, y: 0 } })
    await vi.advanceTimersByTimeAsync(10)
    saver.queue({ 1: { x: 50, y: 50 } })
    reject(new Error('falhou'))
    await vi.advanceTimersByTimeAsync(0)
    await vi.advanceTimersByTimeAsync(10)
    expect(save).toHaveBeenLastCalledWith([{ deviceId: 1, x: 50, y: 50 }])
  })

  it('flush sem pendências não chama a API', async () => {
    const save = vi.fn()
    const saver = createLayoutSaver({ save })
    expect(await saver.flush()).toBe(true)
    expect(save).not.toHaveBeenCalled()
  })
})
