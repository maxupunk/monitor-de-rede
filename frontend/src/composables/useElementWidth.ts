import { onScopeDispose, ref, watch, type Ref } from 'vue'

/**
 * Largura atual de um elemento, por `ResizeObserver` (evento, não polling).
 * Serve a quem precisa decidir pelo espaço que tem — o menu lateral aberto
 * tira largura da página sem mudar a janela, e os pontos de quebra do
 * Vuetify só olham a janela. `0` até o elemento existir.
 */
export function useElementWidth(target: Ref<HTMLElement | null>): Ref<number> {
  const width = ref(0)
  let observer: ResizeObserver | null = null

  function disconnect() {
    observer?.disconnect()
    observer = null
  }

  watch(
    target,
    (el) => {
      disconnect()
      if (!el || typeof ResizeObserver === 'undefined') return
      width.value = el.clientWidth
      observer = new ResizeObserver((entries) => {
        width.value = entries[0]?.contentRect.width ?? el.clientWidth
      })
      observer.observe(el)
    },
    { immediate: true, flush: 'post' }
  )
  onScopeDispose(disconnect)

  return width
}
