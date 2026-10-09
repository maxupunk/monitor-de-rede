<template>
  <div class="ai-conversation-list" :class="{ 'ai-conversation-list--fill': fill }">
    <AiNewConversationButton labeled class="mb-2 flex-grow-0" @started="emit('selected')" />

    <v-text-field
      v-if="conversations.summaries.length > SEARCH_FROM"
      v-model="search"
      placeholder="Buscar conversa"
      prepend-inner-icon="mdi-magnify"
      variant="outlined"
      density="compact"
      hide-details
      clearable
      class="mb-2 flex-grow-0"
    ></v-text-field>

    <v-alert
      v-if="conversations.error"
      type="error"
      variant="tonal"
      density="compact"
      class="mb-2 text-body-small flex-grow-0"
      closable
      @click:close="conversations.error = null"
    >
      {{ conversations.error }}
    </v-alert>

    <v-progress-linear
      v-if="conversations.loading"
      indeterminate
      color="primary"
      class="mb-2 flex-grow-0"
    ></v-progress-linear>

    <div
      v-if="visibleSummaries.length === 0 && !conversations.loading"
      class="text-body-medium text-center py-4 px-2"
    >
      {{
        search
          ? 'Nenhuma conversa com esse título.'
          : 'Suas conversas ficam salvas na conta e aparecem em qualquer computador.'
      }}
    </div>

    <v-list v-else density="compact" nav class="pa-0 conversation-scroll">
      <v-list-item
        v-for="item in visibleSummaries"
        :key="item.id"
        :title="item.title"
        :subtitle="formatRelativeTime(item.updatedAt)"
        :active="item.id === conversations.activeId"
        color="primary"
        rounded="lg"
        prepend-icon="mdi-message-text-outline"
        :disabled="aiStore.isStreaming"
        @click="open(item.id)"
      >
        <template #append>
          <v-btn
            icon="mdi-delete-outline"
            size="small"
            variant="text"
            color="error"
            :aria-label="`Apagar a conversa ${item.title}`"
            title="Apagar conversa"
            @click.stop="remove(item.id, item.title)"
          ></v-btn>
        </template>
      </v-list-item>
    </v-list>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useAiStore } from '@/stores/ai'
import { useAiConversationsStore } from '@/stores/aiConversations'
import { confirm } from '@/composables/useConfirm'
import { formatRelativeTime } from '@/utils/formatters'
import AiNewConversationButton from './AiNewConversationButton.vue'

/** A busca só aparece quando a lista deixa de caber de uma olhada. */
const SEARCH_FROM = 6

withDefaults(
  defineProps<{
    /** Ocupa a altura de quem hospeda e rola por dentro (coluna lateral). */
    fill?: boolean
  }>(),
  { fill: false }
)

const emit = defineEmits<{
  (e: 'selected'): void
}>()

const aiStore = useAiStore()
const conversations = useAiConversationsStore()
const search = ref<string | null>('')

const visibleSummaries = computed(() => {
  const term = (search.value ?? '').trim().toLowerCase()
  return [...conversations.summaries]
    .filter((item) => !term || item.title.toLowerCase().includes(term))
    .sort((a, b) => new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime())
})

// Sempre do servidor ao abrir: pode ter conversa nova vinda de outro computador.
onMounted(() => void conversations.load())

async function open(id: number) {
  await aiStore.openConversation(id)
  emit('selected')
}

async function remove(id: number, title: string) {
  const ok = await confirm({
    title: 'Apagar conversa',
    message: `A conversa "${title}" será apagada de todos os seus computadores.`,
    confirmText: 'Apagar',
    confirmColor: 'error',
    icon: 'mdi-delete-outline',
  })
  if (ok) await aiStore.deleteConversation(id)
}
</script>

<style scoped>
.ai-conversation-list {
  display: flex;
  flex-direction: column;
}

.conversation-scroll {
  max-height: 320px;
  overflow-y: auto;
  background: transparent;
}

.ai-conversation-list--fill {
  height: 100%;
  min-height: 0;
}

.ai-conversation-list--fill .conversation-scroll {
  flex: 1 1 auto;
  max-height: none;
  min-height: 0;
}
</style>
