<template>
  <div class="ai-sidebar">
    <div class="ai-sidebar__head">
      <v-icon color="primary" size="20">mdi-forum-outline</v-icon>
      <span class="text-title-small font-weight-bold">Conversas</span>
    </div>

    <AiConversationList fill class="ai-sidebar__list" @selected="emit('selected')" />

    <div class="ai-sidebar__footer app-footer-bar">
      <v-avatar size="32" color="primary" variant="tonal">
        <v-icon size="18">mdi-robot-outline</v-icon>
      </v-avatar>
      <div class="ai-sidebar__model">
        <div class="text-body-medium font-weight-bold text-truncate">{{ driverLabel }}</div>
        <div class="text-body-small font-mono text-truncate" :title="modelLabel">
          {{ modelLabel }}
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useAiModelInfo } from '@/composables/useAiModelInfo'
import AiConversationList from './AiConversationList.vue'

/** Coluna de conversas do chat em tela cheia (e o painel dela no celular). */
const emit = defineEmits<{ selected: [] }>()

const { driverLabel, modelLabel } = useAiModelInfo()
</script>

<style scoped>
.ai-sidebar {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}

.ai-sidebar__head {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 0 0 auto;
  min-height: 56px;
  padding: 0 16px;
  border-bottom: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
}

.ai-sidebar__list {
  flex: 1 1 auto;
  min-height: 0;
  padding: 12px;
}

.ai-sidebar__footer {
  gap: 10px;
  flex: 0 0 auto;
  padding: 0 16px;
  border-top: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
}

.ai-sidebar__model {
  flex: 1 1 auto;
  min-width: 0;
  line-height: 1.3;
}
</style>
