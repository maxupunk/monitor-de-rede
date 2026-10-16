<template>
  <div class="pa-2">
    <div v-if="rows.length === 0" class="pa-6 text-center empty">
      <v-icon size="36" color="info" class="mb-2">mdi-lan-disconnect</v-icon>
      <div class="font-weight-bold">Nenhum enlace</div>
      <div class="text-body-2">Use a aba Ações para conectar este dispositivo.</div>
    </div>
    <button
      v-for="row in rows"
      :key="row.edge.id"
      type="button"
      class="link-row"
      :class="`status-${row.status}`"
      @click="host.selectEdge(row.edge.id)"
    >
      <div class="d-flex align-center ga-2">
        <v-icon size="18" :color="row.kind.color">{{ row.kind.icon }}</v-icon>
        <span class="font-weight-bold text-truncate flex-grow-1">{{ row.otherName }}</span>
        <v-chip :color="EDGE_STATUS[row.status].color" size="x-small" variant="flat" label>
          {{ EDGE_STATUS[row.status].label }}
        </v-chip>
      </div>
      <div v-if="row.localPort || row.remotePort" class="ports mono">
        {{ row.localPort || '—' }} → {{ row.remotePort || '—' }}
      </div>
      <div v-else class="ports">Portas não informadas</div>
      <TrafficSummary :edge="row.edge" />
    </button>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useTopologyStore } from '@/stores/topology'
import type { TopologyHost, TopologyTarget } from '@/utils/topology/inspectorRegistry'
import {
  EDGE_STATUS,
  edgeEndpoints,
  edgeStatusKey,
  linkKindPresentation,
} from '@/utils/topology/presentation'
import TrafficSummary from './TrafficSummary.vue'

const props = defineProps<{ target: TopologyTarget; host: TopologyHost }>()
const topology = useTopologyStore()

const STATUS_ORDER = { down: 0, unknown: 1, up: 2 } as const

const rows = computed(() => {
  if (props.target.kind !== 'node') return []
  const id = props.target.node.id
  return topology.edges
    .map((edge) => {
      const { source, target } = edgeEndpoints(edge)
      if (source !== id && target !== id) return null
      const outgoing = source === id
      const otherId = outgoing ? target : source
      return {
        edge,
        status: edgeStatusKey(edge),
        kind: linkKindPresentation(edge.linkType),
        otherName:
          topology.nodesById.get(otherId)?.name ??
          (outgoing ? edge.targetDeviceName : edge.sourceDeviceName) ??
          'Dispositivo',
        localPort: outgoing ? edge.sourceInterfaceName : edge.targetInterfaceName,
        remotePort: outgoing ? edge.targetInterfaceName : edge.sourceInterfaceName,
      }
    })
    .filter((row): row is NonNullable<typeof row> => row !== null)
    .sort(
      (a, b) =>
        STATUS_ORDER[a.status] - STATUS_ORDER[b.status] || a.otherName.localeCompare(b.otherName)
    )
})
</script>

<style scoped>
.link-row {
  display: block;
  width: 100%;
  text-align: left;
  padding: 10px 12px;
  margin-bottom: 6px;
  border-radius: 8px;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.16);
  border-left: 4px solid rgba(var(--v-theme-on-surface), 0.4);
  color: rgb(var(--v-theme-on-surface));
  background: rgb(var(--v-theme-surface));
}
.link-row:hover,
.link-row:focus-visible {
  border-color: rgb(var(--v-theme-primary));
  outline: none;
}
.link-row.status-down {
  border-left-color: rgb(var(--v-theme-error));
}
.link-row.status-up {
  border-left-color: rgb(var(--v-theme-success));
}
.link-row.status-unknown {
  border-left-color: rgb(var(--v-theme-info));
}
.ports {
  font-size: 0.78rem;
  margin: 4px 0 6px;
}
.mono {
  font-family: 'Roboto Mono', ui-monospace, monospace;
}
.empty {
  color: rgb(var(--v-theme-on-surface));
}
</style>
