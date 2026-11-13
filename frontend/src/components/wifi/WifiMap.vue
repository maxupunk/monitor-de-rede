<template>
  <div class="d-flex flex-column ga-4">
    <div class="map-layout" :class="{ 'with-panel': selectedRow }">
      <div class="canvas-wrap">
        <div
          ref="container"
          class="canvas"
          role="img"
          :aria-label="ariaLabel"
          data-testid="wifi-map-canvas"
        ></div>

        <div class="controls">
          <v-btn
            icon="mdi-plus"
            size="small"
            color="primary"
            variant="flat"
            aria-label="Aproximar"
            @click="map.zoomIn()"
          ></v-btn>
          <v-btn
            icon="mdi-minus"
            size="small"
            color="primary"
            variant="flat"
            aria-label="Afastar"
            @click="map.zoomOut()"
          ></v-btn>
          <v-btn
            icon="mdi-fit-to-screen-outline"
            size="small"
            color="primary"
            variant="flat"
            aria-label="Enquadrar tudo"
            @click="map.fit()"
          ></v-btn>
        </div>

        <div class="live-chip">
          <v-chip size="small" color="success" variant="flat" prepend-icon="mdi-broadcast">
            Ao vivo · a cada {{ liveInterval }} s
          </v-chip>
        </div>

        <div v-if="recentRoams.length" class="roam-feed" aria-live="polite">
          <div class="roam-feed-title">Trocas de AP agora</div>
          <div v-for="event in recentRoams" :key="event.id" class="roam-line">
            <v-icon size="16" :color="event.pingPong ? 'warning' : 'info'">
              mdi-swap-horizontal
            </v-icon>
            <span class="font-weight-bold">{{ roamName(event.mac) }}</span>
            <span>{{ roamRoute(event) }}</span>
            <span class="roam-time">{{ formatClockTime(event.occurredAt) }}</span>
          </div>
        </div>

        <div
          v-if="hoveredRow"
          class="map-tooltip"
          :style="{ left: `${hover?.x ?? 0}px`, top: `${hover?.y ?? 0}px` }"
          role="tooltip"
        >
          <div class="font-weight-bold">{{ tooltipName }}</div>
          <div>{{ hoveredRow.deviceName }} · {{ hoveredRow.ssid ?? 'rede sem nome' }}</div>
          <div>
            Sinal {{ hoveredRow.signalDbm }} dBm
            <template v-if="hoveredRow.snrDb !== null"> · SNR {{ hoveredRow.snrDb }} dB</template>
          </div>
          <div>Distância {{ distanceText(hoveredRow) }}</div>
          <div class="font-weight-bold" :class="`text-${HEALTH[hoveredRow.health].color}`">
            {{ HEALTH[hoveredRow.health].label }}
          </div>
        </div>
        <div
          v-else-if="hoveredAp"
          class="map-tooltip"
          :style="{ left: `${hover?.x ?? 0}px`, top: `${hover?.y ?? 0}px` }"
          role="tooltip"
        >
          <div class="font-weight-bold">{{ hoveredAp.deviceName }}</div>
          <div>
            {{ countNoun(hoveredAp.stations.length, 'aparelho conectado', 'aparelhos conectados') }}
          </div>
          <div class="font-weight-bold" :class="`text-${AP_STATUS[hoveredAp.status].color}`">
            {{ AP_STATUS[hoveredAp.status].label }}
          </div>
          <div v-if="hoveredAp.error">{{ hoveredAp.error }}</div>
          <div v-else-if="hoveredAp.readAt">Lido {{ formatRelativeTime(hoveredAp.readAt) }}</div>
        </div>

        <div v-if="emptyText" class="empty-overlay">
          <v-icon size="40" color="primary">mdi-access-point-network</v-icon>
          <div class="text-body-1 font-weight-medium">{{ emptyText }}</div>
        </div>
      </div>

      <WifiMapStationPanel
        v-if="selectedRow"
        :row="selectedRow"
        :retention-days="settings.settings?.roamingRetentionDays"
        @close="selected = null"
      />
    </div>

    <WifiMapLegend />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import type { WifiDeviceSnapshot } from '@/bindings/WifiDeviceSnapshot'
import type { WifiReadStatus } from '@/bindings/WifiReadStatus'
import { useTopologyPalette } from '@/composables/topology/useTopologyPalette'
import { useLiveInterest } from '@/composables/useLiveInterest'
import { useWifiMap } from '@/composables/wifi/useWifiMap'
import { useWifiClientsStore } from '@/stores/wifiClients'
import { useWifiSettingsStore } from '@/stores/wifiSettings'
import { countNoun, formatClockTime, formatRelativeTime } from '@/utils/formatters'
import { buildWifiMapElements } from '@/utils/wifiMap/elements'
import { dedupeByMac, layoutWifiMap, type MapAp } from '@/utils/wifiMap/layout'
import {
  distanceText,
  HEALTH,
  roamRoute,
  shortStationName,
  stationName,
  type WifiStationRow,
} from '@/utils/wifiPresentation'
import WifiMapLegend from './WifiMapLegend.vue'
import WifiMapStationPanel from './WifiMapStationPanel.vue'

const props = defineProps<{
  /** As estações que passaram nos filtros da tela. */
  rows: WifiStationRow[]
  /** Os APs a desenhar (o filtro de roteador já aplicado). */
  devices: WifiDeviceSnapshot[]
}>()

/** O aparelho aberto no painel, pelo MAC. */
const selected = defineModel<string | null>('selected', { default: null })

// Enquanto o mapa estiver aberto, o backend lê os APs no intervalo curto.
useLiveInterest('wifi')

const store = useWifiClientsStore()
const settings = useWifiSettingsStore()
const palette = useTopologyPalette()
const container = ref<HTMLElement | null>(null)

const stations = computed(() => dedupeByMac(props.rows))
const aps = computed<MapAp[]>(() =>
  props.devices.map((device) => ({
    deviceId: device.deviceId,
    deviceName: device.deviceName,
    status: device.status,
    error: device.error,
  }))
)
const layout = computed(() => layoutWifiMap(aps.value, stations.value))
const elements = computed(() =>
  buildWifiMapElements({
    layout: layout.value,
    palette: palette.value,
    watched: store.watchedByMac,
    selectedMac: selected.value,
  })
)

/** Conectado em algum AP, filtrado ou não — quem some de verdade esmaece. */
const present = computed(() => new Set(store.rows.map((row) => row.mac)))

const map = useWifiMap(container, elements, {
  onSelect: (mac) => {
    selected.value = mac
  },
  isPresent: (mac) => present.value.has(mac),
})
const hover = map.hovered

const byMac = computed(() => new Map(stations.value.map((row) => [row.mac, row] as const)))
const selectedRow = computed(
  () => (selected.value ? byMac.value.get(selected.value) : null) ?? null
)
const hoveredRow = computed(() =>
  hover.value?.mac ? (byMac.value.get(hover.value.mac) ?? null) : null
)
const hoveredAp = computed(() =>
  hover.value?.deviceId != null
    ? (props.devices.find((device) => device.deviceId === hover.value?.deviceId) ?? null)
    : null
)

/** O estado da leitura de cada AP, em palavras (a cor nunca vai sozinha). */
const AP_STATUS: Record<WifiReadStatus, { label: string; color: string }> = {
  ok: { label: 'Leitura em dia', color: 'success' },
  reading: { label: 'Lendo agora…', color: 'info' },
  failed: { label: 'A leitura falhou', color: 'error' },
}
const tooltipName = computed(() =>
  hoveredRow.value
    ? (store.watchedByMac.get(hoveredRow.value.mac)?.name ?? stationName(hoveredRow.value))
    : ''
)

const liveInterval = computed(() => settings.settings?.liveIntervalS ?? 15)
const recentRoams = computed(() => store.roams.slice(0, 3))

function roamName(mac: string): string {
  const row = store.rows.find((item) => item.mac === mac)
  const watchedName = store.watchedByMac.get(mac)?.name ?? null
  return row ? shortStationName(row, watchedName) : (watchedName ?? `…${mac.slice(-5)}`)
}

const ariaLabel = computed(() => {
  const failed = props.devices.filter((device) => device.status === 'failed').length
  const parts = [
    `Mapa Wi-Fi: ${stations.value.length} aparelhos em ${props.devices.length} APs`,
    failed ? `${failed} AP com leitura falhando` : '',
    'A aba Lista mostra os mesmos dados em tabela.',
  ]
  return parts.filter(Boolean).join('. ')
})

const emptyText = computed(() => {
  if (props.devices.length === 0)
    return 'Nenhum AP lido ainda. A primeira leitura chega em segundos.'
  if (stations.value.length === 0) return 'Nenhum aparelho com esses filtros.'
  return ''
})

// O intervalo mostrado no chip vem das configurações (ação pontual ao abrir).
onMounted(() => {
  if (!settings.settings) void settings.fetch()
})
</script>

<style scoped>
.map-layout {
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  gap: 16px;
}

@media (min-width: 1100px) {
  .map-layout.with-panel {
    grid-template-columns: minmax(0, 1fr) 380px;
  }
}

.canvas-wrap {
  position: relative;
  height: clamp(520px, 68vh, 920px);
  border: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
  border-radius: 8px;
  overflow: hidden;
  background: rgb(var(--v-theme-background));
}

.canvas {
  position: absolute;
  inset: 0;
}

.controls {
  position: absolute;
  top: 12px;
  right: 12px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.live-chip {
  position: absolute;
  top: 12px;
  left: 12px;
}

.roam-feed {
  position: absolute;
  left: 12px;
  bottom: 12px;
  max-width: min(420px, calc(100% - 24px));
  padding: 8px 10px;
  border-radius: 8px;
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
  font-size: 0.8125rem;
  display: grid;
  gap: 2px;
}

.roam-feed-title {
  font-size: 0.7rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}

.roam-line {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}

.roam-time {
  font-variant-numeric: tabular-nums;
}

.map-tooltip {
  position: absolute;
  transform: translate(-50%, calc(-100% - 26px));
  pointer-events: none;
  padding: 8px 10px;
  border-radius: 8px;
  background: rgb(var(--v-theme-surface));
  color: rgb(var(--v-theme-on-surface));
  border: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
  box-shadow: 0 4px 14px rgba(0, 0, 0, 0.25);
  font-size: 0.8125rem;
  white-space: nowrap;
  z-index: 2;
}

.empty-overlay {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  pointer-events: none;
  text-align: center;
  padding: 24px;
}
</style>
