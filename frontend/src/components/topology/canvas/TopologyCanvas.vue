<template>
  <div
    ref="container"
    class="topology-canvas"
    :class="{ 'is-connecting': connectSourceId !== null }"
  ></div>
</template>

<script setup lang="ts">
/**
 * Invólucro do Cytoscape. Só desenha e traduz eventos do Cytoscape em eventos
 * semânticos (`node-tap`, `edge-tap`, `context`…) — nenhuma regra de negócio.
 *
 * Os elementos chegam prontos (`utils/topology/elements.ts`) e são aplicados
 * por diferença: dado e classe que mudaram, nó novo, nó que saiu. Posição só
 * muda por `applyPositions` (layout, outra aba) ou pelo arraste do usuário.
 */
import { onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue'
import type {
  Core,
  EdgeCollection,
  EdgeSingular,
  EventObject,
  NodeCollection,
  NodeSingular,
} from 'cytoscape'
import type { TopologyPosition } from '@/stores/topology'
import { parseElementId, type TopologyElement } from '@/utils/topology/elements'
import {
  buildStylesheet,
  edgeRouteClass,
  lodLevel,
  LOD_CLASSES,
  ROUTE_CLASSES,
  TRAFFIC_ZOOM,
  type LodLevel,
  type TopologyPalette,
} from '@/utils/topology/style'
import {
  fitViewport,
  type CanvasPointerEvent,
  type CanvasViewport,
  type MinimapData,
  type MinimapSource,
} from '@/utils/topology/canvasTypes'

export interface CanvasSelection {
  kind: 'node' | 'edge'
  id: number
}

const props = defineProps<{
  elements: TopologyElement[]
  palette: TopologyPalette
  /** Arrastar nós (modo de edição). */
  editable: boolean
  /** Origem do cabo em andamento (modo conectar). */
  connectSourceId: number | null
  /** Rótulos de detalhe sempre, não só com zoom. */
  alwaysDetail: boolean
  /** Enlaces em ângulo reto (árvore) quando uma ponta está acima da outra. */
  orthogonal: boolean
  selection: CanvasSelection | null
  /** Faixas cobertas por painéis flutuantes — o enquadramento as evita. */
  insets?: { top: number; right: number; bottom: number; left: number }
}>()

const emit = defineEmits<{
  ready: []
  'node-tap': [deviceId: number]
  'edge-tap': [edgeId: number]
  'background-tap': []
  context: [event: CanvasPointerEvent]
  hover: [event: CanvasPointerEvent | null]
  'positions-changed': [positions: Record<number, TopologyPosition>]
  'drag-state': [dragging: boolean]
  viewport: [viewport: CanvasViewport]
}>()

const container = ref<HTMLDivElement | null>(null)
const cy = shallowRef<Core | null>(null)
const sceneListeners = new Set<() => void>()
let resizeObserver: ResizeObserver | null = null
let sceneFrame = 0
let antsFrame = 0
let antsOffset = 0
let trafficOn = false
let lod: LodLevel = 'near'
let draggedBatch: Record<number, TopologyPosition> = {}
let dragFlushQueued = false
let destroyed = false

const BASE_CLASSES = 'baseClasses'
const GHOST_NODE = '__ghost-node'
const GHOST_EDGE = '__ghost-edge'

function isDeviceNode(ele: NodeSingular): boolean {
  return ele.id().startsWith('n')
}

function relative(event: EventObject): { x: number; y: number; clientX: number; clientY: number } {
  const rect = container.value?.getBoundingClientRect()
  const original = event.originalEvent as MouseEvent | TouchEvent | undefined
  let clientX = (rect?.left ?? 0) + (event.renderedPosition?.x ?? 0)
  let clientY = (rect?.top ?? 0) + (event.renderedPosition?.y ?? 0)
  if (original && 'clientX' in original) {
    clientX = original.clientX
    clientY = original.clientY
  } else if (original && 'changedTouches' in original && original.changedTouches.length > 0) {
    clientX = original.changedTouches[0].clientX
    clientY = original.changedTouches[0].clientY
  }
  return {
    x: clientX - (rect?.left ?? 0),
    y: clientY - (rect?.top ?? 0),
    clientX,
    clientY,
  }
}

function pointerFor(event: EventObject): CanvasPointerEvent | null {
  const target = event.target as Core | NodeSingular | EdgeSingular
  const coords = relative(event)
  if (target === cy.value) return { kind: 'background', ...coords }
  const parsed = parseElementId((target as NodeSingular).id())
  if (!parsed || parsed.kind === 'site') return null
  return { kind: parsed.kind, id: parsed.id, ...coords }
}

/** Centro do elemento relativo ao contêiner (para o tooltip). */
function anchorOf(ele: NodeSingular | EdgeSingular): { x: number; y: number } {
  if (ele.isNode()) {
    const pos = ele.renderedPosition()
    return { x: pos.x, y: pos.y - ele.renderedOuterHeight() / 2 }
  }
  const mid = ele.renderedMidpoint()
  return { x: mid.x, y: mid.y }
}

// --- Sincronização de elementos ----------------------------------------------

function syncElements(core: Core, elements: TopologyElement[]) {
  core.batch(() => {
    const nextIds = new Set(elements.map((el) => el.data.id))
    const toAdd: TopologyElement[] = []

    for (const el of elements) {
      const existing = core.getElementById(el.data.id)
      if (existing.empty()) {
        toAdd.push(el)
        continue
      }
      const current = existing.data() as Record<string, unknown>
      const patch: Record<string, unknown> = {}
      let changed = false
      for (const [key, value] of Object.entries(el.data)) {
        if (key === 'parent' || key === 'source' || key === 'target') continue
        if (current[key] !== value) {
          patch[key] = value
          changed = true
        }
      }
      if (changed) existing.data(patch)
      if (existing.isNode()) {
        const wanted = (el.data.parent as string | undefined) ?? null
        const now = (existing.data('parent') as string | undefined) ?? null
        if (wanted !== now) (existing as NodeSingular).move({ parent: wanted })
      }
      const previous = (existing.scratch(BASE_CLASSES) as string | undefined) ?? ''
      if (previous !== el.classes) {
        existing.removeClass(previous)
        existing.addClass(el.classes)
        existing.scratch(BASE_CLASSES, el.classes)
      }
    }

    if (toAdd.length > 0) {
      const added = core.add(toAdd.map((el) => ({ ...el, data: { ...el.data } })))
      added.forEach((ele) => {
        const def = toAdd.find((item) => item.data.id === ele.id())
        ele.scratch(BASE_CLASSES, def?.classes ?? '')
      })
      if (trafficOn) added.edges().addClass('lod-detail')
      if (lod !== 'near') added.filter('[deviceId], [edgeId]').addClass(`lod-${lod}`)
      routeEdges(core, added.edges().union(added.nodes().connectedEdges()))
    }

    // Remoção por último: um site (nó composto) só sai depois que os filhos
    // já foram movidos para fora dele.
    core
      .elements()
      .filter((ele) => !nextIds.has(ele.id()) && !ele.id().startsWith('__'))
      .remove()
  })
  updateAnts()
  scheduleScene()
}

// --- Rótulos por nível de zoom (LOD) --------------------------------------------

/**
 * De perto, todo enlace mostra o tráfego. Antes disso, só o enlace focado (do
 * nó selecionado ou sob o mouse) mostra — dezenas de pílulas ao mesmo tempo
 * viram ruído. Os rótulos dos nós somem sozinhos de longe (fonte mínima).
 */
/**
 * Troca a classe de nível de detalhe na hora do zoom (no evento, não no
 * próximo quadro): se o Cytoscape desenhar o novo zoom antes da troca, ele
 * guarda a textura do nó com a caixa do rótulo antigo e corta o novo.
 */
function updateLod(core: Core) {
  const level = lodLevel(core.zoom())
  if (level === lod) return
  lod = level
  core.batch(() => {
    const items = core.elements('[deviceId], [edgeId]')
    items.removeClass(LOD_CLASSES)
    if (level !== 'near') items.addClass(`lod-${level}`)
  })
}

function updateDetail(core: Core) {
  updateLod(core)
  const edges = props.alwaysDetail || core.zoom() >= TRAFFIC_ZOOM
  if (edges === trafficOn) return
  trafficOn = edges
  core.batch(() => {
    if (edges) core.edges().addClass('lod-detail')
    else core.edges().removeClass('lod-detail')
  })
}

// --- Traçado ortogonal -----------------------------------------------------------

/** Recalcula o traçado dos enlaces dados (ou de todos) pela posição das pontas. */
function routeEdges(core: Core, edges: EdgeCollection = core.edges()) {
  core.batch(() => {
    edges.forEach((edge) => {
      if (edge.id().startsWith('__')) return
      const wanted = props.orthogonal
        ? edgeRouteClass(edge.source().position(), edge.target().position())
        : null
      if (wanted && edge.hasClass(wanted)) return
      edge.removeClass(ROUTE_CLASSES)
      if (wanted) edge.addClass(wanted)
    })
  })
}

function scheduleScene() {
  if (sceneFrame || destroyed) return
  sceneFrame = requestAnimationFrame(() => {
    sceneFrame = 0
    const core = cy.value
    if (!core) return
    updateDetail(core)
    emit('viewport', { zoom: core.zoom(), nodeCount: core.nodes().length })
    for (const listener of sceneListeners) listener()
  })
}

// --- "Formigas" do enlace em teste (animação, não consulta) --------------------

function updateAnts() {
  const core = cy.value
  if (!core) return
  const running = core.edges('.ov-running')
  if (running.empty()) {
    if (antsFrame) cancelAnimationFrame(antsFrame)
    antsFrame = 0
    core.edges().removeStyle('line-dash-offset')
    return
  }
  if (antsFrame) return
  const step = () => {
    const current = cy.value
    if (!current || destroyed) return
    const edges = current.edges('.ov-running')
    if (edges.empty()) {
      antsFrame = 0
      return
    }
    antsOffset = (antsOffset + 1.2) % 1000
    current.batch(() => {
      edges.forEach((edge) => {
        edge.style('line-dash-offset', edge.hasClass('flow-reverse') ? antsOffset : -antsOffset)
      })
    })
    antsFrame = requestAnimationFrame(step)
  }
  antsFrame = requestAnimationFrame(step)
}

// --- Cabo em andamento (modo conectar) -------------------------------------------

/** Destino válido sob o cursor enquanto se puxa o cabo. */
let connectTarget: NodeSingular | null = null

function markConnectTarget(ele: NodeSingular | EdgeSingular) {
  if (props.connectSourceId === null || !ele.isNode()) return
  const node = ele as NodeSingular
  const parsed = parseElementId(node.id())
  if (!isDeviceNode(node) || parsed?.kind !== 'node' || parsed.id === props.connectSourceId) return
  connectTarget = node
  node.addClass('connect-target')
  updateGhost({ ...node.position() })
}

function updateGhost(position?: TopologyPosition) {
  const core = cy.value
  if (!core) return
  const sourceId = props.connectSourceId
  const ghostNode = core.getElementById(GHOST_NODE)
  if (sourceId === null) {
    core.remove(core.getElementById(GHOST_EDGE))
    core.remove(ghostNode)
    core.nodes('.connect-source, .connect-target').removeClass('connect-source connect-target')
    connectTarget = null
    return
  }
  const source = core.getElementById(`n${sourceId}`)
  if (source.empty()) return
  core.nodes('.connect-source').not(source).removeClass('connect-source')
  source.addClass('connect-source')
  const at = position ?? source.position()
  if (ghostNode.empty()) {
    core.add([
      { group: 'nodes', data: { id: GHOST_NODE }, position: { ...at }, classes: 'ghost' },
      {
        group: 'edges',
        data: { id: GHOST_EDGE, source: source.id(), target: GHOST_NODE },
        classes: 'ghost',
      },
    ])
    core.getElementById(GHOST_NODE).ungrabify().unselectify()
  } else if (position) {
    ghostNode.position(position)
  }
}

// --- Inicialização ----------------------------------------------------------------

async function init() {
  const { default: cytoscape } = await import('cytoscape')
  if (destroyed || !container.value) return
  const large = props.elements.filter((el) => el.group === 'nodes').length > 500
  const core = cytoscape({
    container: container.value,
    style: buildStylesheet(props.palette),
    minZoom: 0.08,
    maxZoom: 3,
    boxSelectionEnabled: false,
    selectionType: 'single',
    textureOnViewport: large,
    hideEdgesOnViewport: large,
    motionBlur: false,
    autoungrabify: !props.editable,
  })
  cy.value = core
  syncElements(core, props.elements)

  core.on('tap', 'node', (event) => {
    const node = event.target as NodeSingular
    if (!isDeviceNode(node)) return
    const parsed = parseElementId(node.id())
    if (parsed) emit('node-tap', parsed.id)
  })
  core.on('tap', 'edge', (event) => {
    const parsed = parseElementId((event.target as EdgeSingular).id())
    if (parsed?.kind === 'edge') emit('edge-tap', parsed.id)
  })
  core.on('tap', (event) => {
    if (event.target === core) emit('background-tap')
  })
  const onContext = (event: EventObject) => {
    const pointer = pointerFor(event)
    if (pointer) emit('context', pointer)
  }
  core.on('cxttap', onContext)
  core.on('taphold', onContext)

  core.on('mouseover', 'node, edge', (event) => {
    const ele = event.target as NodeSingular | EdgeSingular
    if (ele.id().startsWith('__') || ele.hasClass('site-group')) return
    ele.addClass('hover')
    markConnectTarget(ele)
    const parsed = parseElementId(ele.id())
    if (!parsed || parsed.kind === 'site') return
    const rect = container.value?.getBoundingClientRect()
    const anchor = anchorOf(ele)
    emit('hover', {
      kind: parsed.kind,
      id: parsed.id,
      ...anchor,
      clientX: (rect?.left ?? 0) + anchor.x,
      clientY: (rect?.top ?? 0) + anchor.y,
    })
  })
  core.on('mouseout', 'node, edge', (event) => {
    ;(event.target as NodeSingular | EdgeSingular).removeClass('hover connect-target')
    if (connectTarget === event.target) connectTarget = null
    emit('hover', null)
  })
  core.on('mousemove', (event) => {
    if (props.connectSourceId === null) return
    // Sobre um destino válido, o cabo "encaixa" no centro dele.
    updateGhost(connectTarget ? { ...connectTarget.position() } : { ...event.position })
  })

  core.on('grab', 'node', () => {
    emit('hover', null)
    emit('drag-state', true)
  })
  core.on('dragfree', 'node', (event) => {
    const node = event.target as NodeSingular
    if (!isDeviceNode(node)) return
    routeEdges(core, node.connectedEdges())
    const parsed = parseElementId(node.id())
    if (!parsed) return
    const pos = node.position()
    draggedBatch[parsed.id] = { x: pos.x, y: pos.y }
    if (dragFlushQueued) return
    dragFlushQueued = true
    queueMicrotask(() => {
      dragFlushQueued = false
      const batch = draggedBatch
      draggedBatch = {}
      emit('positions-changed', batch)
      emit('drag-state', false)
    })
  })
  core.on('free', 'node', () => emit('drag-state', false))
  core.on('zoom', () => updateLod(core))
  core.on('viewport position', scheduleScene)
  core.on('add remove', scheduleScene)

  resizeObserver = new ResizeObserver(() => {
    core.resize()
    scheduleScene()
  })
  resizeObserver.observe(container.value)
  applySelection()
  emit('ready')
}

function applySelection() {
  const core = cy.value
  if (!core) return
  const selection = props.selection
  const wanted = selection
    ? core.getElementById(`${selection.kind === 'node' ? 'n' : 'e'}${selection.id}`)
    : null
  core.batch(() => {
    core
      .$(':selected')
      .filter((ele) => !wanted || ele.id() !== wanted.id())
      .unselect()
    if (wanted && wanted.nonempty() && !wanted.selected()) wanted.select()
    core.edges('.focus').removeClass('focus focus-out focus-in')
    if (wanted?.nonempty() && wanted.isNode()) {
      // O tráfego vai para perto da OUTRA ponta: enlaces que saem em leque do
      // nó selecionado não empilham as pílulas no mesmo lugar.
      wanted.connectedEdges().forEach((edge) => {
        edge.addClass(edge.source().same(wanted) ? 'focus focus-out' : 'focus focus-in')
      })
    }
  })
}

onMounted(() => {
  void init()
})

onBeforeUnmount(() => {
  destroyed = true
  if (sceneFrame) cancelAnimationFrame(sceneFrame)
  if (antsFrame) cancelAnimationFrame(antsFrame)
  resizeObserver?.disconnect()
  sceneListeners.clear()
  cy.value?.destroy()
  cy.value = null
})

watch(
  () => props.elements,
  (elements) => {
    if (cy.value) syncElements(cy.value, elements)
    if (props.connectSourceId !== null) updateGhost()
    applySelection()
  }
)
watch(
  () => props.palette,
  (palette) => cy.value?.style(buildStylesheet(palette))
)
watch(
  () => props.editable,
  (editable) => cy.value?.autoungrabify(!editable)
)
watch(
  () => props.connectSourceId,
  () => updateGhost()
)
watch(
  () => props.orthogonal,
  () => {
    if (cy.value) routeEdges(cy.value)
  }
)
watch(
  () => props.alwaysDetail,
  () => {
    if (cy.value) updateDetail(cy.value)
  }
)
watch(() => props.selection, applySelection)

// --- API pública (página e minimapa) -----------------------------------------------

function centerRendered(core: Core) {
  return { x: core.width() / 2, y: core.height() / 2 }
}

function visibleDeviceNodes(core: Core): NodeCollection {
  return core
    .nodes()
    .filter((node) => isDeviceNode(node) && node.visible())
    .nodes()
}

const api = {
  fit(deviceIds?: number[]) {
    const core = cy.value
    if (!core) return
    const wanted = new Set((deviceIds ?? []).map((id) => `n${id}`))
    const eles =
      wanted.size > 0
        ? core.nodes().filter((node) => wanted.has(node.id()))
        : visibleDeviceNodes(core)
    if (eles.empty()) return
    // Enquadra na área livre: legenda, barra de edição e rodapé cobrem o canvas.
    const insets = props.insets ?? { top: 0, right: 0, bottom: 0, left: 0 }
    const { zoom, pan } = fitViewport(
      eles.boundingBox({ includeLabels: true }),
      { width: core.width(), height: core.height() },
      insets,
      core.minZoom()
    )
    core.animate({ zoom, pan, duration: 260, easing: 'ease-out-cubic' })
  },
  zoomBy(factor: number) {
    const core = cy.value
    if (!core) return
    const level = Math.max(core.minZoom(), Math.min(core.maxZoom(), core.zoom() * factor))
    core.animate({ zoom: { level, renderedPosition: centerRendered(core) }, duration: 160 })
  },
  resetZoom() {
    const core = cy.value
    if (!core) return
    core.animate({ zoom: { level: 1, renderedPosition: centerRendered(core) }, duration: 200 })
  },
  centerOn(deviceId: number) {
    const core = cy.value
    const node = core?.getElementById(`n${deviceId}`)
    if (!core || !node || node.empty()) return
    // O painel lateral pode ter acabado de abrir: mede o contêiner de novo.
    core.resize()
    const zoom = Math.max(core.zoom(), 1.2)
    const pos = node.position()
    // Centro da área livre (a folha do celular cobre a metade de baixo).
    const insets = props.insets ?? { top: 0, right: 0, bottom: 0, left: 0 }
    const cx = insets.left + (core.width() - insets.left - insets.right) / 2
    const cyPos = insets.top + (core.height() - insets.top - insets.bottom) / 2
    core.animate({
      zoom,
      pan: { x: cx - pos.x * zoom, y: cyPos - pos.y * zoom },
      duration: 320,
      easing: 'ease-in-out-cubic',
    })
  },
  /**
   * Garante o nó visível na área livre (fora de painéis e folhas), sem mudar
   * o zoom — só desloca quando ele está escondido.
   */
  reveal(deviceId: number) {
    const core = cy.value
    const node = core?.getElementById(`n${deviceId}`)
    if (!core || !node || node.empty()) return
    core.resize()
    const insets = props.insets ?? { top: 0, right: 0, bottom: 0, left: 0 }
    const pad = 48
    const pos = node.renderedPosition()
    const left = insets.left + pad
    const right = core.width() - insets.right - pad
    const top = insets.top + pad
    const bottom = core.height() - insets.bottom - pad
    if (right <= left || bottom <= top) return
    if (pos.x >= left && pos.x <= right && pos.y >= top && pos.y <= bottom) return
    const pan = core.pan()
    core.animate({
      pan: {
        x: pan.x + (left + right) / 2 - pos.x,
        y: pan.y + (top + bottom) / 2 - pos.y,
      },
      duration: 260,
      easing: 'ease-out-cubic',
    })
  },
  /** Move nós para as posições dadas (layout, outra aba). Só os listados. */
  applyPositions(positions: Record<number, TopologyPosition>, animate = true, fitAfter = false) {
    const core = cy.value
    if (!core) return
    const nodes = core.nodes().filter((node) => {
      const parsed = parseElementId(node.id())
      return parsed?.kind === 'node' && positions[parsed.id] !== undefined
    })
    if (nodes.empty()) return
    const byElementId: Record<string, TopologyPosition> = {}
    for (const [id, pos] of Object.entries(positions)) byElementId[`n${id}`] = pos
    const smooth = animate && nodes.length <= 600
    const layout = nodes.layout({
      name: 'preset',
      positions: byElementId,
      animate: smooth,
      animationDuration: 450,
      animationEasing: 'ease-in-out-cubic',
      fit: false,
    })
    layout.one('layoutstop', () => {
      routeEdges(core, nodes.connectedEdges())
      if (fitAfter) api.fit()
    })
    layout.run()
  },
  getPositions(): Record<number, TopologyPosition> {
    const result: Record<number, TopologyPosition> = {}
    cy.value?.nodes().forEach((node) => {
      const parsed = parseElementId(node.id())
      if (parsed?.kind === 'node') result[parsed.id] = { ...node.position() }
    })
    return result
  },
  nudge(deviceId: number, dx: number, dy: number) {
    const node = cy.value?.getElementById(`n${deviceId}`)
    if (!node || node.empty()) return
    const pos = node.position()
    const next = { x: pos.x + dx, y: pos.y + dy }
    node.position(next)
    if (cy.value) routeEdges(cy.value, node.connectedEdges())
    emit('positions-changed', { [deviceId]: next })
  },
  async exportPng(): Promise<Blob | null> {
    const core = cy.value
    if (!core) return null
    return core.png({
      output: 'blob-promise',
      full: true,
      scale: 2,
      maxWidth: 8000,
      maxHeight: 8000,
      bg: props.palette.background,
    })
  },
  /** Ponto do nó na tela (para menus abertos pelo teclado). */
  clientPointOf(deviceId: number): { x: number; y: number } | null {
    const node = cy.value?.getElementById(`n${deviceId}`)
    const rect = container.value?.getBoundingClientRect()
    if (!node || node.empty() || !rect) return null
    const pos = node.renderedPosition()
    return { x: rect.left + pos.x, y: rect.top + pos.y }
  },
} satisfies Record<string, unknown>

const minimapSource: MinimapSource = {
  getMinimapData(): MinimapData | null {
    const core = cy.value
    if (!core) return null
    const nodes = visibleDeviceNodes(core)
    if (nodes.empty()) return null
    const box = nodes.boundingBox({ includeLabels: false })
    const extent = core.extent()
    return {
      nodes: nodes.map((node) => ({
        ...node.position(),
        status: node.hasClass('status-offline')
          ? 'offline'
          : node.hasClass('status-warning')
            ? 'warning'
            : node.hasClass('status-online')
              ? 'online'
              : 'unknown',
        system: node.hasClass('system'),
      })),
      bounds: { x1: box.x1, y1: box.y1, x2: box.x2, y2: box.y2 },
      viewport: { x1: extent.x1, y1: extent.y1, x2: extent.x2, y2: extent.y2 },
    }
  },
  onSceneChange(listener) {
    sceneListeners.add(listener)
    return () => sceneListeners.delete(listener)
  },
  panToModel(position) {
    const core = cy.value
    if (!core) return
    const zoom = core.zoom()
    core.pan({ x: core.width() / 2 - position.x * zoom, y: core.height() / 2 - position.y * zoom })
  },
}

defineExpose({ ...api, minimapSource })
</script>

<style scoped>
.topology-canvas {
  position: absolute;
  inset: 0;
  background-color: rgb(var(--v-theme-background));
  background-image: radial-gradient(rgba(var(--v-theme-on-surface), 0.09) 1px, transparent 1px);
  background-size: 28px 28px;
  touch-action: none;
}
.topology-canvas.is-connecting {
  cursor: crosshair;
}
</style>
