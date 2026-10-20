<template>
  <div class="path-map" :style="{ height: `${height}px` }">
    <div ref="container" class="canvas" role="img" :aria-label="ariaLabel"></div>
    <div v-if="missing" class="missing text-body-small">
      <v-icon size="18" color="info">mdi-map-marker-question-outline</v-icon>
      {{ missing }}
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * Mini mapa do teste: o caminho medido em linha, da origem ao destino — os
 * mesmos elementos (`buildElements`), o mesmo estilo e a mesma animação do
 * enlace em teste (`antsAnimator`) do mapa de topologia. Só leitura: sem zoom, arraste nem
 * seleção. Usado no assistente (o caminho escolhido) e na tela do teste
 * (pendente → medindo, com o fluxo animado → resultado em cada enlace).
 */
import { computed, onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue'
import type { Core } from 'cytoscape'
import { useTopologyPalette } from '@/composables/topology/useTopologyPalette'
import { useTopologyStore } from '@/stores/topology'
import type { LinkTestOverlay } from '@/utils/linkTests/overlay'
import { pathMapPositions } from '@/utils/linkTests/pathMap'
import { antsAnimator } from '@/utils/topology/ants'
import { syncElements } from '@/utils/topology/elementSync'
import { buildElements } from '@/utils/topology/elements'
import { buildStylesheet, type TopologyPalette } from '@/utils/topology/style'

const props = withDefaults(
  defineProps<{
    /** Os equipamentos do caminho, em ordem de A a B. */
    nodeIds: number[]
    edgeIds: number[]
    /** Estado do teste em cada enlace/ponta; `null` desenha só o caminho. */
    overlay?: LinkTestOverlay | null
    height?: number
    /** Tela estreita: o caminho em pé, de cima para baixo. */
    vertical?: boolean
  }>(),
  { overlay: null, height: 220, vertical: false }
)

/** Nem o zoom do recorte passa disto: dois equipamentos não viram gigantes. */
const MAX_ZOOM = 1.8
const PADDING = 28

/**
 * O estilo do mapa, com os nomes sempre visíveis: no mapa inteiro o rótulo
 * pequeno demais some (não polui), mas no recorte o nome é a informação.
 */
function stylesheet(value: TopologyPalette) {
  return [
    ...buildStylesheet(value),
    { selector: 'node, edge', style: { 'min-zoomed-font-size': 0 } },
  ]
}

const topology = useTopologyStore()
const palette = useTopologyPalette()
const container = ref<HTMLDivElement | null>(null)
const cy = shallowRef<Core | null>(null)
let destroyed = false
let resizeObserver: ResizeObserver | null = null
const ants = antsAnimator(() => (destroyed ? null : cy.value))

const nodes = computed(() =>
  props.nodeIds.flatMap((id) => {
    const node = topology.nodesById.get(id)
    return node ? [node] : []
  })
)
const edges = computed(() =>
  props.edgeIds.flatMap((id) => {
    const edge = topology.edgesById.get(id)
    return edge ? [edge] : []
  })
)

const elements = computed(() =>
  buildElements({
    nodes: nodes.value,
    edges: edges.value,
    positions: pathMapPositions(
      nodes.value.map((node) => node.id),
      props.vertical
    ),
    overlay: {
      highlight: null,
      edgeOverlays: props.overlay?.edges ?? {},
      nodeOverlays: props.overlay?.nodes ?? {},
    },
  })
)

const ariaLabel = computed(() => {
  const names = nodes.value.map((node) => node.name)
  return names.length > 0 ? `Caminho do teste: ${names.join(' → ')}` : 'Caminho do teste'
})

/** Sem o mapa carregado ainda, ou caminho que o mapa não conhece. */
const missing = computed(() => {
  if (props.nodeIds.length === 0) return 'O mapa não conhece o caminho entre as pontas.'
  if (topology.loaded && nodes.value.length === 0) return 'Estes equipamentos não estão no mapa.'
  return null
})

function fit() {
  const core = cy.value
  if (!core || core.elements().empty()) return
  core.resize()
  core.fit(undefined, PADDING)
  if (core.zoom() > MAX_ZOOM) {
    core.zoom(MAX_ZOOM)
    core.center()
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
  // Posição vem do mapa: reaplica e enquadra quando o caminho muda.
  core.batch(() => {
    for (const el of elements.value) {
      if (el.group === 'nodes' && el.position) core.getElementById(el.data.id).position(el.position)
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
  const core = cytoscape({
    container: container.value,
    style: stylesheet(palette.value),
    userZoomingEnabled: false,
    userPanningEnabled: false,
    boxSelectionEnabled: false,
    autoungrabify: true,
    autounselectify: true,
    motionBlur: false,
  })
  cy.value = core
  render()
  fit()
  resizeObserver = new ResizeObserver(() => fit())
  resizeObserver.observe(container.value)
}

watch(elements, render)
watch(palette, (value) => cy.value?.style(stylesheet(value)))
watch(
  () => props.vertical,
  () => {
    render()
    fit()
  }
)

/** Fora da topologia (histórico), o mapa vem sob demanda. */
onMounted(() => {
  if (!topology.loaded) void topology.fetchTopology(null, false)
  void init()
})

onBeforeUnmount(() => {
  destroyed = true
  ants.stop()
  resizeObserver?.disconnect()
  cy.value?.destroy()
  cy.value = null
})
</script>

<style scoped>
.path-map {
  position: relative;
  width: 100%;
  border-radius: 12px;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.18);
  background: rgb(var(--v-theme-surface));
  overflow: hidden;
}
.canvas {
  position: absolute;
  inset: 0;
}
.missing {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 12px;
  text-align: center;
  color: rgb(var(--v-theme-on-surface));
}
</style>
