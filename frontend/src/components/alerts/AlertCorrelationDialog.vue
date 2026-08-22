<template>
  <v-dialog v-model="dialog" max-width="720">
    <v-card rounded="lg">
      <v-card-title class="pa-4 d-flex align-center">
        <v-icon color="primary" class="mr-2">mdi-source-branch</v-icon>
        Correlação temporal
        <v-spacer />
        <v-btn icon variant="text" size="small" @click="dialog = false">
          <v-icon>mdi-close</v-icon>
        </v-btn>
      </v-card-title>

      <v-divider />

      <v-card-text class="pa-4">
        <div v-if="loading" class="d-flex justify-center py-6">
          <v-progress-circular indeterminate color="primary" />
        </div>

        <template v-else-if="correlation">
          <div v-if="correlation.correlationCount === 0" class="text-center py-6 text-grey">
            <v-icon size="48" color="grey-lighten-1" class="mb-2">
              mdi-chart-timeline-variant
            </v-icon>
            <div>
              Nenhum alerta correlacionado encontrado na janela de
              {{ correlation.windowSeconds }} segundos.
            </div>
          </div>

          <template v-else>
            <v-alert
              v-if="correlation.primaryCause"
              type="info"
              variant="tonal"
              class="mb-4"
              border="start"
            >
              <div class="font-weight-bold mb-1">Possível causa raiz</div>
              <div class="d-flex align-center flex-wrap ga-2">
                <v-chip size="small" :color="severityColor(correlation.primaryCause.severity)">
                  {{ severityLabel(correlation.primaryCause.severity) }}
                </v-chip>
                <span class="font-weight-medium">{{ correlation.primaryCause.title }}</span>
                <span class="text-caption text-grey">{{ correlation.primaryCause.message }}</span>
              </div>
              <div class="text-caption text-grey mt-1">
                Iniciado em {{ formatDateTime(correlation.primaryCause.startedAt) }}
              </div>
            </v-alert>

            <div class="text-subtitle-2 font-weight-bold mb-2">
              {{ correlation.correlationCount }} alerta{{
                correlation.correlationCount === 1 ? '' : 's'
              }}
              na mesma janela
            </div>
            <div class="text-caption text-grey mb-3">
              Janela de {{ correlation.windowSeconds }} segundos em torno do alerta.
            </div>

            <v-list density="compact" class="bg-grey-lighten-4 rounded-lg">
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
          </template>
        </template>

        <v-alert v-else-if="error" type="error" variant="tonal" class="mt-2">
          {{ error }}
        </v-alert>
      </v-card-text>

      <v-card-actions class="pa-4">
        <v-spacer />
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
