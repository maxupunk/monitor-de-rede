<template>
  <div class="ai-conversation-list">
    <v-btn
      block
      color="primary"
      variant="tonal"
      size="small"
      prepend-icon="mdi-plus"
      class="mb-2"
      :disabled="aiStore.isStreaming"
      @click="startNew"
    >
      Nova conversa
    </v-btn>

    <v-alert
      v-if="conversations.error"
      type="error"
      variant="tonal"
      density="compact"
      class="mb-2 text-body-small"
      closable
      @click:close="conversations.error = null"
    >
      {{ conversations.error }}
    </v-alert>

    <v-progress-linear v-if="conversations.loading" indeterminate color="primary" class="mb-2" />

    <div
      v-if="conversations.summaries.length === 0 && !conversations.loading"
      class="text-body-small text-medium-emphasis text-center py-3"
    >
      As conversas ficam salvas na sua conta e aparecem em qualquer computador.
    </div>

    <v-list v-else density="compact" nav class="pa-0 conversation-scroll">
      <v-list-item
        v-for="item in conversations.summaries"
        :key="item.id"
        :title="item.title"
        :subtitle="formatRelativeTime(item.updatedAt)"
        :active="item.id === conversations.activeId"
        color="primary"
        rounded="lg"
        :disabled="aiStore.isStreaming"
        @click="open(item.id)"
      >
        <template #append>
          <v-btn
            icon="mdi-delete-outline"
            size="x-small"
            variant="text"
            color="error"
            title="Apagar conversa"
            @click.stop="aiStore.deleteConversation(item.id)"
          />
        </template>
      </v-list-item>
    </v-list>
  </div>
</template>

<script setup lang="ts">
import { onMounted } from 'vue'
import { useAiStore } from '@/stores/ai'
import { useAiConversationsStore } from '@/stores/aiConversations'
import { formatRelativeTime } from '@/utils/formatters'

const emit = defineEmits<{
  (e: 'selected'): void
}>()

const aiStore = useAiStore()
const conversations = useAiConversationsStore()

// Sempre do servidor ao abrir: pode ter conversa nova vinda de outro computador.
onMounted(() => void conversations.load())

function startNew() {
  aiStore.newConversation()
  emit('selected')
}

async function open(id: number) {
  await aiStore.openConversation(id)
  emit('selected')
}
</script>

<style scoped>
.conversation-scroll {
  max-height: 320px;
  overflow-y: auto;
}
</style>
