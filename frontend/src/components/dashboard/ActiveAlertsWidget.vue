<template>
  <v-card elevation="2" class="rounded-lg fill-height">
    <v-card-title class="d-flex align-center justify-space-between py-3 px-4 flex-wrap ga-2">
      <div class="d-flex align-center">
        <v-icon start color="warning">mdi-bell-outline</v-icon>
        <span class="font-weight-bold text-h6">Alertas Críticos Ativos</span>
        <v-chip size="x-small" color="warning" class="ml-2" variant="tonal">
          {{ alertsStore.activeAlerts.length }}
        </v-chip>
      </div>
      <v-btn
        v-if="alertsStore.activeAlerts.length > 0"
        size="small"
        color="primary"
        variant="outlined"
        prepend-icon="mdi-refresh"
        :loading="verifyingAll"
        @click="emit('verify-all')"
      >
        Verificar Todos
      </v-btn>
    </v-card-title>
    <v-divider></v-divider>
    <v-card-text class="pa-0">
      <div v-if="alertsStore.activeAlerts.length > 0">
        <v-list lines="two">
          <v-list-item
            v-for="alert in alertsStore.activeAlerts.slice(0, 5)"
            :key="alert.id"
            :title="alert.title"
            :subtitle="alert.message"
            class="px-4 py-2 border-b cursor-pointer"
            @click="emit('go-to-alert', alert)"
          >
            <template #prepend>
              <v-avatar
                :color="
                  alert.severity === 'critical' || alert.severity === 'error' ? 'error' : 'warning'
                "
                size="36"
              >
                <v-icon color="white">mdi-alert</v-icon>
              </v-avatar>
            </template>
            <template #append>
              <div class="d-flex flex-column flex-md-row align-end align-md-center ga-2">
                <v-chip variant="outlined" size="x-small" class="font-weight-medium">
                  {{ statusLabel(alert.status) }}
                </v-chip>
                <div class="d-flex ga-1">
                  <v-tooltip text="Verificar se resolveu">
                    <template #activator="{ props: tooltipProps }">
                      <v-btn
                        v-bind="tooltipProps"
                        icon
                        size="small"
                        variant="text"
                        color="info"
                        :loading="verifyingId === alert.id"
                        @click.stop="emit('verify', alert.id)"
                      >
                        <v-icon>mdi-refresh</v-icon>
                      </v-btn>
                    </template>
                  </v-tooltip>
                  <v-tooltip text="Reconhecer alerta (testa e remove se resolvido)">
                    <template #activator="{ props: tooltipProps }">
                      <v-btn
                        v-bind="tooltipProps"
                        icon
                        size="small"
                        variant="text"
                        color="primary"
                        :disabled="alert.status === 'acknowledged'"
                        :loading="verifyingId === alert.id"
                        @click.stop="emit('acknowledge', alert.id)"
                      >
                        <v-icon>mdi-check-circle-outline</v-icon>
                      </v-btn>
                    </template>
                  </v-tooltip>
                  <v-tooltip text="Silenciar alerta">
                    <template #activator="{ props: tooltipProps }">
                      <v-btn
                        v-bind="tooltipProps"
                        icon
                        size="small"
                        variant="text"
                        color="warning"
                        :disabled="alert.status === 'silenced'"
                        @click.stop="emit('silence', alert.id)"
                      >
                        <v-icon>mdi-bell-off-outline</v-icon>
                      </v-btn>
                    </template>
                  </v-tooltip>
                </div>
              </div>
            </template>
          </v-list-item>
        </v-list>
      </div>
      <div v-else class="pa-6 text-center text-grey">
        <v-icon size="44" color="success" class="mb-2">mdi-check-all</v-icon>
        <div class="text-subtitle-2 font-weight-medium">Nenhum alerta ativo!</div>
        <div class="text-caption">Todos os sistemas estão funcionando normalmente.</div>
      </div>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import { useAlertsStore, type AlertEvent } from '@/stores/alerts'
import { statusLabel } from '@/utils/alertPresentation'

defineProps<{
  verifyingId: number | null
  verifyingAll: boolean
}>()

const emit = defineEmits<{
  verify: [id: number]
  acknowledge: [id: number]
  silence: [id: number]
  'verify-all': []
  'go-to-alert': [alert: AlertEvent]
}>()

const alertsStore = useAlertsStore()
</script>

<style scoped>
.cursor-pointer {
  cursor: pointer;
}
</style>
