<template>
  <div class="zoom-controls" role="group" aria-label="Zoom do mapa">
    <v-tooltip v-for="item in items" :key="item.event" :text="item.label" location="start">
      <template #activator="{ props: tip }">
        <v-btn
          v-bind="tip"
          :icon="item.icon"
          color="primary"
          variant="flat"
          size="small"
          :aria-label="item.label"
          @click="emitFor(item.event)"
        ></v-btn>
      </template>
    </v-tooltip>
    <v-tooltip
      v-if="!compact"
      :text="minimapOpen ? 'Esconder minimapa' : 'Mostrar minimapa'"
      location="start"
    >
      <template #activator="{ props: tip }">
        <v-btn
          v-bind="tip"
          icon="mdi-map-outline"
          :color="minimapOpen ? 'secondary' : 'primary'"
          variant="flat"
          size="small"
          :aria-pressed="minimapOpen"
          aria-label="Minimapa"
          @click="emit('toggle-minimap')"
        ></v-btn>
      </template>
    </v-tooltip>
    <div class="zoom-level" aria-live="polite">{{ Math.round(zoom * 100) }}%</div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'

type ZoomEvent = 'zoom-in' | 'zoom-out' | 'fit' | 'reset'

const ITEMS: ReadonlyArray<{ event: ZoomEvent; icon: string; label: string }> = [
  { event: 'zoom-in', icon: 'mdi-plus', label: 'Aproximar (+)' },
  { event: 'zoom-out', icon: 'mdi-minus', label: 'Afastar (−)' },
  { event: 'fit', icon: 'mdi-fit-to-screen', label: 'Enquadrar o mapa (F)' },
  { event: 'reset', icon: 'mdi-numeric-1-box-outline', label: 'Zoom 100% (0)' },
]

const props = defineProps<{
  zoom: number
  /** Celular: o zoom é por pinça; fica só o "enquadrar". */
  compact?: boolean
  minimapOpen?: boolean
}>()
const emit = defineEmits<{
  'zoom-in': []
  'zoom-out': []
  fit: []
  reset: []
  'toggle-minimap': []
}>()

const items = computed(() => (props.compact ? ITEMS.filter((item) => item.event === 'fit') : ITEMS))

function emitFor(event: ZoomEvent) {
  if (event === 'zoom-in') emit('zoom-in')
  else if (event === 'zoom-out') emit('zoom-out')
  else if (event === 'fit') emit('fit')
  else emit('reset')
}
</script>

<style scoped>
.zoom-controls {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  padding: 6px;
  border-radius: 12px;
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgba(var(--v-theme-on-surface), 0.2);
  box-shadow: 0 4px 14px rgba(0, 0, 0, 0.22);
}
.zoom-level {
  font-size: 0.7rem;
  font-weight: 700;
  color: rgb(var(--v-theme-on-surface));
}
</style>
