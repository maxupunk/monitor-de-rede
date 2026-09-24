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
    <header class="ai-drawer__header">
      <v-btn
        v-if="view === 'history'"
        icon="mdi-arrow-left"
        size="small"
        variant="text"
        aria-label="Voltar para a conversa"
        title="Voltar para a conversa"
        @click="view = 'chat'"
      />
      <v-avatar v-else size="32" color="primary">
        <v-icon size="18" color="white">mdi-robot-outline</v-icon>
      </v-avatar>

      <div class="ai-drawer__title">
        <div class="text-title-small font-weight-bold">
          {{ view === 'history' ? 'Conversas salvas' : 'Assistente IA' }}
        </div>
        <div
          v-if="view === 'chat'"
          class="text-body-small text-medium-emphasis font-mono text-truncate"
        >
          {{ modelLabel }}
        </div>
      </div>

      <div class="ai-drawer__actions">
        <v-btn
          v-if="view === 'chat'"
          icon="mdi-history"
          size="small"
          variant="text"
          aria-label="Conversas salvas"
          title="Conversas salvas"
          @click="view = 'history'"
        />
        <v-btn
          v-if="view === 'chat'"
          icon="mdi-plus"
          size="small"
          variant="text"
          aria-label="Nova conversa"
          title="Nova conversa"
          :disabled="aiStore.messages.length === 0 || aiStore.isStreaming"
          @click="aiStore.newConversation()"
        />
        <v-btn
          icon="mdi-open-in-new"
          size="small"
          variant="text"
          to="/ai-chat"
          aria-label="Abrir em tela cheia"
          title="Abrir em tela cheia"
          @click="close"
        />
        <v-btn
          icon="mdi-close"
          size="small"
          variant="text"
          aria-label="Fechar o assistente"
          title="Fechar (Esc)"
          @click="close"
        />
      </div>
    </header>

    <div v-if="view === 'history'" class="ai-drawer__history pa-3">
      <AiConversationList @selected="view = 'chat'" />
    </div>

    <AiChatThread v-else ref="thread" compact :placeholder="placeholder" @navigate="close">
      <template #status>
        <v-chip
          size="x-small"
          color="primary"
          variant="tonal"
          :prepend-icon="responseStyle.icon"
          :title="responseStyle.hint"
        >
          {{ responseStyle.title }}
        </v-chip>
        <v-chip
          v-if="aiStore.settings?.allowActiveTools"
          size="x-small"
          color="success"
          variant="tonal"
          title="A IA pode rodar ping, traceroute e testes de porta"
        >
          Ferramentas
        </v-chip>
      </template>
    </AiChatThread>
  </v-navigation-drawer>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useDisplay } from 'vuetify'
import { useAiStore } from '@/stores/ai'
import { useAiModelInfo } from '@/composables/useAiModelInfo'
import AiChatThread from './AiChatThread.vue'
import AiConversationList from './AiConversationList.vue'
import { responseStyleOption } from './aiResponseStyle'

/** Largura do painel fora do celular. */
const PANEL_WIDTH = 480

const aiStore = useAiStore()
const display = useDisplay()
const { modelLabel } = useAiModelInfo()
const thread = ref<InstanceType<typeof AiChatThread> | null>(null)
const view = ref<'chat' | 'history'>('chat')

/*
 * Precisa ser número: o drawer do Vuetify calcula o deslocamento de fechar a
 * partir da largura. Com `'100%'` a conta dava inválida, o `translateX(0)` de
 * aberto ficava no lugar e o painel continuava na frente de tudo — sem
 * receber cliques — depois do X.
 */
const drawerWidth = computed(() =>
  display.xs.value ? Math.max(display.width.value, 280) : PANEL_WIDTH
)

const placeholder = computed(() =>
  display.xs.value
    ? 'Pergunte sobre a rede…'
    : "Sua dúvida ou comando (ex: 'ping no 1.1.1.1'). @ marca um recurso…"
)

const responseStyle = computed(() => responseStyleOption(aiStore.settings?.responseStyle))

function close() {
  aiStore.isDrawerOpen = false
}

watch(
  () => aiStore.isDrawerOpen,
  async (open) => {
    if (!open) {
      view.value = 'chat'
      return
    }
    // No celular, focar abriria o teclado por cima da conversa.
    if (!display.mobile.value) {
      await nextTick()
      thread.value?.focus()
    }
  }
)

/** Esc fecha — primeiro o histórico, depois o painel. Menus abertos (o @) vêm antes. */
function onKeydown(event: KeyboardEvent) {
  if (event.key !== 'Escape' || !aiStore.isDrawerOpen || event.defaultPrevented) return
  if (view.value === 'history') view.value = 'chat'
  else close()
}

onMounted(async () => {
  window.addEventListener('keydown', onKeydown)
  if (!aiStore.settings) await aiStore.loadSettings()
})

onBeforeUnmount(() => window.removeEventListener('keydown', onKeydown))
</script>

<style scoped>
/* Cabeçalho, conversa e campo dividem a altura: só as mensagens rolam. */
.ai-drawer :deep(.v-navigation-drawer__content) {
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.ai-drawer__header {
  display: flex;
  align-items: center;
  gap: 10px;
  flex: 0 0 auto;
  padding: 8px 8px 8px 12px;
  border-bottom: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
  background: rgb(var(--v-theme-surface));
}

.ai-drawer__title {
  flex: 1 1 auto;
  min-width: 0;
  line-height: 1.25;
}

.ai-drawer__actions {
  display: flex;
  align-items: center;
  flex: 0 0 auto;
}

.ai-drawer__history {
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
}
</style>
