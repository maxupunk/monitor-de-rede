/**
 * O Cytoscape do mapa Wi-Fi: carrega sob demanda, aplica os elementos por
 * diferença (`syncElements`), segue o tema e cuida do movimento:
 *
 * - aparelho que mudou de lugar desliza até o novo ponto;
 * - aparelho que **trocou de AP** desliza devagar do AP antigo ao novo, com
 *   destaque e a linha andando (as "formigas" do mapa de topologia);
 * - aparelho que **saiu do Wi-Fi** esmaece e some (um fantasma `__gone-*`,
 *   que o `syncElements` não toca, apagado no fim da animação).
 *
 * É animação, não consulta: nada aqui pede dado ao servidor (AGENTS §9).
 */
import { onBeforeUnmount, onMounted, ref, shallowRef, watch, type Ref } from 'vue'
import type { Core, EventObject, NodeSingular } from 'cytoscape'
import { useTopologyPalette } from '@/composables/topology/useTopologyPalette'
import { antsAnimator } from '@/utils/topology/ants'
import { syncElements, type GraphElement } from '@/utils/topology/elementSync'
import { apNodeId, linkEdgeId, macOfElement } from '@/utils/wifiMap/elements'
import { wifiMapStylesheet } from '@/utils/wifiMap/style'

/** Um deslocamento comum (a distância mudou). */
const MOVE_MS = 450
/** A troca de AP: devagar o bastante para o olho acompanhar. */
const ROAM_MS = 1600
/** Depois de chegar, o destaque ainda fica um pouco. */
const ROAM_HOLD_MS = 1800
/** Quanto o aparelho que saiu leva para sumir. */
const FADE_MS = 3500

export interface WifiMapHover {
  /** O aparelho sob o mouse; `null` quando é um AP. */
  mac: string | null
  /** O AP sob o mouse; `null` quando é um aparelho. */
  deviceId: number | null
  /** Posição na tela, relativa ao contêiner. */
  x: number
  y: number
}

export interface WifiMapOptions {
  /** Toque num aparelho (ou no fundo, com `null`). */
  onSelect: (mac: string | null) => void
  /** O aparelho ainda está conectado em algum AP (sem filtro)? */
  isPresent: (mac: string) => boolean
}

export function useWifiMap(
  container: Ref<HTMLElement | null>,
  elements: Ref<GraphElement[]>,
  options: WifiMapOptions
) {
  const palette = useTopologyPalette()
  const cy = shallowRef<Core | null>(null)
  const hovered = ref<WifiMapHover | null>(null)
  let destroyed = false
  let resizeObserver: ResizeObserver | null = null
  let fittedAps = ''
  const ants = antsAnimator(() => (destroyed ? null : cy.value))

  function fit() {
    const core = cy.value
    if (!core || core.elements().empty()) return
    core.resize()
    core.fit(core.nodes('.ring, .ap, .station'), 24)
  }

  function zoomBy(factor: number) {
    const core = cy.value
    if (!core) return
    core.zoom({
      level: core.zoom() * factor,
      renderedPosition: { x: core.width() / 2, y: core.height() / 2 },
    })
  }

  /** O aparelho que saiu esmaece no lugar e some no fim. */
  function fadeOut(core: Core, node: NodeSingular) {
    const mac = node.data('mac') as string
    const ghostId = `__gone-${mac}`
    if (!core.getElementById(ghostId).empty()) return
    const ghost = core.add({
      group: 'nodes',
      data: { ...node.data(), id: ghostId },
      classes: 'station departed',
      position: { ...node.position() },
    })
    ghost.animate({ style: { opacity: 0 } }, { duration: FADE_MS, complete: () => ghost.remove() })
  }

  /** A troca de AP: sai do AP antigo e desliza até o ponto novo, com destaque. */
  function slideRoam(
    core: Core,
    node: NodeSingular,
    fromApId: number,
    target: { x: number; y: number }
  ) {
    const fromAp = core.getElementById(apNodeId(fromApId))
    const mac = node.data('mac') as string
    const edge = core.getElementById(linkEdgeId(node.data('apId') as number, mac))
    const start = fromAp.nonempty() ? { ...fromAp.position() } : { ...node.position() }
    node.stop(true, false)
    node.position(start)
    node.addClass('roaming')
    edge.addClass('ov-running flow-forward')
    ants.update()
    node.animate(
      { position: target },
      {
        duration: ROAM_MS,
        easing: 'ease-in-out-cubic',
        complete: () => {
          // Uma animação vazia segura o destaque: é o relógio do próprio
          // Cytoscape, não um temporizador da página.
          node.animate(
            { style: {} },
            {
              duration: ROAM_HOLD_MS,
              complete: () => {
                node.removeClass('roaming')
                edge.removeClass('ov-running flow-forward')
                ants.update()
              },
            }
          )
        },
      }
    )
  }

  function render() {
    const core = cy.value
    if (!core) return
    const next = new Map(elements.value.map((element) => [element.data.id, element]))

    // Antes de aplicar: quem vai sair de vez e de qual AP cada um vinha.
    const previousAp = new Map<string, number>()
    core.nodes('.station').forEach((node) => {
      if (node.id().startsWith('__')) return
      const mac = node.data('mac') as string
      previousAp.set(node.id(), node.data('apId') as number)
      if (!next.has(node.id()) && !options.isPresent(mac)) fadeOut(core, node)
    })

    syncElements(core, elements.value)

    core.batch(() => {
      for (const element of elements.value) {
        if (element.group !== 'nodes' || !element.position) continue
        const node = core.getElementById(element.data.id)
        if (node.empty()) continue
        const target = element.position
        const before = previousAp.get(element.data.id)
        const nowAp = element.data.apId
        if (before === undefined) {
          node.position(target)
          continue
        }
        if (typeof nowAp === 'number' && before !== nowAp) {
          slideRoam(core, node, before, target)
          continue
        }
        const current = node.position()
        if (Math.abs(current.x - target.x) > 0.5 || Math.abs(current.y - target.y) > 0.5) {
          if (node.hasClass('roaming')) continue
          node.stop(true, false)
          node.animate({ position: target }, { duration: MOVE_MS, easing: 'ease-out-cubic' })
        }
      }
    })

    // Enquadra quando o conjunto de APs muda (abrir a aba, AP novo, filtro).
    const aps = core
      .nodes('.ap')
      .map((node) => node.id())
      .sort()
      .join(',')
    if (aps !== fittedAps) {
      fittedAps = aps
      fit()
    }
    ants.update()
  }

  /** Ao passar o mouse: o aparelho, a linha dele e o AP dele; o resto esmaece. */
  function focus(core: Core, node: NodeSingular | null) {
    core.batch(() => {
      core.elements().removeClass('dim focus')
      if (!node) return
      const neighborhood = node.closedNeighborhood()
      core.elements().not(neighborhood).not('.ring').addClass('dim')
      neighborhood.addClass('focus')
    })
  }

  function hoverAt(event: EventObject) {
    const node = event.target as NodeSingular
    const mac = macOfElement(node.id())
    const deviceId = node.hasClass('ap') ? (node.data('deviceId') as number) : null
    if (!mac && deviceId === null) return
    const position = node.renderedPosition()
    hovered.value = { mac, deviceId, x: position.x, y: position.y }
  }

  async function init() {
    const { default: cytoscape } = await import('cytoscape')
    if (destroyed || !container.value) return
    const core = cytoscape({
      container: container.value,
      style: wifiMapStylesheet(palette.value),
      minZoom: 0.12,
      maxZoom: 3,
      wheelSensitivity: 0.25,
      boxSelectionEnabled: false,
      autoungrabify: true,
      autounselectify: true,
      motionBlur: false,
    })
    cy.value = core
    core.on('mouseover', 'node.station, node.ap', (event) => {
      focus(core, event.target as NodeSingular)
      hoverAt(event)
    })
    core.on('mouseout', 'node.station, node.ap', () => {
      focus(core, null)
      hovered.value = null
    })
    core.on('viewport', () => {
      hovered.value = null
    })
    core.on('tap', 'node.station', (event) => {
      const mac = macOfElement((event.target as NodeSingular).id())
      if (mac) options.onSelect(mac)
    })
    // Tocar num AP aproxima a célula dele; tocar no fundo fecha o painel.
    core.on('tap', 'node.ap', (event) => {
      const id = (event.target as NodeSingular).id()
      const deviceId = Number(id.slice(3))
      const cell = core.nodes(`[deviceId = ${deviceId}], .station[apId = ${deviceId}]`)
      core.animate(
        { fit: { eles: cell.union(core.nodes(`.ring[id ^= "ring-${deviceId}-"]`)), padding: 24 } },
        { duration: 400 }
      )
    })
    core.on('tap', (event) => {
      if (event.target === core) options.onSelect(null)
    })
    render()
    fit()
    resizeObserver = new ResizeObserver(() => fit())
    resizeObserver.observe(container.value)
  }

  watch(elements, render)
  watch(palette, (value) => cy.value?.style(wifiMapStylesheet(value)))

  onMounted(() => void init())
  onBeforeUnmount(() => {
    destroyed = true
    ants.stop()
    resizeObserver?.disconnect()
    cy.value?.destroy()
    cy.value = null
  })

  return {
    fit,
    zoomIn: () => zoomBy(1.25),
    zoomOut: () => zoomBy(0.8),
    hovered,
  }
}
