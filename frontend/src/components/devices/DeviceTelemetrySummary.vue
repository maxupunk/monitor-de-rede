<template>
  <div v-if="telemetryCards.length > 0" class="mb-6">
    <div class="text-subtitle-1 font-weight-bold mb-3 d-flex align-center justify-space-between">
      <div class="d-flex align-center ga-2">
        <v-icon color="amber-darken-2">mdi-solar-power</v-icon>
        <span>Telemetria de Sensores & Energia</span>
      </div>
      <v-chip size="x-small" color="amber-darken-3" variant="tonal">
        {{ telemetryCards.length }}
        {{ telemetryCards.length === 1 ? 'métrica ativa' : 'métricas ativas' }}
      </v-chip>
    </div>

    <v-row>
      <v-col v-for="card in telemetryCards" :key="card.key" cols="12" sm="6" md="4" lg="3">
        <v-card
          border
          flat
          class="pa-3 rounded-lg h-100 card-clicavel d-flex flex-column justify-space-between"
          role="button"
          tabindex="0"
          :aria-label="`Ver histórico de ${card.title}`"
          @click="abrir(card)"
          @keydown.enter="abrir(card)"
          @keydown.space.prevent="abrir(card)"
        >
          <div>
            <div class="d-flex align-center justify-space-between mb-2 ga-2">
              <span
                class="text-caption font-weight-bold text-truncate d-flex align-center ga-1 text-grey-darken-1"
              >
                <v-icon size="16" :color="card.color">{{ card.icon }}</v-icon>
                {{ card.title }}
              </span>
              <v-chip size="x-small" :color="card.badgeColor" variant="tonal">
                {{ card.categoryLabel }}
              </v-chip>
            </div>

            <div class="d-flex align-baseline ga-1 mb-2">
              <span class="text-h6 font-weight-bold text-no-wrap" :class="card.valueClass">
                {{ card.formattedValue }}
              </span>
            </div>
          </div>

          <div>
            <MonitorSparkline
              v-if="card.history.length > 1"
              :data="card.history"
              :color="card.hexColor"
              :width="180"
              :height="28"
              class="mb-2"
            />

            <div class="d-flex align-center justify-space-between text-caption text-grey ga-1">
              <span class="text-truncate text-xxs">{{ card.collectedAt }}</span>
              <v-icon size="14" color="grey">mdi-chart-line</v-icon>
            </div>
          </div>
        </v-card>
      </v-col>
    </v-row>

    <MetricHistoryDialog
      v-model="dialogAberto"
      :metric-names="cardAberto ? [cardAberto.key] : []"
      :title="cardAberto?.title ?? ''"
      :icon="cardAberto?.icon"
      :color="cardAberto?.hexColor"
      :data-type="cardAberto?.dataType"
      :unit-type="cardAberto?.unitType ?? 'generic'"
      :custom-unit="cardAberto?.unit"
      :metrics="metrics"
      :format="formatActiveCard"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import MetricHistoryDialog from '@/components/devices/MetricHistoryDialog.vue'
import MonitorSparkline from '@/components/MonitorSparkline.vue'
import {
  useDeviceDetailStore,
  type DeviceMetric,
  type DiscoveredSensorItem,
  type SensorStateSpec,
} from '@/stores/deviceDetail'

const props = withDefaults(
  defineProps<{
    metrics: DeviceMetric[]
    sensors?: DiscoveredSensorItem[]
  }>(),
  {
    sensors: undefined,
  }
)

const SPARKLINE_LIMIT = 25

interface TelemetryDef {
  key: string
  title: string
  icon: string
  color: string
  hexColor: string
  category: 'solar' | 'battery' | 'load' | 'environment' | 'general'
  categoryLabel: string
  dataType: 'float' | 'integer' | 'boolean' | 'state'
  unitType?: 'generic' | 'percentage' | 'boolean' | 'state'
}

const KNOWN_SENSORS: Record<string, TelemetryDef> = {
  pv_voltage: {
    key: 'pv_voltage',
    title: 'Tensão Solar (PV)',
    icon: 'mdi-solar-power-variant',
    color: 'amber-darken-2',
    hexColor: '#f59e0b',
    category: 'solar',
    categoryLabel: 'Solar',
    dataType: 'float',
    unitType: 'generic',
  },
  pv_current: {
    key: 'pv_current',
    title: 'Corrente Solar (PV)',
    icon: 'mdi-current-dc',
    color: 'amber-darken-1',
    hexColor: '#d97706',
    category: 'solar',
    categoryLabel: 'Solar',
    dataType: 'float',
    unitType: 'generic',
  },
  pv_power: {
    key: 'pv_power',
    title: 'Potência Solar (PV)',
    icon: 'mdi-flash',
    color: 'orange-darken-1',
    hexColor: '#ea580c',
    category: 'solar',
    categoryLabel: 'Solar',
    dataType: 'float',
    unitType: 'generic',
  },
  energy_generated: {
    key: 'energy_generated',
    title: 'Energia Acumulada',
    icon: 'mdi-lightning-bolt-circle',
    color: 'amber-darken-3',
    hexColor: '#b45309',
    category: 'solar',
    categoryLabel: 'Solar',
    dataType: 'float',
    unitType: 'generic',
  },
  battery_voltage: {
    key: 'battery_voltage',
    title: 'Tensão da Bateria',
    icon: 'mdi-battery-charging',
    color: 'blue-darken-1',
    hexColor: '#2563eb',
    category: 'battery',
    categoryLabel: 'Bateria',
    dataType: 'float',
    unitType: 'generic',
  },
  battery_current: {
    key: 'battery_current',
    title: 'Corrente da Bateria',
    icon: 'mdi-battery-arrow-up',
    color: 'blue-grey',
    hexColor: '#64748b',
    category: 'battery',
    categoryLabel: 'Bateria',
    dataType: 'float',
    unitType: 'generic',
  },
  battery_status: {
    key: 'battery_status',
    title: 'Status da Bateria',
    icon: 'mdi-battery-check',
    color: 'teal',
    hexColor: '#0d9488',
    category: 'battery',
    categoryLabel: 'Bateria',
    dataType: 'state',
    unitType: 'state',
  },
  load_voltage: {
    key: 'load_voltage',
    title: 'Tensão da Carga',
    icon: 'mdi-power-plug',
    color: 'cyan-darken-2',
    hexColor: '#0891b2',
    category: 'load',
    categoryLabel: 'Carga',
    dataType: 'float',
    unitType: 'generic',
  },
  load_current: {
    key: 'load_current',
    title: 'Corrente da Carga',
    icon: 'mdi-gauge',
    color: 'cyan-darken-3',
    hexColor: '#0e7490',
    category: 'load',
    categoryLabel: 'Carga',
    dataType: 'float',
    unitType: 'generic',
  },
  load_status: {
    key: 'load_status',
    title: 'Status da Saída',
    icon: 'mdi-power',
    color: 'indigo',
    hexColor: '#4f46e5',
    category: 'load',
    categoryLabel: 'Carga',
    dataType: 'boolean',
    unitType: 'boolean',
  },
  relay_status: {
    key: 'relay_status',
    title: 'Status do Relé',
    icon: 'mdi-toggle-switch',
    color: 'deep-purple',
    hexColor: '#7c3aed',
    category: 'load',
    categoryLabel: 'Relé',
    dataType: 'boolean',
    unitType: 'boolean',
  },
  internal_temp: {
    key: 'internal_temp',
    title: 'Temperatura Interna',
    icon: 'mdi-thermometer',
    color: 'deep-orange',
    hexColor: '#e11d48',
    category: 'environment',
    categoryLabel: 'Ambiente',
    dataType: 'float',
    unitType: 'generic',
  },
  external_temp: {
    key: 'external_temp',
    title: 'Temperatura Externa',
    icon: 'mdi-thermometer-lines',
    color: 'light-blue-darken-1',
    hexColor: '#0284c7',
    category: 'environment',
    categoryLabel: 'Ambiente',
    dataType: 'float',
    unitType: 'generic',
  },
  charge_status: {
    key: 'charge_status',
    title: 'Modo de Operação',
    icon: 'mdi-state-machine',
    color: 'purple',
    hexColor: '#9333ea',
    category: 'environment',
    categoryLabel: 'Operação',
    dataType: 'state',
    unitType: 'state',
  },
}

const SYSTEM_HEALTH_KEYS = new Set([
  'cpu_usage',
  'memory_usage',
  'memory_used_bytes',
  'memory_total_bytes',
  'storage_usage',
  'load_average_1m',
  'process_memory_bytes',
  'uptime_seconds',
  'snmp_uptime',
  'ifHCInOctets',
  'ifHCOutOctets',
  'inBps',
  'outBps',
])

const detailStore = useDeviceDetailStore()

const availableSensors = computed<DiscoveredSensorItem[]>(() => {
  if (props.sensors && props.sensors.length > 0) return props.sensors
  if (detailStore.scanResult?.sensors && detailStore.scanResult.sensors.length > 0) {
    return detailStore.scanResult.sensors
  }
  // Reconstitui a partir dos monitores de sensores persistidos no dispositivo:
  const fromMonitors: DiscoveredSensorItem[] = []
  for (const mon of detailStore.monitors) {
    const conf = mon.configuration as Record<string, unknown> | undefined
    if (conf && conf.metric === 'sensor') {
      const key = (conf.sensorKey as string) || ''
      if (key) {
        fromMonitors.push({
          key,
          label: (conf.label as string) || mon.name.replace(/^Sensor\s+/, ''),
          name: mon.name,
          oid: (conf.oid as string) || '',
          unit: (conf.unit as string) || '',
          scale: Number(conf.scale) || 1.0,
          category: (conf.category as string) || 'sensor',
          dataType: (conf.dataType as DiscoveredSensorItem['dataType']) || 'float',
          formattedValue: '',
          isMonitored: true,
          icon: conf.icon as string | undefined,
          color: conf.color as string | undefined,
          states: conf.states as Record<string, string | SensorStateSpec> | undefined,
        })
      }
    }
  }
  return fromMonitors
})

const sensorsByKey = computed<Map<string, DiscoveredSensorItem>>(() => {
  const map = new Map<string, DiscoveredSensorItem>()
  for (const s of availableSensors.value) {
    map.set(s.key, s)
  }
  return map
})

const COLOR_HEX_MAP: Record<string, string> = {
  success: '#10b981',
  warning: '#f59e0b',
  error: '#ef4444',
  info: '#06b6d4',
  amber: '#f59e0b',
  'amber-darken-1': '#d97706',
  'amber-darken-2': '#b45309',
  'amber-darken-3': '#92400e',
  orange: '#ea580c',
  'orange-darken-1': '#c2410c',
  'yellow-darken-2': '#ca8a04',
  blue: '#2563eb',
  'blue-darken-1': '#1d4ed8',
  'blue-grey': '#64748b',
  teal: '#0d9488',
  cyan: '#0891b2',
  'cyan-darken-2': '#0e7490',
  indigo: '#4f46e5',
  'deep-purple': '#7c3aed',
  'deep-orange': '#e11d48',
  'light-blue-darken-1': '#0284c7',
  purple: '#9333ea',
  grey: '#6b7280',
}

function formatTelemetryValue(
  value: number,
  unit?: string | null,
  sensorItem?: DiscoveredSensorItem
): string {
  // 1. Se o sensor possui estados declarados no perfil SNMP:
  if (sensorItem?.states) {
    const k = String(Math.round(value))
    const s = sensorItem.states[k]
    if (s) {
      return typeof s === 'string' ? s : s.label
    }
  }

  // 2. Se for tipo booleano:
  if (sensorItem?.dataType === 'boolean') {
    return Math.round(value) === 1 ? 'Ligado' : 'Desligado'
  }

  // 3. Formatação física:
  const u = unit || sensorItem?.unit || ''
  if (u === 'kWh') {
    return `${value.toFixed(2)} kWh`
  }
  if (u.length > 0) {
    return value % 1 === 0 ? `${value.toFixed(0)} ${u}` : `${value.toFixed(1)} ${u}`
  }
  return value % 1 === 0 ? `${value.toFixed(0)}` : `${value.toFixed(1)}`
}

interface TelemetryCardItem {
  key: string
  title: string
  icon: string
  color: string
  hexColor: string
  badgeColor: string
  categoryLabel: string
  formattedValue: string
  unit?: string
  dataType: 'float' | 'integer' | 'boolean' | 'state'
  unitType?: 'generic' | 'percentage' | 'boolean' | 'state'
  collectedAt: string
  valueClass?: string
  history: Array<{ value: number; recordedAt: string }>
}

const telemetryCards = computed<TelemetryCardItem[]>(() => {
  // Encontra todas as métricas mais recentes que são telemetria de sensores
  const byName = new Map<string, DeviceMetric[]>()
  for (const m of props.metrics) {
    if (SYSTEM_HEALTH_KEYS.has(m.metricName)) continue
    const list = byName.get(m.metricName) ?? []
    list.push(m)
    byName.set(m.metricName, list)
  }

  const cards: TelemetryCardItem[] = []
  for (const [name, samples] of byName.entries()) {
    if (samples.length === 0) continue
    const latest = samples[0]
    const val = Number(latest.metricValue)
    if (!Number.isFinite(val)) continue

    const sensorDef = sensorsByKey.value.get(name)
    const fallback = KNOWN_SENSORS[name]

    const title = sensorDef?.label || fallback?.title || name.replace(/_/g, ' ').toUpperCase()
    let icon = sensorDef?.icon || fallback?.icon || 'mdi-gauge'
    let color = sensorDef?.color || fallback?.color || 'primary'
    let hexColor =
      (sensorDef?.color && COLOR_HEX_MAP[sensorDef.color]) || fallback?.hexColor || '#1976d2'
    const categoryLabel = fallback?.categoryLabel || sensorDef?.category || 'Sensor'
    const unit = latest.unit || sensorDef?.unit || undefined
    const dataType = (sensorDef?.dataType ||
      fallback?.dataType ||
      'float') as TelemetryDef['dataType']
    const unitType =
      dataType === 'boolean' || dataType === 'state' ? dataType : fallback?.unitType || 'generic'

    // Se possui mapeamento de estados declarativo, ajusta ícone e cor dinamicamente para o valor atual:
    if (sensorDef?.states) {
      const stateKey = String(Math.round(val))
      const matchedState = sensorDef.states[stateKey]
      if (matchedState && typeof matchedState === 'object') {
        if (matchedState.icon) icon = matchedState.icon
        if (matchedState.color) {
          color = matchedState.color
          hexColor = COLOR_HEX_MAP[matchedState.color] || hexColor
        }
      }
    }

    const formatted = formatTelemetryValue(val, unit, sensorDef)
    const history = samples
      .slice(0, SPARKLINE_LIMIT)
      .reverse()
      .map((s) => ({ value: Number(s.metricValue) || 0, recordedAt: s.createdAt }))

    cards.push({
      key: name,
      title,
      icon,
      color,
      hexColor,
      badgeColor: color,
      categoryLabel,
      formattedValue: formatted,
      unit,
      dataType,
      unitType,
      collectedAt: latest.createdAt || 'Recente',
      history,
    })
  }

  // Ordena de acordo com a prioridade das categorias
  const order = [
    'pv_',
    'energy_',
    'battery_',
    'load_',
    'relay_',
    'internal_',
    'external_',
    'charge_',
  ]
  cards.sort((a, b) => {
    const idxA = order.findIndex((prefix) => a.key.startsWith(prefix))
    const idxB = order.findIndex((prefix) => b.key.startsWith(prefix))
    const rankA = idxA === -1 ? 99 : idxA
    const rankB = idxB === -1 ? 99 : idxB
    return rankA - rankB
  })

  return cards
})

const dialogAberto = ref(false)
const cardAberto = ref<TelemetryCardItem | null>(null)

const formatActiveCard = computed(() => {
  const card = cardAberto.value
  if (!card) return undefined
  const sensorDef = sensorsByKey.value.get(card.key)
  return (v: number) => formatTelemetryValue(v, card.unit, sensorDef)
})

function abrir(card: TelemetryCardItem): void {
  cardAberto.value = card
  dialogAberto.value = true
}
</script>

<style scoped>
.card-clicavel {
  cursor: pointer;
  transition:
    border-color 0.15s ease,
    transform 0.15s ease;
}

.card-clicavel:hover,
.card-clicavel:focus-visible {
  border-color: rgb(var(--v-theme-primary));
  transform: translateY(-1px);
}

.text-xxs {
  font-size: 0.7rem;
}
</style>
