<template>
  <div v-if="edge" class="pa-4">
    <div class="endpoints mb-4">
      <button type="button" class="endpoint" @click="host.focusNode(ends.source)">
        <span class="badge">A</span>
        <span class="flex-grow-1 overflow-hidden">
          <span class="d-block font-weight-bold text-truncate">
            {{ edge.sourceDeviceName || 'Origem' }}
          </span>
          <span class="d-flex align-center ga-1 port mono">
            <span
              class="dot"
              :style="{
                background: `rgb(var(--v-theme-${portColor(edge.sourceInterfaceStatus)}))`,
              }"
            ></span>
            {{ edge.sourceInterfaceName || 'Porta não definida' }}
          </span>
        </span>
      </button>
      <div class="link-line">
        <span class="rail"></span>
        <v-icon :color="kind.color" size="18">{{ kind.icon }}</v-icon>
        <span class="link-caption">{{ kind.label }} · {{ edgeSpeedLabel(edge) }}</span>
      </div>
      <button type="button" class="endpoint" @click="host.focusNode(ends.target)">
        <span class="badge">B</span>
        <span class="flex-grow-1 overflow-hidden">
          <span class="d-block font-weight-bold text-truncate">
            {{ edge.targetDeviceName || 'Destino' }}
          </span>
          <span class="d-flex align-center ga-1 port mono">
            <span
              class="dot"
              :style="{
                background: `rgb(var(--v-theme-${portColor(edge.targetInterfaceStatus)}))`,
              }"
            ></span>
            {{ edge.targetInterfaceName || 'Porta não definida' }}
          </span>
        </span>
      </button>
    </div>

    <div class="section-title">Tráfego ao vivo</div>
    <TrafficSummary :edge="edge" class="mb-4" />

    <div class="section-title">Origem do enlace</div>
    <div class="d-flex flex-wrap ga-2 mb-2">
      <v-chip color="primary" size="small" variant="flat" :prepend-icon="kind.icon" label>
        {{ kind.label }}
      </v-chip>
      <v-chip color="info" size="small" variant="flat" prepend-icon="mdi-radar" label>
        {{ discoveryLabel(edge.discoveryMethod) }}
      </v-chip>
      <v-chip
        v-if="confidence !== null && !virtual"
        :color="confidence >= 80 ? 'success' : confidence >= 50 ? 'warning' : 'error'"
        size="small"
        variant="flat"
        prepend-icon="mdi-gauge"
        label
      >
        Confiança {{ confidence }}%
      </v-chip>
      <v-chip
        v-if="edge.confirmed && !virtual"
        color="success"
        size="small"
        variant="flat"
        prepend-icon="mdi-check-decagram"
        label
      >
        Confirmado
      </v-chip>
      <v-chip
        v-if="edge.stale"
        color="warning"
        size="small"
        variant="flat"
        prepend-icon="mdi-clock-alert"
        label
      >
        Não visto {{ edge.lastSeenAt ? formatRelativeTime(edge.lastSeenAt) : 'recentemente' }}
      </v-chip>
    </div>

    <v-alert
      v-if="virtual"
      type="info"
      variant="tonal"
      density="compact"
      icon="mdi-sitemap"
      class="mt-3"
    >
      Ligação vinda do cadastro ("está atrás de"). Edite para registrar o cabo real com as portas.
    </v-alert>
    <v-alert v-else-if="confirmable" type="warning" variant="tonal" density="compact" class="mt-3">
      <div class="mb-2">
        Enlace {{ edge.discoveryMethod === 'subnet_inference' ? 'inferido' : 'descoberto' }} e ainda
        não confirmado. Confirme se ele existe de verdade.
      </div>
      <v-tooltip :text="READ_ONLY_REASON" :disabled="host.canWrite">
        <template #activator="{ props: tip }">
          <span v-bind="tip">
            <v-btn
              color="success"
              variant="flat"
              size="small"
              prepend-icon="mdi-check-decagram"
              :disabled="!host.canWrite"
              @click="host.confirmEdge(edge)"
            >
              Confirmar enlace
            </v-btn>
          </span>
        </template>
      </v-tooltip>
    </v-alert>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { formatRelativeTime } from '@/utils/formatters'
import {
  READ_ONLY_REASON,
  type TopologyHost,
  type TopologyTarget,
} from '@/utils/topology/inspectorRegistry'
import {
  canConfirmEdge,
  confidencePercent,
  discoveryLabel,
  edgeEndpoints,
  edgeSpeedLabel,
  isVirtualEdge,
  linkKindPresentation,
} from '@/utils/topology/presentation'
import TrafficSummary from './TrafficSummary.vue'

const props = defineProps<{ target: TopologyTarget; host: TopologyHost }>()

const edge = computed(() => (props.target.kind === 'edge' ? props.target.edge : null))
const ends = computed(() => (edge.value ? edgeEndpoints(edge.value) : { source: 0, target: 0 }))
const kind = computed(() => linkKindPresentation(edge.value?.linkType))
const virtual = computed(() => (edge.value ? isVirtualEdge(edge.value) : false))
const confirmable = computed(() => (edge.value ? canConfirmEdge(edge.value) : false))
const confidence = computed(() => confidencePercent(edge.value?.confidence))

function portColor(status?: string | null): 'success' | 'error' | 'info' {
  if (status === 'up') return 'success'
  if (status === 'down') return 'error'
  return 'info'
}
</script>

<style scoped>
.endpoints {
  display: flex;
  flex-direction: column;
  align-items: stretch;
}
.endpoint {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  text-align: left;
  padding: 8px 10px;
  border-radius: 8px;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.18);
  color: rgb(var(--v-theme-on-surface));
  background: rgb(var(--v-theme-surface));
}
.endpoint:hover,
.endpoint:focus-visible {
  border-color: rgb(var(--v-theme-primary));
  outline: none;
}
.badge {
  width: 26px;
  height: 26px;
  border-radius: 50%;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-weight: 700;
  font-size: 0.8rem;
  background: rgb(var(--v-theme-primary));
  color: #fff;
  flex-shrink: 0;
}
.link-line {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 30px;
  padding-left: 22px;
}
.rail {
  align-self: stretch;
  border-left: 2px dashed rgba(var(--v-theme-on-surface), 0.45);
  margin-right: 6px;
}
.link-caption {
  font-size: 0.78rem;
  font-weight: 600;
}
.port {
  font-size: 0.78rem;
}
.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
}
.mono {
  font-family: 'Roboto Mono', ui-monospace, monospace;
}
.section-title {
  font-size: 0.72rem;
  font-weight: 700;
  letter-spacing: 0.05em;
  text-transform: uppercase;
  margin-bottom: 6px;
  color: rgb(var(--v-theme-on-surface));
}
</style>
