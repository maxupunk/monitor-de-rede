<template>
  <span
    class="live-indicator text-caption"
    :class="{ 'live-indicator--off': !events.isConnected }"
    :title="hint"
  >
    <span class="live-indicator__dot"></span>
    {{ events.isConnected ? 'Ao vivo' : 'Reconectando…' }}
    <span v-if="updatedAt" class="live-indicator__time">· {{ formatClockTime(updatedAt) }}</span>
  </span>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useEventsStore } from '@/stores/events'
import { formatClockTime } from '@/utils/formatters'

/**
 * Estado do canal em tempo real (SSE) junto dos dados que ele alimenta: verde
 * e pulsando quando conectado, âmbar quando reconectando — e a hora da última
 * amostra, para quem quer saber se o número é de agora.
 */
defineProps<{ updatedAt?: string | null }>()

const events = useEventsStore()

const hint = computed(() =>
  events.isConnected
    ? 'Os dados chegam pelo canal em tempo real (SSE), sem recarregar a página.'
    : 'O canal em tempo real caiu e está reconectando; os dados retomam sozinhos.'
)
</script>

<style scoped>
.live-indicator {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 2px 10px;
  border-radius: 999px;
  color: rgb(var(--v-theme-success));
  background: rgba(var(--v-theme-success), 0.12);
  font-weight: 500;
  white-space: nowrap;
  cursor: default;
}

.live-indicator--off {
  color: rgb(var(--v-theme-warning));
  background: rgba(var(--v-theme-warning), 0.12);
}

.live-indicator__time {
  color: rgba(var(--v-theme-on-surface), var(--v-medium-emphasis-opacity));
  font-weight: 400;
}

.live-indicator__dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: currentColor;
  animation: live-pulse 1.6s ease-in-out infinite;
}

.live-indicator--off .live-indicator__dot {
  animation: none;
}

@keyframes live-pulse {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.35;
  }
}

@media (prefers-reduced-motion: reduce) {
  .live-indicator__dot {
    animation: none;
  }
}
</style>
