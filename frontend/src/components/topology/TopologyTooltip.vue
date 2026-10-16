<template>
  <div class="topology-tooltip" :style="style" role="tooltip">
    <template v-if="node">
      <div class="d-flex align-center ga-2">
        <v-icon size="18" :color="nodeStatus.color">{{ nodeIcon(node) }}</v-icon>
        <span class="font-weight-bold title">{{ node.name }}</span>
      </div>
      <div class="meta">
        <span
          class="status-dot"
          :style="{ background: `rgb(var(--v-theme-${nodeStatus.color}))` }"
        ></span>
        {{ nodeStatus.label }} · {{ nodeTypeLabel(node) }}
      </div>
      <div v-if="ip" class="meta mono">{{ ip }}</div>
      <div v-if="node.vendor || node.model" class="meta">
        {{ [node.vendor, node.model].filter(Boolean).join(' ') }}
      </div>
      <div v-if="nodeOverlayLabel" class="meta font-weight-bold">{{ nodeOverlayLabel }}</div>
    </template>
    <template v-else-if="edge">
      <div class="font-weight-bold title">
        {{ edge.sourceDeviceName || 'Origem' }} ↔ {{ edge.targetDeviceName || 'Destino' }}
      </div>
      <div v-if="edge.sourceInterfaceName || edge.targetInterfaceName" class="meta mono">
        {{ edge.sourceInterfaceName || '—' }} → {{ edge.targetInterfaceName || '—' }}
      </div>
      <div class="meta">
        <span
          class="status-dot"
          :style="{ background: `rgb(var(--v-theme-${edgeStatus.color}))` }"
        ></span>
        {{ edgeStatus.label }} · {{ linkKindPresentation(edge.linkType).label }}
      </div>
      <div class="meta">Velocidade: {{ edgeSpeedLabel(edge) }}</div>
      <div v-if="traffic" class="meta">
        {{ traffic }}
        <span v-if="utilization !== null"> · {{ formatPercent(utilization * 100, 0) }}</span>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { TopologyEdge, TopologyNode } from '@/stores/topology'
import { useTopologyOverlayStore } from '@/stores/topologyOverlay'
import { formatPercent } from '@/utils/formatters'
import {
  EDGE_STATUS,
  edgeSpeedLabel,
  edgeStatusKey,
  edgeTrafficLabel,
  edgeUtilization,
  linkKindPresentation,
  nodeIcon,
  nodeIp,
  nodeStatusPresentation,
  nodeTypeLabel,
} from '@/utils/topology/presentation'

const props = defineProps<{
  node?: TopologyNode | null
  edge?: TopologyEdge | null
  x: number
  y: number
}>()

const overlay = useTopologyOverlayStore()
const nodeOverlayLabel = computed(() =>
  props.node ? (overlay.nodeOverlays[props.node.id]?.label ?? null) : null
)
const nodeStatus = computed(() => nodeStatusPresentation(props.node?.status))
const edgeStatus = computed(() =>
  props.edge ? EDGE_STATUS[edgeStatusKey(props.edge)] : EDGE_STATUS.unknown
)
const ip = computed(() => (props.node ? nodeIp(props.node) : null))
const traffic = computed(() => (props.edge ? edgeTrafficLabel(props.edge) : null))
const utilization = computed(() => (props.edge ? edgeUtilization(props.edge) : null))
const style = computed(() => ({ left: `${props.x}px`, top: `${props.y}px` }))
</script>

<style scoped>
.topology-tooltip {
  position: absolute;
  transform: translate(-50%, calc(-100% - 10px));
  pointer-events: none;
  z-index: 6;
  min-width: 180px;
  max-width: 300px;
  padding: 8px 10px;
  border-radius: 8px;
  background: rgb(var(--v-theme-surface));
  color: rgb(var(--v-theme-on-surface));
  border: 1px solid rgba(var(--v-theme-on-surface), 0.22);
  box-shadow: 0 6px 18px rgba(0, 0, 0, 0.3);
  font-size: 0.8rem;
}
.title {
  font-size: 0.85rem;
}
.meta {
  margin-top: 2px;
  display: flex;
  align-items: center;
  gap: 6px;
}
.mono {
  font-family: 'Roboto Mono', ui-monospace, monospace;
}
.status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}
</style>
