<template>
  <div class="picker-map" :style="{ height: `${height}px` }">
    <div class="canvas-area">
      <div ref="container" class="canvas" role="img" :aria-label="`Mapa da rede. ${hint}`"></div>
    </div>
    <div class="hint text-body-small">
      <v-icon size="16" color="info">mdi-gesture-tap</v-icon>
      {{ hint }}
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * Mapa de escolha do destino (assistente): a rede nas posições do próprio
 * mapa, com a origem e o caminho escolhido em destaque. Tocar num
 * equipamento que pode ser destino o escolhe — a lista "Até" continua sendo
 * o caminho acessível. Zoom e arraste livres; enquadra o caminho (ou a
 * origem e os vizinhos) quando a escolha muda.
 */
import { computed, ref, watch } from 'vue'
import { useMiniTopology } from '@/composables/linkTests/useMiniTopology'
import { useTopologyStore } from '@/stores/topology'
import type { EdgeOverlay } from '@/stores/topologyOverlay'
import { lightUp, pickerPositions } from '@/utils/linkTests/pathMap'
import { buildElements, edgeElementId, nodeElementId } from '@/utils/topology/elements'
import { edgeEndpoints } from '@/utils/topology/presentation'

const props = withDefaults(
  defineProps<{
    sourceId: number | null
    /** O caminho até o destino escolhido (vazio sem destino). */
    routeDeviceIds: number[]
    routeEdgeIds: number[]
    /** Quem pode ser escolhido como destino. */
    pickableIds: number[]
    height?: number
  }>(),
  { height: 300 }
)

const emit = defineEmits<{ pick: [deviceId: number] }>()

/** Fora do caminho: o que não pode ser destino fica mais claro, mas legível. */
const EXTRA_STYLE = [{ selector: 'node.path-dimmed, edge.path-dimmed', style: { opacity: 0.4 } }]

const topology = useTopologyStore()
const container = ref<HTMLDivElement | null>(null)

const highlighted = computed(() => {
  if (props.routeDeviceIds.length > 0) {
    return { nodeIds: props.routeDeviceIds, edgeIds: props.routeEdgeIds }
  }
  return { nodeIds: props.sourceId !== null ? [props.sourceId] : [], edgeIds: [] }
})

const pickable = computed(() => new Set(props.pickableIds))

const elements = computed(() => {
  const edgeOverlays: Record<number, EdgeOverlay> = {}
  for (const id of props.routeEdgeIds) edgeOverlays[id] = { state: 'pending' }
  const built = buildElements({
    nodes: topology.nodes,
    edges: topology.edges,
    positions: pickerPositions(
      topology.nodes.map((node) => node.id),
      topology.layout
    ),
    overlay: {
      highlight: highlighted.value.nodeIds.length > 0 ? highlighted.value : null,
      edgeOverlays,
      nodeOverlays: {},
    },
  })
  return lightUp(built, pickable.value)
})

/** Sem destino: a origem e os vizinhos dela; com destino, o caminho. */
function fitIds(): string[] {
  if (props.routeDeviceIds.length > 0) {
    return [...props.routeDeviceIds.map(nodeElementId), ...props.routeEdgeIds.map(edgeElementId)]
  }
  const source = props.sourceId
  if (source === null) return []
  const near = new Set<number>([source])
  for (const edge of topology.edges) {
    const { source: a, target: b } = edgeEndpoints(edge)
    if (a === source) near.add(b)
    if (b === source) near.add(a)
  }
  return [...near].map(nodeElementId)
}

const { fit } = useMiniTopology(container, elements, {
  interactive: true,
  maxZoom: 1.6,
  extraStyle: EXTRA_STYLE,
  fitTo: fitIds,
  onNodeTap: (id) => {
    if (pickable.value.has(id)) emit('pick', id)
  },
})

watch(
  () => [props.sourceId, props.routeDeviceIds.join(',')],
  () => fit()
)

const hint = computed(() =>
  props.sourceId === null
    ? 'Escolha de onde medir para ver a rede a partir dele.'
    : 'Toque no equipamento de destino no mapa (ou escolha na lista). Role para aproximar.'
)
</script>

<style scoped>
.picker-map {
  display: flex;
  flex-direction: column;
  width: 100%;
  border-radius: 12px;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.18);
  background: rgb(var(--v-theme-surface));
  overflow: hidden;
}
.canvas-area {
  position: relative;
  flex: 1 1 auto;
  min-height: 0;
}
.canvas {
  position: absolute;
  inset: 0;
}
/* Rodapé, não sobreposição: a dica nunca cobre um equipamento do mapa. */
.hint {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 10px;
  border-top: 1px solid rgba(var(--v-theme-on-surface), 0.18);
  color: rgb(var(--v-theme-on-surface));
}
</style>
