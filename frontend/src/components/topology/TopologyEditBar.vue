<template>
  <div class="edit-bar" role="region" aria-label="Modo de edição do mapa">
    <v-icon :color="cableActive ? 'secondary' : 'warning'" class="pulse">
      {{ cableActive ? 'mdi-cable-data' : 'mdi-cursor-move' }}
    </v-icon>
    <span class="message">{{ message }}</span>
    <v-btn
      v-if="!cableActive"
      color="info"
      variant="flat"
      size="small"
      prepend-icon="mdi-cable-data"
      @click="$emit('cable')"
    >
      Puxar cabo
    </v-btn>
    <v-btn
      v-else
      color="error"
      variant="flat"
      size="small"
      prepend-icon="mdi-close"
      @click="$emit('cancel-cable')"
    >
      Cancelar cabo
    </v-btn>
    <v-btn
      color="success"
      variant="flat"
      size="small"
      prepend-icon="mdi-check"
      @click="$emit('done')"
    >
      Concluir
    </v-btn>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{ cableActive: boolean; cableSourceName: string | null }>()
defineEmits<{ cable: []; 'cancel-cable': []; done: [] }>()

const message = computed(() => {
  if (!props.cableActive)
    return 'Editando: arraste os dispositivos; as posições são salvas sozinhas.'
  if (!props.cableSourceName) return 'Clique no dispositivo de ORIGEM do cabo.'
  return `Origem: ${props.cableSourceName}. Agora clique no DESTINO (Esc cancela).`
})
</script>

<style scoped>
.edit-bar {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px 10px;
  max-width: min(720px, calc(100vw - 32px));
  padding: 6px 8px 6px 12px;
  border-radius: 12px;
  background: rgb(var(--v-theme-surface));
  border: 2px solid rgb(var(--v-theme-warning));
  box-shadow: 0 6px 18px rgba(0, 0, 0, 0.3);
  color: rgb(var(--v-theme-on-surface));
  font-size: 0.85rem;
  font-weight: 600;
}
.message {
  flex: 1;
  min-width: 180px;
}
.pulse {
  animation: pulse 1.6s ease-in-out infinite;
}
@keyframes pulse {
  50% {
    transform: scale(1.15);
  }
}
@media (prefers-reduced-motion: reduce) {
  .pulse {
    animation: none;
  }
}
</style>
