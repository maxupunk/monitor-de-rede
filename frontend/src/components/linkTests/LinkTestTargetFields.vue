<template>
  <div class="target-fields">
    <div id="lt-kind-label" class="field-label text-title-small font-weight-bold mb-2">
      O que medir
    </div>
    <v-item-group
      :model-value="modelValue.kind"
      mandatory
      class="kinds mb-4"
      role="radiogroup"
      aria-labelledby="lt-kind-label"
      @update:model-value="setKind"
    >
      <v-item v-for="option in KIND_OPTIONS" :key="option.value" :value="option.value">
        <template #default="{ isSelected, toggle }">
          <v-card
            class="kind-card"
            :class="{ selected: isSelected }"
            :color="isSelected ? option.color : undefined"
            :variant="isSelected ? 'flat' : 'outlined'"
            role="radio"
            :aria-checked="isSelected"
            :disabled="readonly"
            @click="toggle"
          >
            <v-card-text class="d-flex align-start ga-3">
              <v-icon size="28" :color="isSelected ? 'white' : option.color">
                {{ option.icon }}
              </v-icon>
              <div>
                <div class="text-title-small font-weight-bold">{{ option.label }}</div>
                <div class="text-body-small mt-1">{{ option.description }}</div>
              </div>
            </v-card-text>
          </v-card>
        </template>
      </v-item>
    </v-item-group>

    <p v-if="hiddenCount > 0" class="transparent-hint text-body-small mb-3">
      <v-icon size="16" color="info">mdi-information</v-icon>
      <span>{{ TRANSPARENT_HINT }}</span>
    </p>

    <template v-if="modelValue.kind === 'link'">
      <v-autocomplete
        :model-value="modelValue.sourceDeviceId"
        :items="deviceItems"
        item-props
        label="Equipamento de uma das pontas"
        prepend-inner-icon="mdi-router-network"
        variant="outlined"
        density="comfortable"
        :readonly="readonly"
        clearable
        @update:model-value="setLinkDevice"
      ></v-autocomplete>
      <v-select
        :model-value="selectedKey"
        :items="edgeItems"
        item-props
        label="Enlace"
        prepend-inner-icon="mdi-lan"
        variant="outlined"
        density="comfortable"
        :readonly="readonly"
        :disabled="modelValue.sourceDeviceId === null"
        :loading="topologyLoading"
        :no-data-text="
          modelValue.sourceDeviceId === null
            ? 'Escolha o equipamento primeiro'
            : 'Este equipamento não tem enlaces testáveis no mapa'
        "
        :hint="linkHint"
        persistent-hint
        :list-props="TARGET_LIST_PROPS"
        @update:model-value="chooseTarget"
      ></v-select>
    </template>

    <template v-else-if="modelValue.kind === 'path'">
      <div class="d-flex align-start ga-2 flex-wrap flex-sm-nowrap">
        <v-autocomplete
          :model-value="modelValue.sourceDeviceId"
          :items="deviceItems"
          item-props
          label="Origem"
          prepend-inner-icon="mdi-ray-start"
          variant="outlined"
          density="comfortable"
          :readonly="readonly"
          clearable
          class="flex-1-1-0"
          @update:model-value="(value) => patch({ sourceDeviceId: value ?? null })"
        ></v-autocomplete>
        <v-btn
          icon="mdi-swap-horizontal"
          color="primary"
          variant="flat"
          size="small"
          class="mt-2 swap"
          aria-label="Inverter origem e destino"
          :disabled="readonly"
          @click="swap"
        ></v-btn>
        <v-autocomplete
          :model-value="modelValue.targetDeviceId"
          :items="deviceItems"
          item-props
          label="Destino"
          prepend-inner-icon="mdi-ray-end"
          variant="outlined"
          density="comfortable"
          :readonly="readonly"
          clearable
          class="flex-1-1-0"
          @update:model-value="(value) => patch({ targetDeviceId: value ?? null })"
        ></v-autocomplete>
      </div>
    </template>

    <template v-else>
      <div class="origin mb-4">
        <v-icon color="info">mdi-monitor-dashboard</v-icon>
        <div>
          <div class="text-title-small font-weight-bold">Origem: servidor NetMonitor</div>
          <div class="text-body-small">
            Quando o destino só é alcançável por um agente remoto, o agente do site mede no lugar do
            servidor.
          </div>
        </div>
      </div>
      <v-autocomplete
        :model-value="modelValue.targetDeviceId"
        :items="deviceItems.filter((item) => !item.system)"
        item-props
        label="Até qual equipamento"
        prepend-inner-icon="mdi-ray-end"
        variant="outlined"
        density="comfortable"
        :readonly="readonly"
        clearable
        @update:model-value="(value) => patch({ targetDeviceId: value ?? null })"
      ></v-autocomplete>
    </template>
  </div>
</template>

<script setup lang="ts">
/**
 * Passo "O que testar": tipo do teste e as pontas. Usado pelo assistente e
 * pelo diálogo de agendamento (mesmo alvo, mesmas regras).
 */
import { computed, onMounted, ref, watch } from 'vue'
import { useDevicesStore } from '@/stores/devices'
import { useTopologyStore, type TopologyEdge } from '@/stores/topology'
import type { LinkTestKind } from '@/types/linkTests'
import { formatLinkSpeed } from '@/utils/formatters'
import { KIND_OPTIONS, viaText } from '@/utils/linkTests/presentation'
import {
  TRANSPARENT_HINT,
  isLinkTestEndpoint,
  linkTargets,
  selectedTargetKey,
  type LinkTarget,
} from '@/utils/linkTests/targets'
import { linkOptionSubtitle, type LinkTestTargetDraft } from '@/utils/linkTests/wizard'
import { edgeEndpoints, edgeSpeed } from '@/utils/topology/presentation'

const props = defineProps<{
  modelValue: LinkTestTargetDraft
  readonly?: boolean
}>()

const emit = defineEmits<{ 'update:modelValue': [value: LinkTestTargetDraft] }>()

const devices = useDevicesStore()
const topology = useTopologyStore()
const topologyLoading = ref(false)

function patch(change: Partial<LinkTestTargetDraft>) {
  emit('update:modelValue', { ...props.modelValue, ...change })
}

function setKind(kind: unknown) {
  if (kind !== 'link' && kind !== 'path' && kind !== 'server') return
  const next: LinkTestKind = kind
  if (next === props.modelValue.kind) return
  // O destino escolhido continua valendo entre caminho e servidor.
  patch({ kind: next, linkId: null })
}

function swap() {
  patch({
    sourceDeviceId: props.modelValue.targetDeviceId,
    targetDeviceId: props.modelValue.sourceDeviceId,
  })
}

/**
 * O subtítulo da opção ("passando por switch-a › switch-b") é parte da
 * escolha, não um detalhe: opacidade cheia, e ele quebra linha em vez de
 * sumir na tela estreita.
 */
const TARGET_LIST_PROPS = { style: { '--v-list-item-subtitle-opacity': '1' } }

const hiddenCount = computed(
  () => devices.devices.filter((device) => !isLinkTestEndpoint(device)).length
)

const deviceItems = computed(() =>
  devices.devices
    .filter(isLinkTestEndpoint)
    .map((device) => ({
      title: device.name,
      subtitle: device.isSystem ? 'Servidor NetMonitor' : (device.ipAddress ?? ''),
      value: device.id,
      system: Boolean(device.isSystem),
    }))
    .sort((a, b) => a.title.localeCompare(b.title, 'pt-BR'))
)

function deviceName(id: number): string {
  return (
    topology.nodesById.get(id)?.name ??
    devices.devices.find((device) => device.id === id)?.name ??
    `#${id}`
  )
}

function edgeSubtitle(edge: TopologyEdge): string {
  const speed = edgeSpeed(edge)
  return linkOptionSubtitle(
    edge.sourceInterfaceName,
    edge.targetInterfaceName,
    speed ? formatLinkSpeed(speed) : null
  )
}

/** Vizinhos com IP e, atrás de switches sem IP, os equipamentos com IP. */
const targets = computed<LinkTarget[]>(() => {
  const from = props.modelValue.sourceDeviceId
  if (from === null) return []
  return linkTargets(from, (id) => topology.nodesById.get(id), topology.edges)
})

function targetSubtitle(target: LinkTarget): string {
  const via = viaText(target.viaNames)
  if (via) return `passando por ${via}`
  const edge = target.linkId !== null ? topology.edgesById.get(target.linkId) : undefined
  return edge ? edgeSubtitle(edge) : ''
}

const edgeItems = computed(() =>
  targets.value.map((target) => ({
    title: `até ${target.name}`,
    subtitle: targetSubtitle(target),
    value: target.key,
    prependIcon: target.viaNames.length > 0 ? 'mdi-switch' : 'mdi-lan',
  }))
)

const selectedKey = computed(() => selectedTargetKey(props.modelValue, targets.value))

const linkHint = computed(() => {
  if (props.modelValue.sourceDeviceId === null) {
    return 'Escolha o equipamento para ver os enlaces dele.'
  }
  const chosen = targets.value.find((target) => target.key === selectedKey.value)
  const via = chosen ? viaText(chosen.viaNames) : null
  return via
    ? `Mede os cabos entre ${deviceName(props.modelValue.sourceDeviceId)} e ${chosen?.name}, passando por ${via} (sem IP).`
    : ''
})

function setLinkDevice(value: number | null) {
  patch({ sourceDeviceId: value ?? null, targetDeviceId: null, linkId: null })
}

function chooseTarget(key: string | null) {
  const from = props.modelValue.sourceDeviceId
  const chosen = key !== null ? targets.value.find((target) => target.key === key) : undefined
  if (!chosen || from === null) {
    patch({ linkId: null, targetDeviceId: null })
    return
  }
  patch({ linkId: chosen.linkId, sourceDeviceId: from, targetDeviceId: chosen.deviceId })
}

/** Os enlaces vêm do mapa: fora da topologia, carrega o grafo sob demanda. */
async function ensureTopology() {
  if (topology.loaded || topologyLoading.value) return
  topologyLoading.value = true
  try {
    await topology.fetchTopology(null, false)
  } finally {
    topologyLoading.value = false
  }
}

/** Pedido só com o enlace (ex.: "repetir"): completa a ponta pelo mapa. */
function completeFromLink() {
  const { linkId, sourceDeviceId } = props.modelValue
  if (linkId === null || sourceDeviceId !== null) return
  const edge = topology.edgesById.get(linkId)
  if (!edge) return
  const { source, target } = edgeEndpoints(edge)
  patch({ sourceDeviceId: source, targetDeviceId: target })
}

watch(
  () => props.modelValue.kind,
  (kind) => {
    if (kind === 'link') void ensureTopology().then(completeFromLink)
  },
  { immediate: true }
)

onMounted(() => {
  if (devices.devices.length === 0) void devices.fetchDevices()
})
</script>

<style scoped>
.kinds {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 12px;
}
.kind-card {
  cursor: pointer;
  height: 100%;
}
.kind-card:not(.selected) {
  border-color: rgba(var(--v-theme-on-surface), 0.3);
}
.field-label {
  color: rgb(var(--v-theme-on-surface));
}
.origin {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  padding: 12px;
  border-radius: 10px;
  border: 1px solid rgb(var(--v-theme-info));
  color: rgb(var(--v-theme-on-surface));
}
.swap {
  align-self: flex-start;
}
.transparent-hint {
  display: flex;
  align-items: center;
  gap: 6px;
  color: rgb(var(--v-theme-on-surface));
}
</style>
