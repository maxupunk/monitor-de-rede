/**
 * Um Cytoscape pequeno com o desenho do mapa de topologia — os mesmos
 * elementos (`buildElements`), o mesmo estilo e a mesma animação do enlace em
 * teste (`antsAnimator`) — para os mapas do teste de velocidade: o recorte do
 * caminho (tela do teste) e o mapa de escolha do destino (assistente).
 *
 * Carrega o Cytoscape sob demanda, aplica os elementos por diferença, segue
 * o tema, enquadra quando o conjunto muda e avisa o toque num equipamento.
 */
import { onBeforeUnmount, onMounted, shallowRef, watch, type Ref } from 'vue'
import type { Core, NodeSingular } from 'cytoscape'
import { useTopologyPalette } from '@/composables/topology/useTopologyPalette'
import { antsAnimator } from '@/utils/topology/ants'
import { syncElements } from '@/utils/topology/elementSync'
import { parseElementId, type TopologyElement } from '@/utils/topology/elements'
import { buildStylesheet, type TopologyPalette } from '@/utils/topology/style'

export interface MiniTopologyOptions {
  /** Zoom e arraste do usuário (mapa de escolha). */
  interactive?: boolean
  /** Nem o enquadramento passa disto: dois equipamentos não viram gigantes. */
  maxZoom?: number
  padding?: number
  /** Ids (`n<id>`, `e<id>`) a enquadrar; vazio = tudo. */
  fitTo?: () => string[]
  /** Toque num equipamento do mapa. */
  onNodeTap?: (deviceId: number) => void
  /** Regras de estilo a mais, depois das do mapa. */
  extraStyle?: Array<{ selector: string; style: Record<string, string | number> }>
}

/**
 * O estilo do mapa, com os nomes sempre visíveis: no mapa inteiro o rótulo
 * pequeno demais some (não polui), mas aqui o nome é a informação.
 */
function stylesheet(palette: TopologyPalette, extra: MiniTopologyOptions['extraStyle'] = []) {
  return [
    ...buildStylesheet(palette),
    { selector: 'node, edge', style: { 'min-zoomed-font-size': 0 } },
    ...extra,
  ]
}

export function useMiniTopology(
  container: Ref<HTMLElement | null>,
  elements: Ref<TopologyElement[]>,
  options: MiniTopologyOptions = {}
) {
  const maxZoom = options.maxZoom ?? 1.8
  const padding = options.padding ?? 28
  const palette = useTopologyPalette()
  const cy = shallowRef<Core | null>(null)
  let destroyed = false
  let resizeObserver: ResizeObserver | null = null
  const ants = antsAnimator(() => (destroyed ? null : cy.value))

  function fit() {
    const core = cy.value
    if (!core || core.elements().empty()) return
    core.resize()
    const ids = new Set(options.fitTo?.() ?? [])
    const target = ids.size > 0 ? core.elements().filter((el) => ids.has(el.id())) : core.elements()
    core.fit(target.nonempty() ? target : core.elements(), padding)
    if (core.zoom() > maxZoom) {
      core.zoom(maxZoom)
      core.center(target.nonempty() ? target : undefined)
    }
  }

  function render() {
    const core = cy.value
    if (!core) return
    const before = core
      .nodes()
      .map((node) => node.id())
      .join(',')
    syncElements(core, elements.value)
    // Posição vem de fora (o mapa): reaplica a cada mudança.
    core.batch(() => {
      for (const el of elements.value) {
        if (el.group === 'nodes' && el.position)
          core.getElementById(el.data.id).position(el.position)
      }
    })
    if (
      core
        .nodes()
        .map((node) => node.id())
        .join(',') !== before
    )
      fit()
    ants.update()
  }

  async function init() {
    const { default: cytoscape } = await import('cytoscape')
    if (destroyed || !container.value) return
    const interactive = Boolean(options.interactive)
    const core = cytoscape({
      container: container.value,
      style: stylesheet(palette.value, options.extraStyle),
      userZoomingEnabled: interactive,
      userPanningEnabled: interactive,
      minZoom: 0.15,
      maxZoom: 3,
      boxSelectionEnabled: false,
      autoungrabify: true,
      autounselectify: true,
      motionBlur: false,
    })
    cy.value = core
    if (options.onNodeTap) {
      const tap = options.onNodeTap
      core.on('tap', 'node', (event) => {
        const parsed = parseElementId((event.target as NodeSingular).id())
        if (parsed?.kind === 'node') tap(parsed.id)
      })
    }
    render()
    fit()
    resizeObserver = new ResizeObserver(() => fit())
    resizeObserver.observe(container.value)
  }

  watch(elements, render)
  watch(palette, (value) => cy.value?.style(stylesheet(value, options.extraStyle)))

  onMounted(() => void init())
  onBeforeUnmount(() => {
    destroyed = true
    ants.stop()
    resizeObserver?.disconnect()
    cy.value?.destroy()
    cy.value = null
  })

  return { fit }
}
