import { afterEach, describe, expect, it, vi } from 'vitest'
import { effectScope, nextTick, ref } from 'vue'
import { useElementWidth } from '@/composables/useElementWidth'

type Callback = (entries: Array<{ contentRect: { width: number } }>) => void

class FakeObserver {
  static last: FakeObserver | null = null
  disconnected = false
  constructor(public callback: Callback) {
    FakeObserver.last = this
  }
  observe() {}
  disconnect() {
    this.disconnected = true
  }
}

afterEach(() => vi.unstubAllGlobals())

describe('useElementWidth', () => {
  it('acompanha a largura pelo ResizeObserver e solta o observador ao trocar o elemento', async () => {
    vi.stubGlobal('ResizeObserver', FakeObserver)
    const el = ref<HTMLElement | null>(null)
    const scope = effectScope()
    const width = scope.run(() => useElementWidth(el))!
    expect(width.value).toBe(0)

    const div = document.createElement('div')
    Object.defineProperty(div, 'clientWidth', { value: 945 })
    el.value = div
    await nextTick()
    expect(width.value).toBe(945)

    const first = FakeObserver.last!
    first.callback([{ contentRect: { width: 1185 } }])
    expect(width.value).toBe(1185)

    el.value = null
    await nextTick()
    expect(first.disconnected).toBe(true)
    scope.stop()
  })
})
