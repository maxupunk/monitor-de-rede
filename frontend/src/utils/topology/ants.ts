/**
 * "Formigas" do enlace em teste: o tracejado das arestas `.ov-running` anda no
 * sentido medido (`flow-reverse` anda ao contrário). É animação, não consulta:
 * um `requestAnimationFrame` só enquanto houver aresta em teste.
 *
 * Usado pelo mapa (`TopologyCanvas`) e pelo mini mapa do teste
 * (`LinkTestPathMap`) — o mesmo movimento nos dois.
 */
import type { Core } from 'cytoscape'

export interface AntsAnimator {
  /** Liga ou desliga conforme haja aresta em teste (chame a cada mudança). */
  update(): void
  /** Para de vez (o canvas foi destruído). */
  stop(): void
}

/** Quanto o tracejado anda por quadro. */
const STEP = 1.2

export function antsAnimator(coreOf: () => Core | null): AntsAnimator {
  let frame = 0
  let offset = 0
  let stopped = false

  const step = () => {
    const core = coreOf()
    if (!core || stopped) {
      frame = 0
      return
    }
    const edges = core.edges('.ov-running')
    if (edges.empty()) {
      frame = 0
      return
    }
    offset = (offset + STEP) % 1000
    core.batch(() => {
      edges.forEach((edge) => {
        edge.style('line-dash-offset', edge.hasClass('flow-reverse') ? offset : -offset)
      })
    })
    frame = requestAnimationFrame(step)
  }

  return {
    update() {
      const core = coreOf()
      if (!core || stopped) return
      if (core.edges('.ov-running').empty()) {
        if (frame) cancelAnimationFrame(frame)
        frame = 0
        core.edges().removeStyle('line-dash-offset')
        return
      }
      if (!frame) frame = requestAnimationFrame(step)
    },
    stop() {
      stopped = true
      if (frame) cancelAnimationFrame(frame)
      frame = 0
    },
  }
}
