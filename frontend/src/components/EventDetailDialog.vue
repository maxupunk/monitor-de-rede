<template>
  <v-dialog
    :model-value="modelValue"
    max-width="700"
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card v-if="event" class="rounded-lg">
      <v-card-title class="d-flex align-center justify-space-between py-3 px-4">
        <div class="d-flex align-center ga-3">
          <v-avatar :color="details.color" size="40" variant="tonal">
            <v-icon>{{ details.icon }}</v-icon>
          </v-avatar>
          <div>
            <div class="text-h6 font-weight-bold">{{ details.title }}</div>
            <div class="text-caption text-grey">{{ formattedFullDate }} ({{ details.time }})</div>
          </div>
        </div>
        <v-btn icon variant="text" size="small" @click="emit('update:modelValue', false)">
          <v-icon>mdi-close</v-icon>
        </v-btn>
      </v-card-title>

      <v-divider></v-divider>

      <v-card-text class="pa-4">
        <!-- Event Type Chip & Quick Summary -->
        <div class="d-flex align-center ga-2 mb-4">
          <v-chip size="small" :color="details.color" variant="flat" class="font-weight-medium">
            {{ event.type }}
          </v-chip>
          <span class="text-body-2 font-weight-medium text-grey-darken-2">
            {{ details.message }}
          </span>
        </div>

        <!-- Metrics Table (when metrics exist) -->
        <div v-if="details.metrics && details.metrics.length > 0" class="mb-4">
          <div class="d-flex align-center justify-space-between mb-2 flex-wrap ga-2">
            <div class="text-subtitle-2 font-weight-bold d-flex align-center ga-1">
              <v-icon size="18" color="primary">mdi-chart-bar</v-icon>
              Métricas Coletadas ({{ details.metrics.length }})
            </div>
            <v-chip v-if="interfaceCount > 0" size="x-small" variant="tonal" color="info">
              {{ interfaceCount }} interface(s) monitorada(s)
            </v-chip>
          </div>

          <!-- Filtro interno quando há muitas métricas -->
          <v-text-field
            v-if="details.metrics.length > 5"
            v-model="metricSearch"
            placeholder="Filtrar métricas (ex.: ifHCInOctets, interface 2, cpu)..."
            prepend-inner-icon="mdi-magnify"
            density="compact"
            variant="outlined"
            hide-details
            clearable
            class="mb-3"
          ></v-text-field>

          <v-table
            density="compact"
            class="border rounded"
            style="max-height: 320px; overflow-y: auto"
          >
            <thead>
              <tr>
                <th class="text-left font-weight-bold">Métrica</th>
                <th class="text-left font-weight-bold">Interface / Contexto</th>
                <th class="text-left font-weight-bold">Valor Coletado</th>
                <th class="text-left font-weight-bold">Unidade</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="(m, idx) in filteredMetrics" :key="idx">
                <td class="font-mono text-body-2 font-weight-medium">{{ m.name }}</td>
                <td>
                  <v-chip
                    v-if="m.interfaceId !== null && m.interfaceId !== undefined"
                    size="x-small"
                    variant="tonal"
                    color="primary"
                  >
                    Interface #{{ m.interfaceId }}
                  </v-chip>
                  <span v-else class="text-caption text-grey">Sistema (Host)</span>
                </td>
                <td class="font-weight-bold">
                  {{ m.value }}
                  <span
                    v-if="humanFormatted(m.value, m.unit) !== String(m.value)"
                    class="text-caption text-grey ml-1 font-weight-regular"
                  >
                    ({{ humanFormatted(m.value, m.unit) }})
                  </span>
                </td>
                <td class="text-grey">{{ m.unit || '-' }}</td>
              </tr>
              <tr v-if="filteredMetrics.length === 0">
                <td colspan="4" class="text-center text-grey py-4">
                  Nenhuma métrica corresponde ao filtro "{{ metricSearch }}"
                </td>
              </tr>
            </tbody>
          </v-table>
        </div>

        <!-- Full Event Payload / Raw JSON -->
        <div>
          <div class="d-flex align-center justify-space-between mb-2">
            <div class="text-subtitle-2 font-weight-bold d-flex align-center ga-1">
              <v-icon size="18" color="info">mdi-code-json</v-icon>
              Payload Completo do Evento (JSON)
            </div>
            <v-btn
              size="x-small"
              variant="tonal"
              color="primary"
              prepend-icon="mdi-content-copy"
              @click="copyJson"
            >
              {{ copied ? 'Copiado!' : 'Copiar JSON' }}
            </v-btn>
          </div>
          <v-card variant="outlined" class="bg-grey-lighten-4 pa-3 overflow-x-auto rounded">
            <pre
              class="text-caption font-mono text-grey-darken-3 mb-0"
              style="white-space: pre-wrap; word-break: break-word"
            >{{ details.rawJson }}</pre>
          </v-card>
        </div>
      </v-card-text>

      <v-divider></v-divider>

      <v-card-actions class="pa-3 justify-end">
        <v-btn variant="tonal" color="grey" @click="emit('update:modelValue', false)">
          Fechar
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import type { RealtimeEventPayload } from '@/stores/events'
import { formatEventDetails, formatHumanReadableValue } from '@/utils/eventPresentation'

const props = defineProps<{
  modelValue: boolean
  event: RealtimeEventPayload | null
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', val: boolean): void
}>()

const copied = ref(false)
const metricSearch = ref('')

const details = computed(() => {
  if (!props.event) {
    return {
      title: '',
      message: '',
      icon: 'mdi-pulse',
      color: 'info',
      time: '',
      rawJson: '{}',
    }
  }
  return formatEventDetails(props.event)
})

const interfaceCount = computed(() => {
  if (!details.value.metrics) return 0
  const ids = details.value.metrics
    .map((m) => m.interfaceId)
    .filter((id): id is number => id != null)
  return new Set(ids).size
})

const filteredMetrics = computed(() => {
  const list = details.value.metrics || []
  if (!metricSearch.value.trim()) return list
  const q = metricSearch.value.toLowerCase().trim()

  return list.filter((m) => {
    const nameMatch = m.name.toLowerCase().includes(q)
    const valMatch = String(m.value).toLowerCase().includes(q)
    const unitMatch = String(m.unit || '')
      .toLowerCase()
      .includes(q)
    const ifMatch =
      m.interfaceId != null &&
      (`interface ${m.interfaceId}`.includes(q) ||
        `if#${m.interfaceId}`.includes(q) ||
        `#${m.interfaceId}`.includes(q) ||
        String(m.interfaceId) === q)

    return nameMatch || valMatch || unitMatch || ifMatch
  })
})

function humanFormatted(val: any, unit?: string): string {
  return formatHumanReadableValue(val, unit)
}

const formattedFullDate = computed(() => {
  if (!props.event?.timestamp) return 'Data não informada'
  const d = new Date(props.event.timestamp)
  return Number.isNaN(d.getTime()) ? props.event.timestamp : d.toLocaleString('pt-BR')
})

async function copyJson() {
  if (!details.value.rawJson) return
  try {
    await navigator.clipboard.writeText(details.value.rawJson)
    copied.value = true
    setTimeout(() => {
      copied.value = false
    }, 2000)
  } catch {
    // clipboard failure fallback
  }
}
</script>
