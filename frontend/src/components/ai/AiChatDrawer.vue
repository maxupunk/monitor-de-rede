<template>
  <v-navigation-drawer
    v-model="aiStore.isDrawerOpen"
    location="right"
    temporary
    touchless
    elevation="4"
    :width="$vuetify.display.xs ? '100%' : 480"
    class="ai-drawer"
  >
    <!-- Cabeçalho do Drawer -->
    <div class="pa-3 border-b d-flex align-center justify-space-between bg-surface">
      <div class="d-flex align-center ga-2">
        <v-avatar size="32" color="primary" class="elevation-1">
          <v-icon size="18" color="white">mdi-robot-outline</v-icon>
        </v-avatar>
        <div>
          <div class="text-subtitle-2 font-weight-bold lh-1">Assistente IA</div>
          <div class="text-caption text-medium-emphasis font-mono">
            {{ activeModelLabel }}
          </div>
        </div>
      </div>

      <div class="d-flex align-center ga-1">
        <v-tooltip location="bottom" text="Abrir em tela cheia">
          <template #activator="{ props: tipProps }">
            <v-btn
              v-bind="tipProps"
              icon
              size="small"
              variant="text"
              to="/ai-chat"
              @click="aiStore.isDrawerOpen = false"
            >
              <v-icon size="18">mdi-open-in-new</v-icon>
            </v-btn>
          </template>
        </v-tooltip>

        <v-menu v-model="historyOpen" location="bottom end" :close-on-content-click="false">
          <template #activator="{ props: menuProps }">
            <v-btn
              v-bind="menuProps"
              icon
              size="small"
              variant="text"
              color="primary"
              title="Conversas salvas"
            >
              <v-icon size="18">mdi-history</v-icon>
            </v-btn>
          </template>
          <v-card class="pa-3" min-width="280" max-width="340">
            <AiConversationList @selected="historyOpen = false" />
          </v-card>
        </v-menu>

        <v-tooltip location="bottom" text="Nova conversa">
          <template #activator="{ props: tipProps }">
            <v-btn
              v-bind="tipProps"
              icon
              size="small"
              variant="text"
              color="primary"
              :disabled="aiStore.messages.length === 0 || aiStore.isStreaming"
              @click="aiStore.newConversation()"
            >
              <v-icon size="18">mdi-plus</v-icon>
            </v-btn>
          </template>
        </v-tooltip>

        <v-btn icon size="small" variant="text" @click="aiStore.isDrawerOpen = false">
          <v-icon size="20">mdi-close</v-icon>
        </v-btn>
      </div>
    </div>

    <!-- Área de Mensagens -->
    <div ref="chatContainer" class="pa-3 chat-messages-container overflow-y-auto">
      <!-- Aviso se a IA estiver desativada -->
      <v-alert
        v-if="aiStore.settings && !aiStore.settings.enabled"
        type="warning"
        variant="tonal"
        density="compact"
        class="mb-3"
      >
        O Assistente IA está desativado. Ative-o em
        <router-link
          to="/settings"
          class="text-decoration-underline font-weight-bold"
          @click="aiStore.isDrawerOpen = false"
        >
          Configurações
        </router-link>
        .
      </v-alert>

      <!-- Estado Inicial / Sugestões -->
      <div v-if="aiStore.messages.length === 0" class="text-center py-6">
        <v-avatar size="56" color="primary" variant="tonal" class="mb-3">
          <v-icon size="32">mdi-sparkles</v-icon>
        </v-avatar>
        <div class="text-subtitle-1 font-weight-bold mb-1">Como posso ajudar?</div>
        <p class="text-caption text-medium-emphasis mb-4 px-2">
          Faça perguntas sobre a operação do NetMonitor ou solicite diagnósticos ativos na rede.
        </p>

        <div class="d-flex flex-column ga-2 text-left">
          <v-chip
            v-for="suggestion in suggestions"
            :key="suggestion"
            size="small"
            variant="outlined"
            color="primary"
            class="justify-start pa-3 cursor-pointer"
            @click="sendSuggestion(suggestion)"
          >
            <v-icon start size="14">mdi-lightbulb-outline</v-icon>
            <span class="text-truncate">{{ suggestion }}</span>
          </v-chip>
        </div>
      </div>

      <!-- Histórico de Mensagens -->
      <template v-else>
        <AiChatMessage
          v-for="msg in aiStore.messages"
          :key="msg.id"
          :message="msg"
          @rewind="handleRewind"
        />
      </template>
    </div>

    <!-- Campo da pergunta, com @ para marcar recursos -->
    <template #append>
      <div class="pa-3 border-t bg-surface">
        <AiChatComposer
          ref="composer"
          compact
          :max-rows="6"
          placeholder="Sua dúvida ou comando (ex: 'ping no 1.1.1.1'). @ marca um recurso..."
        >
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
            >
              Ferramentas
            </v-chip>
          </template>
        </AiChatComposer>
      </div>
    </template>
  </v-navigation-drawer>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import { useAiStore } from '@/stores/ai'
import AiChatMessage from './AiChatMessage.vue'
import AiChatComposer from './AiChatComposer.vue'
import AiConversationList from './AiConversationList.vue'
import { responseStyleOption } from './aiResponseStyle'

const aiStore = useAiStore()
const chatContainer = ref<HTMLElement | null>(null)
const composer = ref<InstanceType<typeof AiChatComposer> | null>(null)
const historyOpen = ref(false)

const activeModelLabel = computed(() => {
  const settings = aiStore.settings
  if (!settings) return 'Carregando...'
  if (settings.activeDriver === 'opencode') {
    return settings.opencodeModel || 'muse-spark-1.3-contributor-free'
  }
  if (settings.activeDriver === 'openrouter') {
    return settings.openrouterModel || 'openrouter/free'
  }
  if (settings.activeDriver === 'ollama') {
    return settings.ollamaModel || 'llama3.2'
  }
  return 'Padrão'
})

const responseStyle = computed(() => responseStyleOption(aiStore.settings?.responseStyle))

const suggestions = [
  'Testar conectividade com a Internet',
  'Resumir os alertas críticos recentes',
  'Mostrar o gráfico de latência da última hora',
  'Quais interfaces estão caídas ou saturadas?',
]

onMounted(async () => {
  if (!aiStore.settings) {
    await aiStore.loadSettings()
  }
})

function scrollToBottom() {
  nextTick(() => {
    if (chatContainer.value) {
      chatContainer.value.scrollTop = chatContainer.value.scrollHeight
    }
  })
}

watch(
  () => aiStore.messages.length,
  () => scrollToBottom()
)

watch(
  () => aiStore.messages[aiStore.messages.length - 1]?.content,
  () => scrollToBottom()
)

function sendSuggestion(suggestion: string) {
  if (aiStore.isStreaming || !aiStore.settings?.enabled) return
  void aiStore.sendMessage(suggestion)
}

function handleRewind(messageId: string) {
  const draft = aiStore.rewind(messageId)
  if (draft) composer.value?.setDraft(draft)
}
</script>

<style scoped>
/*
 * Cabeçalho, mensagens e campo dividem a altura do drawer: só as mensagens
 * rolam. A altura fixa em `100vh` empurrava o campo para fora da tela no
 * celular, onde o `vh` inclui a barra de endereço do navegador.
 */
.ai-drawer :deep(.v-navigation-drawer__content) {
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.chat-messages-container {
  flex: 1 1 auto;
  min-height: 0;
}

/* A lista do @ abre para cima, por cima das mensagens: o rodapé não pode cortá-la. */
.ai-drawer :deep(.v-navigation-drawer__append) {
  overflow: visible;
}
</style>
