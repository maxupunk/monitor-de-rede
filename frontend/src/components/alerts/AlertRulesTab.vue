<template>
  <v-alert
    v-if="deviceFilter"
    type="info"
    variant="tonal"
    density="comfortable"
    class="mb-4 rounded-lg"
  >
    <div class="d-flex flex-wrap align-center justify-space-between ga-2">
      <span>
        Mostrando apenas as regras de
        <strong>{{ deviceName(deviceFilter) }}</strong
        >.
      </span>
      <div class="d-flex ga-2">
        <v-btn size="small" variant="text" :to="`/devices/${deviceFilter}?tab=rules`">
          Abrir o dispositivo
        </v-btn>
        <v-btn size="small" variant="tonal" @click="emit('clear-device-filter')"> Ver todas </v-btn>
      </div>
    </div>
  </v-alert>

  <v-alert
    v-if="!alertsStore.loading && alertsStore.alertRules.length === 0"
    type="info"
    variant="tonal"
    density="comfortable"
    class="mb-4"
  >
    Nenhuma regra configurada. Comece pelas
    <a class="font-weight-bold text-primary" href="#" @click.prevent="emit('open-catalog')"
      >regras pré-configuradas</a
    >
    para cobrir indisponibilidade, latência, perda de pacotes e quedas de interface.
  </v-alert>

  <ResponsiveDataTable
    :headers="rulesHeaders"
    :items="visibleRules"
    :loading="alertsStore.loading"
    :items-per-page="-1"
    hide-default-footer
    no-data-text="Nenhuma regra configurada"
    :clickable="false"
  >
    <template #item.name="{ item }">
      <div class="d-flex align-center ga-2">
        <span>{{ item.name }}</span>
        <v-tooltip v-if="item.templateKey" text="Criada a partir do catálogo de regras">
          <template #activator="{ props: tooltipProps }">
            <v-icon v-bind="tooltipProps" size="16" color="primary"> mdi-playlist-check </v-icon>
          </template>
        </v-tooltip>
      </div>
    </template>

    <template #item.scope="{ item }">
      <div v-if="item.deviceId || item.monitorId" class="d-flex flex-column">
        <RouterLink
          v-if="item.deviceId"
          class="text-primary text-decoration-none font-weight-medium"
          :to="`/devices/${item.deviceId}?tab=rules`"
        >
          <v-icon size="14" class="mr-1">mdi-router-network</v-icon>
          {{ deviceName(item.deviceId) }}
        </RouterLink>
        <span v-if="item.monitorId" class="text-caption text-grey">
          Monitor #{{ item.monitorId }}
        </span>
      </div>
      <span v-else class="text-caption text-grey">Todos os dispositivos</span>
    </template>

    <template #item.metric="{ item }">
      {{ metricLabel(item.condition?.field) }}
    </template>

    <template #item.criteria="{ item }">
      <span class="text-body-2">
        {{ operatorLabel(item.condition?.operator).toLowerCase() }}
        <strong>
          {{ formatConditionValue(item.condition?.field, item.condition?.value) }}
        </strong>
      </span>
    </template>

    <template #item.durationSeconds="{ item }">
      {{ durationLabel(item.durationSeconds) }}
    </template>

    <template #item.severity="{ item }">
      <v-chip :color="severityColor(item.severity)" size="small">
        {{ severityLabel(item.severity) }}
      </v-chip>
    </template>

    <template #item.enabled="{ item }">
      <v-switch
        :model-value="item.isEnabled ?? item.enabled"
        color="success"
        density="compact"
        hide-details
        @update:model-value="emit('toggle-rule', item, $event)"
      ></v-switch>
    </template>

    <template #item.actions="{ item }">
      <div class="d-flex ga-1">
        <v-btn icon size="small" variant="text" @click="emit('edit-rule', item)">
          <v-icon>mdi-pencil</v-icon>
        </v-btn>
        <v-btn icon size="small" variant="text" color="error" @click="emit('delete-rule', item)">
          <v-icon>mdi-delete</v-icon>
        </v-btn>
      </div>
    </template>

    <template #mobile-item="{ item }">
      <div class="d-flex flex-column ga-2">
        <div class="d-flex align-start justify-space-between ga-2">
          <div class="flex-grow-1 text-break">
            <div class="d-flex flex-wrap align-center ga-2">
              <span class="text-subtitle-2 font-weight-bold">{{ item.name }}</span>
              <v-chip :color="severityColor(item.severity)" size="x-small">
                {{ severityLabel(item.severity) }}
              </v-chip>
            </div>
            <div class="text-body-2 text-grey-darken-1 mt-1">
              {{ metricLabel(item.condition?.field) }}
              {{ operatorLabel(item.condition?.operator).toLowerCase() }}
              <strong>
                {{ formatConditionValue(item.condition?.field, item.condition?.value) }}
              </strong>
            </div>
            <div class="text-caption text-grey mt-1">
              Tolerância: {{ durationLabel(item.durationSeconds) }}
            </div>
          </div>
          <v-switch
            :model-value="item.isEnabled ?? item.enabled"
            color="success"
            density="compact"
            hide-details
            style="transform: scale(0.85); transform-origin: right top"
            @update:model-value="emit('toggle-rule', item, $event)"
          ></v-switch>
        </div>
        <div class="d-flex ga-1 mt-1">
          <v-btn icon size="small" variant="text" @click="emit('edit-rule', item)">
            <v-icon>mdi-pencil</v-icon>
          </v-btn>
          <v-btn icon size="small" variant="text" color="error" @click="emit('delete-rule', item)">
            <v-icon>mdi-delete</v-icon>
          </v-btn>
        </div>
      </div>
    </template>
  </ResponsiveDataTable>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useAlertsStore, type AlertRule } from '@/stores/alerts'
import { useDevicesStore } from '@/stores/devices'
import ResponsiveDataTable from '@/components/ResponsiveDataTable.vue'
import {
  ALERT_DURATIONS,
  metricLabel,
  operatorLabel,
  severityLabel,
  severityColor,
  formatConditionValue,
} from '@/utils/alertPresentation'

const props = defineProps<{
  deviceFilter: number | null
}>()

const emit = defineEmits<{
  'clear-device-filter': []
  'open-catalog': []
  'edit-rule': [rule: AlertRule]
  'delete-rule': [rule: AlertRule]
  'toggle-rule': [rule: AlertRule, enabled: boolean | null]
}>()

const alertsStore = useAlertsStore()
const devicesStore = useDevicesStore()

const visibleRules = computed(() => {
  if (props.deviceFilter == null) return alertsStore.alertRules
  return alertsStore.alertRules.filter((rule) => rule.deviceId === props.deviceFilter)
})

function deviceName(id: number): string {
  return devicesStore.devices.find((device) => device.id === id)?.name ?? `Dispositivo #${id}`
}

const rulesHeaders = [
  { title: 'Nome da Regra', key: 'name' },
  { title: 'Escopo', key: 'scope', sortable: false, width: '200px' },
  { title: 'Métrica Monitorada', key: 'metric', sortable: false },
  { title: 'Critério de Disparo', key: 'criteria', sortable: false },
  { title: 'Tolerância', key: 'durationSeconds', width: '150px' },
  { title: 'Severidade', key: 'severity', width: '120px' },
  { title: 'Ativa', key: 'enabled', sortable: false, width: '90px' },
  { title: 'Ações', key: 'actions', sortable: false, width: '110px' },
]

function durationLabel(seconds?: number): string {
  return ALERT_DURATIONS.find((d) => d.value === (seconds ?? 0))?.title ?? `${seconds}s`
}
</script>
