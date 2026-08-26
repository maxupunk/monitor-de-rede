import { afterEach, describe, expect, it, vi } from 'vitest'
import { apiService, NetworkError } from '@/services/apiService'

describe('timeout individual da API', () => {
  afterEach(() => {
    vi.useRealTimers()
    vi.unstubAllGlobals()
  })

  it('não aplica o teto global de 15 segundos a uma operação longa declarada', async () => {
    vi.useFakeTimers()
    vi.stubGlobal('localStorage', {
      getItem: vi.fn(() => null),
      removeItem: vi.fn(),
      setItem: vi.fn(),
    })
    let signal: AbortSignal | null = null
    vi.stubGlobal(
      'fetch',
      vi.fn((_input: RequestInfo | URL, init?: RequestInit) => {
        signal = init?.signal ?? null
        return new Promise<Response>((_resolve, reject) => {
          signal?.addEventListener('abort', () => {
            reject(Object.assign(new Error('Abortado'), { name: 'AbortError' }))
          })
        })
      })
    )

    const request = apiService.post('/operacao-longa', {}, { timeoutMs: 65_000 })
    await vi.advanceTimersByTimeAsync(15_000)
    expect(signal?.aborted).toBe(false)

    const rejection = expect(request).rejects.toBeInstanceOf(NetworkError)
    await vi.advanceTimersByTimeAsync(50_000)
    await rejection
  })
})
