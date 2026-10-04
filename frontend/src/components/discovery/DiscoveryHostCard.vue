<template>
  <v-card
    variant="outlined"
    rounded="lg"
    class="host-card d-flex flex-column fill-height"
    :aria-label="`Detalhes de ${name}`"
    @click="emit('open', host)"
  >
    <div class="d-flex align-start ga-3 pa-3 pb-2">
      <v-avatar :color="type.color" size="44" variant="tonal" rounded="lg" class="flex-shrink-0">
        <v-icon :icon="type.icon" size="24"></v-icon>
      </v-avatar>

      <div class="flex-grow-1 min-w-0">
        <div class="d-flex align-start justify-space-between ga-2">
          <div class="text-subtitle-1 font-weight-bold text-truncate" :title="name">
            {{ name }}
          </div>
          <v-chip
            size="x-small"
            :color="confidenceColor"
            variant="flat"
            class="flex-shrink-0 font-weight-bold"
            :title="`Confiança na classificação: ${confidenceLabel}`"
          >
            {{ confidenceLabel }}
          </v-chip>
        </div>
        <div class="text-body-2 font-mono text-primary">{{ host.ipAddress }}</div>
        <div v-if="host.macAddress" class="text-caption font-mono">{{ host.macAddress }}</div>
      </div>
    </div>

    <div class="px-3 d-flex flex-wrap ga-1">
      <v-chip size="x-small" :color="type.color" variant="tonal" :prepend-icon="type.icon">
        {{ type.label }}
      </v-chip>
      <v-chip
        v-if="isGateway"
        size="x-small"
        color="primary"
        variant="flat"
        prepend-icon="mdi-router-network"
      >
        Gateway
      </v-chip>
      <v-chip v-if="snmpVersion !== null" size="x-small" color="teal" variant="tonal">
        SNMP {{ snmpVersion }}
      </v-chip>
      <v-chip
        v-if="added"
        size="x-small"
        color="success"
        variant="tonal"
        prepend-icon="mdi-check-circle"
      >
        Cadastrado
      </v-chip>
      <LayaSuggestionChip :suggestion="laya?.deviceType ?? null" :format-label="deviceTypeLabel" />
    </div>

    <div class="px-3 pt-2 flex-grow-1">
      <div v-if="description" class="text-body-2 host-card__clamp" :title="description">
        {{ description }}
      </div>
      <div class="text-caption mt-1">
        <v-icon size="14" class="me-1">mdi-domain</v-icon
        >{{ vendor ?? 'Fabricante não identificado' }}
      </div>
      <div v-if="reason" class="text-caption mt-1 text-info host-card__clamp" :title="reason">
        <v-icon size="14" class="me-1">mdi-lightbulb-on-outline</v-icon>{{ reason }}
      </div>
    </div>

    <div class="d-flex align-center ga-2 pa-3 pt-2">
      <span v-if="ports.length > 0" class="text-caption">
        <v-icon size="14" class="me-1">mdi-lan-connect</v-icon>{{ ports.length }} porta(s)
      </span>
      <v-spacer></v-spacer>
      <v-btn
        v-if="!added"
        size="small"
        color="success"
        variant="flat"
        prepend-icon="mdi-plus"
        @click.stop="emit('add', host)"
      >
        Adicionar
      </v-btn>
      <v-btn
        v-else
        size="small"
        color="primary"
        variant="tonal"
        append-icon="mdi-chevron-right"
        @click.stop="emit('open', host)"
      >
        Detalhes
      </v-btn>
    </div>
  </v-card>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import LayaSuggestionChip from '@/components/ai/LayaSuggestionChip.vue'
import type { StreamedDiscoveryHost } from '@/stores/discovery'
import { deviceTypeLabel } from '@/utils/deviceTypes'
import {
  discoveryConfidenceColor,
  discoveryDescription,
  discoveryDeviceName,
  discoveryLaya,
  discoveryOpenPorts,
  discoveryReasons,
  discoverySnmpVersion,
  discoveryTypeMeta,
  discoveryVendor,
} from '@/utils/discoveryPresentation'
import { formatPercent } from '@/utils/formatters'

const props = defineProps<{
  host: StreamedDiscoveryHost
  added: boolean
  isGateway: boolean
}>()

const emit = defineEmits<{
  open: [host: StreamedDiscoveryHost]
  add: [host: StreamedDiscoveryHost]
}>()

const type = computed(() => discoveryTypeMeta(props.host))
const name = computed(() => discoveryDeviceName(props.host) ?? type.value.label)
const description = computed(() => discoveryDescription(props.host))
const vendor = computed(() => discoveryVendor(props.host))
const reason = computed(() => discoveryReasons(props.host)[0] ?? null)
const ports = computed(() => discoveryOpenPorts(props.host))
const snmpVersion = computed(() => discoverySnmpVersion(props.host))
const laya = computed(() => discoveryLaya(props.host))
const confidenceColor = computed(() => discoveryConfidenceColor(props.host.confidence))
const confidenceLabel = computed(() => formatPercent(props.host.confidence, 0))
</script>

<style scoped>
.host-card {
  cursor: pointer;
  transition:
    border-color 0.15s ease,
    transform 0.15s ease;
}

.host-card:hover,
.host-card:focus-visible {
  border-color: rgb(var(--v-theme-primary));
  transform: translateY(-1px);
}

.host-card__clamp {
  display: -webkit-box;
  -webkit-line-clamp: 2;
  line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.min-w-0 {
  min-width: 0;
}
</style>
