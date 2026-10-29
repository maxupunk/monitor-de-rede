<template>
  <DashboardCard
    title="Eventos em tempo real"
    icon="mdi-pulse"
    icon-color="info"
    subtitle="Mudanças de status e verificações"
    to="/events"
    flush
  >
    <template #actions>
      <LiveIndicator />
    </template>

    <v-list v-if="items.length > 0" max-height="360" class="overflow-y-auto py-0">
      <v-list-item
        v-for="item in items"
        :key="item.key"
        :title="item.details.title"
        :subtitle="item.details.message"
        class="px-5"
        @click="openDetail(item.event)"
      >
        <template #prepend>
          <v-avatar :color="item.details.color" size="32" variant="tonal" rounded="lg">
            <v-icon size="18">{{ item.details.icon }}</v-icon>
          </v-avatar>
        </template>
        <template #append>
          <span class="text-caption font-weight-medium">{{ item.details.time }}</span>
        </template>
      </v-list-item>
    </v-list>

    <div v-else class="dashboard-empty">
      <v-icon size="40" color="info">mdi-access-point-network</v-icon>
      <div class="text-subtitle-2 font-weight-bold mt-2">Aguardando eventos…</div>
      <div class="text-body-2">Mudanças de status aparecem aqui automaticamente.</div>
    </div>

    <EventDetailDialog v-model="detailOpen" :event="selected" />
  </DashboardCard>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import DashboardCard from '@/components/dashboard/DashboardCard.vue'
import EventDetailDialog from '@/components/EventDetailDialog.vue'
import LiveIndicator from '@/components/LiveIndicator.vue'
import { useEventsStore, type RealtimeEventPayload } from '@/stores/events'
import { formatEventDetails } from '@/utils/eventPresentation'

const eventsStore = useEventsStore()
const detailOpen = ref(false)
const selected = ref<RealtimeEventPayload | null>(null)

/** Formata uma vez por evento, não três vezes por linha a cada render. */
const items = computed(() =>
  eventsStore.recentEvents.slice(0, 10).map((event, index) => ({
    key: index,
    event,
    details: formatEventDetails(event),
  }))
)

function openDetail(event: RealtimeEventPayload) {
  selected.value = event
  detailOpen.value = true
}
</script>
