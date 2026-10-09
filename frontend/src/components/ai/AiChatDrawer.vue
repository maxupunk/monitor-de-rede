<template>
  <v-navigation-drawer
    v-model="aiStore.isDrawerOpen"
    location="right"
    temporary
    touchless
    elevation="4"
    :width="drawerWidth"
    class="ai-drawer"
    aria-label="Assistente IA"
  >
    <!--
      Sem v-tooltip nos botões: no toque a dica abre no tap e fica presa sobre
      o painel. `title` + `aria-label` bastam no desktop e no leitor de tela.
    -->
    <AiChatHeader
      compact
      :title="view === 'history' ? 'Conversas salvas' : 'Assistente IA'"
      :subtitle="view === 'chat' ? modelLabel : ''"
    >
      <template v-if="view === 'history'" #leading>
        <v-btn
          icon="mdi-arrow-left"
          size="small"
          variant="text"
          color="primary"
          aria-label="Voltar para a conversa"
          title="Voltar para a conversa"
          @click="view = 'chat'"
        />
      </template>
      <template #actions>
        <v-btn
          v-if="view === 'chat'"
          icon="mdi-history"
          size="small"
          variant="text"
          color="primary"
          aria-label="Conversas salvas"
          title="Conversas salvas"
          @click="view = 'history'"
        />
        <AiNewConversationButton v-if="view === 'chat'" size="small" variant="text" />
        <v-btn
          icon="mdi-open-in-new"
          size="small"
          variant="text"
          color="primary"
          to="/ai-chat"
          aria-label="Abrir em tela cheia"
          title="Abrir em tela cheia"
          @click="close"
        />
        <v-btn
          icon="mdi-close"
          size="small"
          variant="text"
          color="primary"
          aria-label="Fechar o assistente"
          title="Fechar (Esc)"
          @click="close"
        />
      </template>
    </AiChatHeader>

    <div v-if="view === 'history'" class="ai-drawer__history pa-3">
      <AiConversationList @selected="view = 'chat'" />
    </div>

    <AiChatThread v-else ref="thread" compact :placeholder="placeholder" @navigate="close" />
  </v-navigation-drawer>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useDisplay } from 'vuetify'
import { useAiStore } from '@/stores/ai'
import { useAiModelInfo } from '@/composables/useAiModelInfo'
import { useAiChatShell } from '@/composables/useAiChatShell'
import AiChatHeader from './AiChatHeader.vue'
import AiChatThread from './AiChatThread.vue'
import AiConversationList from './AiConversationList.vue'
import AiNewConversationButton from './AiNewConversationButton.vue'

/** Largura do painel fora do celular. */
const PANEL_WIDTH = 480

const aiStore = useAiStore()
const display = useDisplay()
const { modelLabel } = useAiModelInfo()
const thread = ref<InstanceType<typeof AiChatThread> | null>(null)
const view = ref<'chat' | 'history'>('chat')
const { placeholder, focusThread } = useAiChatShell(thread)

/*
 * Precisa ser número: o drawer do Vuetify calcula o deslocamento de fechar a
 * partir da largura. Com `'100%'` a conta dava inválida, o `translateX(0)` de
 * aberto ficava no lugar e o painel continuava na frente de tudo — sem
 * receber cliques — depois do X.
 */
const drawerWidth = computed(() =>
  display.xs.value ? Math.max(display.width.value, 280) : PANEL_WIDTH
)

function close() {
  aiStore.isDrawerOpen = false
}

watch(
  () => aiStore.isDrawerOpen,
  (open) => {
    if (!open) {
      view.value = 'chat'
      return
    }
    void focusThread()
  }
)

/** Esc fecha — primeiro o histórico, depois o painel. Menus abertos (o @) vêm antes. */
function onKeydown(event: KeyboardEvent) {
  if (event.key !== 'Escape' || !aiStore.isDrawerOpen || event.defaultPrevented) return
  if (view.value === 'history') view.value = 'chat'
  else close()
}

onMounted(() => window.addEventListener('keydown', onKeydown))

onBeforeUnmount(() => window.removeEventListener('keydown', onKeydown))
</script>

<style scoped>
/* Cabeçalho, conversa e campo dividem a altura: só as mensagens rolam. */
.ai-drawer :deep(.v-navigation-drawer__content) {
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.ai-drawer__history {
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
}
</style>
