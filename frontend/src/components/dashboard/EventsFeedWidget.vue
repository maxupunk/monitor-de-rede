<template>
  <v-card elevation="2" class="rounded-lg fill-height">
    <v-card-title class="d-flex align-center justify-space-between py-3 px-4">
      <div class="d-flex align-center">
        <v-icon start color="info">mdi-pulse</v-icon>
        <span class="font-weight-bold text-h6">Feed de Eventos em Tempo Real</span>
      </div>
      <v-chip :color="eventsStore.isConnected ? 'success' : 'error'" size="x-small" variant="flat">
        {{ eventsStore.isConnected ? 'Ao Vivo' : 'Desconectado' }}
      </v-chip>
    </v-card-title>
    <v-divider></v-divider>
    <v-card-text class="pa-0">
      <div v-if="eventsStore.recentEvents.length > 0">
        <v-list max-height="360" class="overflow-y-auto pa-0">
          <v-list-item
            v-for="(evt, evtIdx) in eventsStore.recentEvents.slice(0, 10)"
            :key="evtIdx"
            :title="formatEventDetails(evt).title"
            :subtitle="formatEventDetails(evt).message"
            class="px-4 py-2 border-b cursor-pointer"
            @click="emit('open-detail', evt)"
          >
            <template #prepend>
              <v-avatar
                :color="formatEventDetails(evt).color"
                size="32"
                variant="tonal"
                class="mr-3"
              >
                <v-icon size="18">{{ formatEventDetails(evt).icon }}</v-icon>
              </v-avatar>
            </template>
            <template #append>
              <span class="text-caption text-grey">{{ formatEventDetails(evt).time }}</span>
            </template>
          </v-list-item>
        </v-list>
      </div>
      <div v-else class="pa-6 text-center text-grey">
        <v-icon size="44" color="grey-lighten-1" class="mb-2"> mdi-access-point-network </v-icon>
        <div class="text-subtitle-2 font-weight-medium">Aguardando eventos em tempo real...</div>
        <div class="text-caption">
          Mudanças de status e verificações aparecerão aqui automaticamente.
        </div>
      </div>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import { useEventsStore, type RealtimeEventPayload } from '@/stores/events'
import { formatEventDetails } from '@/utils/eventPresentation'

const emit = defineEmits<{
  'open-detail': [event: RealtimeEventPayload]
}>()

const eventsStore = useEventsStore()
</script>

<style scoped>
.cursor-pointer {
  cursor: pointer;
}
</style>
