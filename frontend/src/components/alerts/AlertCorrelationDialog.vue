<template>
  <v-dialog v-model="dialog" max-width="780">
    <v-card rounded="lg">
      <v-card-title class="pa-4 d-flex align-center">
        <v-icon color="primary" class="mr-2">mdi-chart-tree</v-icon>
        <span>Correlação e Causa Raiz Automática (RCA)</span>
        <v-spacer></v-spacer>
        <v-btn icon variant="text" size="small" @click="dialog = false">
          <v-icon>mdi-close</v-icon>
        </v-btn>
      </v-card-title>

      <v-divider></v-divider>

      <v-card-text class="pa-4">
        <div v-if="loading" class="d-flex justify-center py-6">
          <v-progress-circular indeterminate color="primary"></v-progress-circular>
        </div>

        <template v-else-if="correlation">
          <!-- Diagnóstico em Linguagem Natural -->
          <v-card
            variant="tonal"
            :color="categoryColor(correlation.causalCategory)"
            class="mb-4 pa-4 rounded-lg"
          >
            <div class="d-flex align-center justify-space-between flex-wrap ga-2 mb-2">
              <div class="d-flex align-center ga-2">
                <v-chip
                  size="small"
                  :color="categoryColor(correlation.causalCategory)"
                  variant="flat"
                  class="font-weight-bold"
                >
                  <v-icon start size="small">{{ categoryIcon(correlation.causalCategory) }}</v-icon>
                  {{ correlation.causalCategoryLabel }}
                </v-chip>
                <v-chip
                  size="small"
                  :color="confidenceColor(correlation.confidence)"
                  variant="outlined"
                  class="font-weight-bold"
                >
                  <v-icon start size="small">mdi-shield-check</v-icon>
                  {{ correlation.confidence }}% de Confiança
                </v-chip>
              </div>

              <div class="text-caption text-medium-emphasis">
                Janela de {{ correlation.windowSeconds }}s
              </div>
            </div>

            <div class="text-body-1 font-weight-medium my-2 text-high-emphasis">
              <v-icon color="primary" class="mr-1" size="small">mdi-format-quote-open</v-icon>
              {{ correlation.explanation }}
              <v-icon color="primary" class="ml-1" size="small">mdi-format-quote-close</v-icon>
            </div>

            <div
              v-if="correlation.impactedDevicesCount > 0"
              class="text-caption text-medium-emphasis mt-2"
            >
              <v-icon size="small" class="mr-1">mdi-alert-octagon</v-icon>
              Raio de impacto: {{ correlation.impactedDevicesCount }} dispositivo{{
                correlation.impactedDevicesCount === 1 ? '' : 's'
              }}
              afetado{{ correlation.impactedDevicesCount === 1 ? '' : 's' }} em cascata.
            </div>
          </v-card>

          <!-- Cadeia de Dependência Topológica -->
          <div
            v-if="correlation.dependencyChain && correlation.dependencyChain.length > 1"
            class="mb-4"
          >
            <div class="text-subtitle-2 font-weight-bold mb-2 d-flex align-center">
              <v-icon size="small" color="primary" class="mr-1">mdi-arrow-decision-auto</v-icon>
              Cadeia de Dependência até o Alvo
            </div>

            <v-sheet rounded="lg" border class="pa-3 bg-surface">
              <div class="d-flex align-center flex-wrap ga-2">
                <template v-for="(node, idx) in correlation.dependencyChain" :key="node.id">
                  <v-chip
                    :color="node.isRootCause ? 'error' : node.isTarget ? 'warning' : 'primary'"
                    :variant="node.isRootCause ? 'flat' : 'tonal'"
                    size="small"
                    class="font-weight-medium"
                  >
                    <v-icon start size="x-small">
                      {{ node.isRootCause ? 'mdi-alert-decagram' : 'mdi-server' }}
                    </v-icon>
                    {{ node.name }}
                    <span v-if="node.ipAddress" class="text-caption opacity-75 ml-1">
                      ({{ node.ipAddress }})
                    </span>
                  </v-chip>

                  <v-icon
                    v-if="idx < correlation.dependencyChain.length - 1"
                    size="small"
                    color="grey"
                  >
                    mdi-chevron-right
                  </v-icon>
                </template>
              </div>
            </v-sheet>
          </div>

          <!-- Causa Raiz Primária Detalhada -->
          <v-alert
            v-if="correlation.primaryCause"
            type="error"
            variant="tonal"
            class="mb-4"
            border="start"
          >
            <div class="font-weight-bold mb-1 d-flex align-center justify-space-between">
              <span>Incidente Causa Raiz</span>
              <v-chip size="x-small" :color="severityColor(correlation.primaryCause.severity)">
                {{ severityLabel(correlation.primaryCause.severity) }}
              </v-chip>
            </div>
            <div class="d-flex align-center flex-wrap ga-2">
              <span class="font-weight-medium">{{ correlation.primaryCause.title }}</span>
              <span class="text-caption text-grey">{{ correlation.primaryCause.message }}</span>
            </div>
            <div class="text-caption text-grey mt-1">
              Iniciado em {{ formatDateTime(correlation.primaryCause.startedAt) }}
            </div>
          </v-alert>

          <!-- Dispositivos Impactados -->
          <div
            v-if="correlation.impactedDevices && correlation.impactedDevices.length > 0"
            class="mb-4"
          >
            <div class="text-subtitle-2 font-weight-bold mb-2 d-flex align-center">
              <v-icon size="small" color="warning" class="mr-1">mdi-devices</v-icon>
              Dispositivos Afetados ({{ correlation.impactedDevices.length }})
            </div>

            <div class="d-flex flex-wrap ga-1">
              <v-chip
                v-for="imp in correlation.impactedDevices"
                :key="imp.id"
                size="x-small"
                variant="outlined"
                color="grey-darken-2"
              >
                {{ imp.name }}
                <span v-if="imp.ipAddress" class="text-caption opacity-75 ml-1">
                  {{ imp.ipAddress }}
                </span>
              </v-chip>
            </div>
          </div>

          <!-- Demais Eventos Correlacionados -->
          <div v-if="correlation.relatedEvents.length > 0">
            <div class="text-subtitle-2 font-weight-bold mb-2">
              {{ correlation.relatedEvents.length }} alerta{{
                correlation.relatedEvents.length === 1 ? '' : 's'
              }}
              correlacionado{{ correlation.relatedEvents.length === 1 ? '' : 's' }}
            </div>

            <v-list density="compact" class="bg-surface rounded-lg border">
              <v-list-item
                v-for="event in correlation.relatedEvents"
                :key="event.id"
                :title="event.title"
                :subtitle="event.message ?? undefined"
              >
                <template #prepend>
                  <v-icon size="small" color="grey" class="mr-2">mdi-alert-circle-outline</v-icon>
                </template>
                <template #append>
                  <v-chip size="x-small" :color="severityColor(event.severity)">
                    {{ severityLabel(event.severity) }}
                  </v-chip>
                </template>
              </v-list-item>
            </v-list>
          </div>

          <div v-else-if="!correlation.primaryCause" class="text-center py-4 text-medium-emphasis">
            <v-icon size="36" color="grey" class="mb-1">mdi-check-circle-outline</v-icon>
            <div class="text-caption">
              Nenhuma correlação em cascata identificada. O alerta parece ser um evento isolado.
            </div>
          </div>
        </template>

        <v-alert v-else-if="error" type="error" variant="tonal" class="mt-2">
          {{ error }}
        </v-alert>
      </v-card-text>

      <v-card-actions class="pa-4">
        <v-spacer></v-spacer>
        <v-btn variant="text" @click="dialog = false">Fechar</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useAlertsStore, type AlertCorrelation } from '@/stores/alerts'
import { severityLabel, severityColor } from '@/utils/alertPresentation'
import { formatDateTime } from '@/utils/formatters'

const props = defineProps<{
  modelValue: boolean
  alertId: number | null
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
}>()

const alertsStore = useAlertsStore()

const dialog = computed({
  get: () => props.modelValue,
  set: (value) => emit('update:modelValue', value),
})

const loading = ref(false)
const correlation = ref<AlertCorrelation | null>(null)
const error = ref<string | null>(null)

function categoryColor(cat?: string): string {
  switch (cat) {
    case 'gateway':
      return 'deep-purple'
    case 'router':
      return 'indigo'
    case 'switch':
      return 'teal'
    case 'firewall':
      return 'red-darken-1'
    case 'vpn':
      return 'blue'
    case 'isp_link':
      return 'cyan-darken-2'
    case 'site_outage':
      return 'amber-darken-3'
    default:
      return 'grey-darken-1'
  }
}

function categoryIcon(cat?: string): string {
  switch (cat) {
    case 'gateway':
      return 'mdi-router-network'
    case 'router':
      return 'mdi-router-wireless'
    case 'switch':
      return 'mdi-lan'
    case 'firewall':
      return 'mdi-shield-lock'
    case 'vpn':
      return 'mdi-vpn'
    case 'isp_link':
      return 'mdi-web'
    case 'site_outage':
      return 'mdi-transmission-tower-off'
    default:
      return 'mdi-server'
  }
}

function confidenceColor(conf: number): string {
  if (conf >= 80) return 'success'
  if (conf >= 50) return 'warning'
  return 'grey'
}

async function load() {
  if (!props.alertId) return
  loading.value = true
  error.value = null
  correlation.value = null
  try {
    const result = await alertsStore.fetchCorrelation(props.alertId)
    if (result) {
      correlation.value = result
    } else {
      error.value = alertsStore.error
    }
  } finally {
    loading.value = false
  }
}

watch(
  () => props.modelValue,
  (open) => {
    if (open) void load()
  },
  { immediate: true }
)
</script>
