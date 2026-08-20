<template>
  <div>
    <v-infinite-scroll :key="eventsHistory.scrollKey.value" @load="eventsHistory.load">
      <div class="table-responsive">
        <v-table hover density="comfortable" class="rounded-lg border">
          <thead>
            <tr>
              <th>Severidade</th>
              <th>Mensagem</th>
              <th>Data/Hora</th>
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
                >
                  {{ (evt.severity || 'INFO').toUpperCase() }}
                </v-chip>
              </td>
              <td>{{ evt.message }}</td>
              <td>{{ evt.createdAt }}</td>
            </tr>
          </tbody>
        </v-table>
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
