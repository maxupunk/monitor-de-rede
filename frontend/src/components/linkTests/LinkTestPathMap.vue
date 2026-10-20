<template>
  <div class="path-map" :style="{ height: `${mapHeight}px` }">
    <div ref="container" class="canvas" role="img" :aria-label="ariaLabel"></div>
    <div v-if="missing" class="missing text-body-small">
      <v-icon size="18" color="info">mdi-map-marker-question-outline</v-icon>
      {{ missing }}
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * Recorte do mapa com o caminho do teste: os equipamentos nas posições do
 * próprio mapa (com folga para os Mbps, `pathMapPositions`), o mesmo desenho
 * e a mesma animação (`useMiniTopology`). Só leitura. Na tela do teste:
 * pendente → medindo, com o fluxo animado → resultado em cada enlace.
 */
import { computed, onMounted, ref } from 'vue'
import { useMiniTopology } from '@/composables/linkTests/useMiniTopology'
import { useTopologyStore } from '@/stores/topology'
import type { LinkTestOverlay } from '@/utils/linkTests/overlay'
import { pathMapHeight, pathMapPositions } from '@/utils/linkTests/pathMap'
import { buildElements } from '@/utils/topology/elements'

const props = withDefaults(
  defineProps<{
    /** Os equipamentos do caminho, em ordem de A a B. */
    nodeIds: number[]
    edgeIds: number[]
    /** Estado do teste em cada enlace/ponta; `null` desenha só o caminho. */
    overlay?: LinkTestOverlay | null
    /** Teto da altura; o recorte cresce com o caminho até ele. */
    maxHeight?: number
  }>(),
  { overlay: null, maxHeight: 560 }
)

const topology = useTopologyStore()
const container = ref<HTMLDivElement | null>(null)

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

const positions = computed(() =>
  pathMapPositions(
    nodes.value.map((node) => node.id),
    topology.layout
  )
)
const mapHeight = computed(() => pathMapHeight(positions.value, 240, props.maxHeight))

const elements = computed(() =>
  buildElements({
    nodes: nodes.value,
    edges: edges.value,
    positions: positions.value,
    overlay: {
      highlight: null,
      edgeOverlays: props.overlay?.edges ?? {},
      nodeOverlays: props.overlay?.nodes ?? {},
    },
  })
)

useMiniTopology(container, elements)

const ariaLabel = computed(() => {
  const names = nodes.value.map((node) => node.name)
  return names.length > 0 ? `Caminho do teste: ${names.join(' → ')}` : 'Caminho do teste'
})

/** Caminho que o mapa não conhece, ou equipamentos fora dele. */
const missing = computed(() => {
  if (props.nodeIds.length === 0) return 'O mapa não conhece o caminho entre as pontas.'
  if (topology.loaded && nodes.value.length === 0) return 'Estes equipamentos não estão no mapa.'
  return null
})

/** Fora da topologia (histórico), o mapa vem sob demanda. */
onMounted(() => {
  if (!topology.loaded) void topology.fetchTopology(null, false)
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
