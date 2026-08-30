<template>
  <div>
    <v-infinite-scroll :key="eventsHistory.scrollKey.value" @load="eventsHistory.load">
      <!-- Desktop: Tabela -->
      <div v-if="$vuetify.display.mdAndUp" class="table-responsive">
        <v-table hover density="comfortable" class="rounded-lg border">
          <thead>
            <tr>
              <th style="width: 120px">Severidade</th>
              <th>Mensagem</th>
              <th style="width: 180px">Data/Hora</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="evt in eventsHistory.items.value" :key="evt.id">
              <td>
                <v-chip
                  :color="
                    evt.severity === 'critical' || evt.severity === 'error' ? 'error' : 'warning'
                  "
                  size="x-small"
                  variant="flat"
                  class="font-weight-bold text-uppercase px-2"
                >
                  {{ (evt.severity || 'INFO').toUpperCase() }}
                </v-chip>
              </td>
              <td class="text-body-2">{{ evt.message }}</td>
              <td class="text-caption text-grey">{{ evt.createdAt }}</td>
            </tr>
          </tbody>
        </v-table>
      </div>

      <!-- Mobile: Cards Responsivos -->
      <div v-else class="d-flex flex-column ga-2">
        <v-card
          v-for="evt in eventsHistory.items.value"
          :key="evt.id"
          border
          rounded="lg"
          class="pa-3"
        >
          <div class="d-flex align-center justify-space-between ga-2 mb-1">
            <v-chip
              :color="evt.severity === 'critical' || evt.severity === 'error' ? 'error' : 'warning'"
              size="x-small"
              variant="flat"
              class="font-weight-bold text-uppercase px-2"
            >
              {{ (evt.severity || 'INFO').toUpperCase() }}
            </v-chip>
            <span class="text-caption text-grey d-flex align-center ga-1">
              <v-icon size="12">mdi-clock-outline</v-icon>
              {{ evt.createdAt }}
            </span>
          </div>
          <div class="text-body-2 text-high-emphasis text-break">
            {{ evt.message }}
          </div>
        </v-card>
      </div>

      <template #empty>
        <div class="text-caption text-grey text-center py-4">
          Nenhum outro evento registrado no histórico.
        </div>
      </template>
    </v-infinite-scroll>
  </div>
</template>

<script setup lang="ts">
import { useInfiniteList } from '@/composables/useInfiniteList'

export interface DeviceEventItem {
  id: number
  deviceId: number
  eventType: string
  severity: string
  message: string
  createdAt: string
}

const props = defineProps<{
  deviceId: number
}>()

const eventsHistory = useInfiniteList<DeviceEventItem>(() => `/devices/${props.deviceId}/events`, {
  label: 'histórico de eventos',
})
</script>
