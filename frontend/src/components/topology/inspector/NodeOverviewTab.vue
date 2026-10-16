<template>
  <div v-if="node" class="pa-4">
    <div class="stats mb-4">
      <div class="stat">
        <div class="stat-value">{{ links.length }}</div>
        <div class="stat-label">Enlaces</div>
      </div>
      <div class="stat" :class="{ alert: downCount > 0 }">
        <div class="stat-value">{{ downCount }}</div>
        <div class="stat-label">Fora do ar</div>
      </div>
      <div class="stat">
        <div class="stat-value">{{ node.interfaceCount }}</div>
        <div class="stat-label">Interfaces</div>
      </div>
    </div>

    <dl class="facts">
      <div class="fact">
        <dt>Endereço IP</dt>
        <dd class="mono">
          {{ ip || 'Não configurado' }}
          <v-btn
            v-if="ip"
            icon="mdi-content-copy"
            size="x-small"
            variant="text"
            color="primary"
            :aria-label="`Copiar ${ip}`"
            @click="copy(ip)"
          ></v-btn>
        </dd>
      </div>
      <div class="fact">
        <dt>Tipo</dt>
        <dd>{{ nodeTypeLabel(node) }}</dd>
      </div>
      <div v-if="node.vendor || node.model" class="fact">
        <dt>Fabricante / modelo</dt>
        <dd>{{ [node.vendor, node.model].filter(Boolean).join(' · ') }}</dd>
      </div>
      <div class="fact">
        <dt>Site</dt>
        <dd>{{ siteName }}</dd>
      </div>
      <div v-if="parent" class="fact">
        <dt>Atrás de</dt>
        <dd>
          <a href="#" class="link" @click.prevent="host.focusNode(parent.id)">{{ parent.name }}</a>
        </dd>
      </div>
      <div class="fact">
        <dt>SNMP</dt>
        <dd>{{ node.snmpEnabled ? 'Ativo' : 'Desligado' }}</dd>
      </div>
      <div v-if="device?.updatedAt" class="fact">
        <dt>Cadastro atualizado</dt>
        <dd>{{ formatRelativeTime(device.updatedAt) }}</dd>
      </div>
    </dl>

    <div class="d-flex flex-wrap ga-2 mt-4">
      <v-btn
        v-if="!unmanaged"
        color="primary"
        variant="flat"
        prepend-icon="mdi-open-in-new"
        @click="host.openDevice(node.id)"
      >
        Abrir ficha
      </v-btn>
      <v-btn
        color="info"
        variant="flat"
        prepend-icon="mdi-crosshairs-gps"
        @click="host.focusNode(node.id)"
      >
        Centralizar
      </v-btn>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useDevicesStore } from '@/stores/devices'
import { useSitesStore } from '@/stores/sites'
import { useTopologyStore } from '@/stores/topology'
import { formatRelativeTime } from '@/utils/formatters'
import type { TopologyHost, TopologyTarget } from '@/utils/topology/inspectorRegistry'
import {
  edgeEndpoints,
  edgeStatusKey,
  isUnmanagedSwitch,
  nodeIp,
  nodeTypeLabel,
} from '@/utils/topology/presentation'

const props = defineProps<{ target: TopologyTarget; host: TopologyHost }>()

const topology = useTopologyStore()
const sites = useSitesStore()
const devices = useDevicesStore()

const node = computed(() => (props.target.kind === 'node' ? props.target.node : null))
const ip = computed(() => (node.value ? nodeIp(node.value) : null))
const unmanaged = computed(() => (node.value ? isUnmanagedSwitch(node.value) : false))
const device = computed(() => devices.devices.find((item) => item.id === node.value?.id) ?? null)

const links = computed(() =>
  topology.edges.filter((edge) => {
    const { source, target } = edgeEndpoints(edge)
    return source === node.value?.id || target === node.value?.id
  })
)
const downCount = computed(
  () => links.value.filter((edge) => edgeStatusKey(edge) === 'down').length
)

const siteName = computed(() => {
  const current = node.value
  if (!current?.siteId) return 'Sem site'
  return (
    current.siteName ||
    sites.sites.find((site) => site.id === current.siteId)?.name ||
    `Site #${current.siteId}`
  )
})

const parent = computed(() =>
  node.value?.parentId ? (topology.nodesById.get(node.value.parentId) ?? null) : null
)

function copy(text: string) {
  void navigator.clipboard?.writeText(text)
}
</script>

<style scoped>
.stats {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
}
.stat {
  border: 1px solid rgba(var(--v-theme-on-surface), 0.16);
  border-radius: 8px;
  padding: 8px;
  text-align: center;
}
.stat.alert {
  border-color: rgb(var(--v-theme-error));
  background: rgba(var(--v-theme-error), 0.12);
}
.stat-value {
  font-size: 1.35rem;
  font-weight: 700;
  line-height: 1.2;
}
.stat-label {
  font-size: 0.72rem;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}
.facts {
  margin: 0;
}
.fact {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  padding: 7px 0;
  border-bottom: 1px solid rgba(var(--v-theme-on-surface), 0.1);
}
dt {
  font-weight: 600;
  font-size: 0.82rem;
}
dd {
  margin: 0;
  text-align: right;
  font-size: 0.85rem;
  display: flex;
  align-items: center;
  gap: 2px;
}
.mono {
  font-family: 'Roboto Mono', ui-monospace, monospace;
}
.link {
  color: rgb(var(--v-theme-primary));
  font-weight: 600;
}
</style>
