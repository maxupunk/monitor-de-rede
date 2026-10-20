<template>
  <div class="target-fields">
    <div class="ends">
      <v-autocomplete
        :model-value="source"
        :items="sourceItems"
        item-props
        label="De"
        prepend-inner-icon="mdi-ray-start"
        variant="outlined"
        density="comfortable"
        :readonly="readonly"
        hide-details
        class="end"
        @update:model-value="setSource"
      ></v-autocomplete>
      <v-btn
        icon="mdi-swap-horizontal"
        color="primary"
        variant="flat"
        size="small"
        class="swap"
        aria-label="Inverter origem e destino"
        :disabled="readonly || source === null || modelValue.targetDeviceId === null"
        @click="swap"
      ></v-btn>
      <v-autocomplete
        :model-value="selectedKey"
        :items="destinationItems"
        item-props
        label="Até"
        prepend-inner-icon="mdi-ray-end"
        variant="outlined"
        density="comfortable"
        :readonly="readonly"
        :disabled="source === null"
        :loading="topologyLoading"
        :no-data-text="
          source === null ? 'Escolha de onde medir primeiro' : 'Nenhum equipamento para medir daqui'
        "
        :list-props="LIST_PROPS"
        hide-details
        class="end"
        @update:model-value="chooseDestination"
      ></v-autocomplete>
    </div>

    <p v-if="hiddenCount > 0" class="hint text-body-small mt-2">
      <v-icon size="16" color="info">mdi-information</v-icon>
      <span>{{ TRANSPARENT_HINT }}</span>
    </p>

    <template v-if="destination">
      <div v-if="!destination.direct" class="mode mt-4">
        <span id="lt-mode-label" class="text-title-small font-weight-bold">Como medir</span>
        <v-btn-toggle
          :model-value="mode"
          mandatory
          color="primary"
          variant="outlined"
          density="comfortable"
          divided
          aria-labelledby="lt-mode-label"
          :disabled="readonly"
          @update:model-value="setMode"
        >
          <v-btn value="end_to_end" prepend-icon="mdi-arrow-expand-horizontal">Ponta a ponta</v-btn>
          <v-btn value="per_hop" prepend-icon="mdi-transit-connection-variant">
            Enlace por enlace
          </v-btn>
        </v-btn-toggle>
      </div>

      <section class="what mt-4" aria-live="polite">
        <div class="d-flex align-start ga-3 mb-3">
          <v-avatar :color="kindInfo.color" size="36" rounded="lg">
            <v-icon color="white" size="20">{{ kindInfo.icon }}</v-icon>
          </v-avatar>
          <div>
            <div class="text-title-small font-weight-bold">{{ kindInfo.label }}</div>
            <div class="text-body-medium">{{ explanation }}</div>
          </div>
        </div>
        <LinkTestPathMap
          :node-ids="destination.deviceIds"
          :edge-ids="destination.edgeIds"
          :height="smAndDown ? verticalMapHeight(destination.deviceIds.length) : 220"
          :vertical="smAndDown"
        />
      </section>
    </template>
  </div>
</template>

<script setup lang="ts">
/**
 * Passo "O que testar": **De** e **Até**. O vizinho direto mede o enlace; os
 * demais, ponta a ponta (ou enlace por enlace, para achar o gargalo). O tipo
 * do teste sai da escolha (`utils/linkTests/destinations`), nunca de um
 * cartão a mais. Usado pelo assistente e pelo agendamento (mesmo alvo,
 * mesmas regras).
 */
import { computed, onMounted, ref, watch } from 'vue'
import { useDisplay } from 'vuetify'
import { useDevicesStore } from '@/stores/devices'
import { useTopologyStore } from '@/stores/topology'
import {
  destinationSubtitle,
  destinationsFrom,
  measureKind,
  modeOfKind,
  type Destination,
  type DestinationNode,
  type MeasureMode,
} from '@/utils/linkTests/destinations'
import { verticalMapHeight } from '@/utils/linkTests/pathMap'
import { TEST_KINDS, viaText } from '@/utils/linkTests/presentation'
import { TRANSPARENT_HINT, isLinkTestEndpoint } from '@/utils/linkTests/targets'
import type { LinkTestTargetDraft } from '@/utils/linkTests/wizard'
import LinkTestPathMap from './LinkTestPathMap.vue'

const props = defineProps<{
  modelValue: LinkTestTargetDraft
  readonly?: boolean
}>()

const emit = defineEmits<{ 'update:modelValue': [value: LinkTestTargetDraft] }>()

/** O subtítulo (enlace direto, saltos, por onde passa) é parte da escolha. */
const LIST_PROPS = { style: { '--v-list-item-subtitle-opacity': '1' } }

const { smAndDown } = useDisplay()
const devices = useDevicesStore()
const topology = useTopologyStore()
const topologyLoading = ref(false)
const mode = ref<MeasureMode>(modeOfKind(props.modelValue.kind))

const systemId = computed(
  () =>
    devices.devices.find((device) => device.isSystem)?.id ??
    topology.nodes.find((node) => node.isSystem)?.id ??
    null
)

/** O teste do servidor não guarda a origem: ela é o próprio servidor. */
const source = computed(() =>
  props.modelValue.sourceDeviceId !== null
    ? props.modelValue.sourceDeviceId
    : props.modelValue.kind === 'server'
      ? systemId.value
      : null
)

const endpoints = computed<Array<DestinationNode & { type: string }>>(() =>
  devices.devices.filter(isLinkTestEndpoint).map((device) => ({
    id: device.id,
    name: device.name,
    ipAddress: device.ipAddress,
    isSystem: device.isSystem,
    type: device.type,
  }))
)

const hiddenCount = computed(
  () => devices.devices.filter((device) => !isLinkTestEndpoint(device)).length
)

const sourceItems = computed(() => {
  const items = endpoints.value.map((device) => ({
    title: device.isSystem ? 'Servidor NetMonitor' : device.name,
    subtitle: device.isSystem ? device.name : (device.ipAddress ?? ''),
    value: device.id,
    prependIcon: device.isSystem ? 'mdi-monitor-dashboard' : 'mdi-router-network',
    system: Boolean(device.isSystem),
  }))
  return items.sort(
    (a, b) => Number(b.system) - Number(a.system) || a.title.localeCompare(b.title, 'pt-BR')
  )
})

function nodeOf(id: number): DestinationNode | undefined {
  return topology.nodesById.get(id) ?? endpoints.value.find((device) => device.id === id)
}

function canBeEndpoint(node: DestinationNode): boolean {
  const device = devices.devices.find((item) => item.id === node.id)
  return device ? isLinkTestEndpoint(device) : !node.isSystem && Boolean(node.ipAddress)
}

function destinationsOf(from: number | null): Destination[] {
  if (from === null) return []
  return destinationsFrom(from, nodeOf, topology.edges, endpoints.value, canBeEndpoint)
}

const destinations = computed(() => destinationsOf(source.value))

function keyOf(destination: Pick<Destination, 'direct' | 'linkId' | 'deviceId'>): string {
  return destination.direct && destination.linkId !== null
    ? `edge:${destination.linkId}`
    : `device:${destination.deviceId}`
}

const destinationItems = computed(() =>
  destinations.value.map((destination) => ({
    title: destination.name,
    subtitle: destinationSubtitle(destination, (names) => viaText(names) ?? ''),
    value: keyOf(destination),
    prependIcon: destination.direct ? 'mdi-lan' : 'mdi-arrow-expand-horizontal',
  }))
)

/** A opção escolhida: o enlace, quando o pedido traz um; senão, o destino. */
const destination = computed<Destination | null>(() => {
  const { linkId, targetDeviceId } = props.modelValue
  if (linkId !== null) {
    const byEdge = destinations.value.find((item) => item.direct && item.linkId === linkId)
    if (byEdge) return byEdge
  }
  if (targetDeviceId === null) return null
  return (
    destinations.value.find((item) => item.deviceId === targetDeviceId && item.direct) ??
    destinations.value.find((item) => item.deviceId === targetDeviceId) ??
    null
  )
})

const selectedKey = computed(() => (destination.value ? keyOf(destination.value) : null))

const kindInfo = computed(() => TEST_KINDS[props.modelValue.kind])

function nameOf(id: number | null): string {
  if (id === null) return '?'
  if (id === systemId.value) return 'o servidor NetMonitor'
  return nodeOf(id)?.name ?? `#${id}`
}

const explanation = computed(() => {
  const chosen = destination.value
  if (!chosen) return ''
  const from = nameOf(source.value)
  const via = viaText(chosen.viaNames)
  if (chosen.direct) {
    return via
      ? `Mede só o enlace entre ${from} e ${chosen.name}, passando por ${via} (sem IP).`
      : `Mede só o enlace entre ${from} e ${chosen.name}.`
  }
  if (props.modelValue.kind === 'path') {
    return `Mede, um de cada vez, os ${chosen.edgeIds.length} enlaces entre ${from} e ${chosen.name} — mostra onde está o gargalo.`
  }
  const through = via ? `, atravessando ${via}` : ''
  return `Uma medição de ${from} até ${chosen.name}${through} — a velocidade que quem usa a rede sente.`
})

function draftFor(from: number | null, chosen: Destination | null): LinkTestTargetDraft {
  if (from === null || !chosen) {
    return {
      kind: from !== null && from === systemId.value ? 'server' : 'end_to_end',
      linkId: null,
      sourceDeviceId: from,
      targetDeviceId: null,
    }
  }
  const { kind, linkId } = measureKind(chosen, from === systemId.value, mode.value)
  return { kind, linkId, sourceDeviceId: from, targetDeviceId: chosen.deviceId }
}

function update(next: LinkTestTargetDraft) {
  emit('update:modelValue', next)
}

function setSource(value: number | null) {
  update(draftFor(value ?? null, null))
}

function chooseDestination(key: string | null) {
  const chosen = destinationItems.value.findIndex((item) => item.value === key)
  update(draftFor(source.value, chosen >= 0 ? destinations.value[chosen] : null))
}

function setMode(value: MeasureMode) {
  mode.value = value
  update(draftFor(source.value, destination.value))
}

/** Inverte as pontas: o destino vira origem e o caminho é recalculado. */
function swap() {
  const from = props.modelValue.targetDeviceId
  const to = source.value
  if (from === null || to === null) return
  const chosen = destinationsOf(from).find((item) => item.deviceId === to) ?? null
  update(draftFor(from, chosen))
}

watch(
  () => props.modelValue.kind,
  (kind) => {
    if (kind === 'path' || kind === 'end_to_end' || kind === 'server') mode.value = modeOfKind(kind)
  }
)

/** Os destinos vêm do mapa: fora da topologia, carrega o grafo sob demanda. */
async function ensureTopology() {
  if (topology.loaded || topologyLoading.value) return
  topologyLoading.value = true
  try {
    await topology.fetchTopology(null, false)
  } finally {
    topologyLoading.value = false
  }
}

onMounted(() => {
  if (devices.devices.length === 0) void devices.fetchDevices()
  void ensureTopology()
})
</script>

<style scoped>
.ends {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
  align-items: center;
  gap: 8px;
}
@media (max-width: 600px) {
  .ends {
    grid-template-columns: minmax(0, 1fr);
  }
  .swap {
    justify-self: center;
    transform: rotate(90deg);
  }
}
.mode {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 12px;
  color: rgb(var(--v-theme-on-surface));
}
.what {
  padding: 12px;
  border-radius: 12px;
  border: 1px solid rgb(var(--v-theme-primary));
  color: rgb(var(--v-theme-on-surface));
}
.hint {
  display: flex;
  align-items: center;
  gap: 6px;
  color: rgb(var(--v-theme-on-surface));
}
</style>
