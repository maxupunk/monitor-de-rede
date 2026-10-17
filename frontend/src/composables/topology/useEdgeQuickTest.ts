import { onBeforeUnmount, ref } from 'vue'
import type { CanvasPointerEvent } from '@/utils/topology/canvasTypes'

export interface EdgeQuickTestState {
  edgeId: number
  x: number
  y: number
}

/** Atraso para aparecer (passar o mouse por cima do mapa não acende atalhos à toa). */
const SHOW_DELAY = 350
/** Tempo para alcançar o botão depois de sair do enlace. */
const HIDE_DELAY = 1200

/**
 * Estado do atalho "Testar velocidade" sobre o enlace sob o mouse. Os
 * atrasos são só de interface (aparecer/sumir), nenhum consulta a API.
 */
export function useEdgeQuickTest(isTestable: (edgeId: number) => boolean) {
  const state = ref<EdgeQuickTestState | null>(null)
  let showTimer: ReturnType<typeof setTimeout> | undefined
  let hideTimer: ReturnType<typeof setTimeout> | undefined

  function clearTimers() {
    clearTimeout(showTimer)
    clearTimeout(hideTimer)
  }

  function release() {
    clearTimeout(hideTimer)
    hideTimer = setTimeout(() => (state.value = null), HIDE_DELAY)
  }

  function onHover(event: CanvasPointerEvent | null) {
    if (event?.kind !== 'edge' || event.id === undefined || !isTestable(event.id)) {
      clearTimeout(showTimer)
      if (state.value) release()
      return
    }
    clearTimers()
    const next = { edgeId: event.id, x: event.x, y: event.y }
    if (state.value) state.value = next
    else showTimer = setTimeout(() => (state.value = next), SHOW_DELAY)
  }

  function hide() {
    clearTimers()
    state.value = null
  }

  onBeforeUnmount(clearTimers)

  return { state, onHover, hold: () => clearTimeout(hideTimer), release, hide }
}
