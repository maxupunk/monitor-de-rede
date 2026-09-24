import { describe, expect, it } from 'vitest'
import { nextTick, ref } from 'vue'
import { useChatAutoScroll } from '@/composables/useChatAutoScroll'

/** Elemento rolável falso: 1000px de conteúdo numa janela de 300px. */
function fakeScroller() {
  const element = {
    scrollHeight: 1000,
    clientHeight: 300,
    scrollTop: 700,
    scrollTo({ top }: { top: number }) {
      element.scrollTop = Math.min(top, element.scrollHeight - element.clientHeight)
    },
  }
  return element
}

async function flush() {
  await nextTick()
  await nextTick()
}

describe('useChatAutoScroll', () => {
  it('acompanha o fim enquanto quem lê está nele', async () => {
    const element = fakeScroller()
    const count = ref(1)
    const text = ref('a')
    const { onScroll, showJump } = useChatAutoScroll(
      ref(element as unknown as HTMLElement),
      () => count.value,
      () => text.value
    )
    onScroll()
    element.scrollHeight = 1400
    text.value = 'ab'
    await flush()
    expect(element.scrollTop).toBe(1100)
    expect(showJump.value).toBe(false)
  })

  it('quem rolou para cima não é puxado para baixo: aparece o botão', async () => {
    const element = fakeScroller()
    const count = ref(1)
    const text = ref('a')
    const { onScroll, showJump, scrollToBottom } = useChatAutoScroll(
      ref(element as unknown as HTMLElement),
      () => count.value,
      () => text.value
    )
    element.scrollTop = 100
    onScroll()
    expect(showJump.value).toBe(true)

    element.scrollHeight = 1400
    text.value = 'ab'
    await flush()
    expect(element.scrollTop).toBe(100)

    scrollToBottom()
    await flush()
    expect(element.scrollTop).toBe(1100)
    expect(showJump.value).toBe(false)
  })

  it('mensagem nova sempre leva ao fim', async () => {
    const element = fakeScroller()
    const count = ref(1)
    const { onScroll } = useChatAutoScroll(
      ref(element as unknown as HTMLElement),
      () => count.value,
      () => null
    )
    element.scrollTop = 0
    onScroll()
    count.value = 2
    await flush()
    expect(element.scrollTop).toBe(700)
  })
})
