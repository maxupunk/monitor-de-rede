<template>
  <v-navigation-drawer
    v-model="aiStore.isDrawerOpen"
    location="right"
    temporary
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
        <AiChatMessage v-for="msg in aiStore.messages" :key="msg.id" :message="msg" />
      </template>
    </div>

    <!-- Barra de Entrada Inferior (Instruções no TOPO + Textarea Alto + Ações) -->
    <template #append>
      <div class="pa-3 border-t bg-surface">
        <!-- 1. TEXTO DE EXPLICAÇÃO NO TOPO DO CAMPO -->
        <div
          class="d-flex align-center justify-space-between px-3 py-1 bg-surface-variant rounded-t-lg border-t border-s border-e"
        >
          <div class="d-flex align-center ga-1 text-caption text-medium-emphasis">
            <v-icon size="14" color="primary">mdi-keyboard-outline</v-icon>
            <span>
              <kbd class="kbd-key">Enter</kbd> envia &bull;
              <kbd class="kbd-key">Shift+Enter</kbd> quebra linha
            </span>
          </div>
          <span
            v-if="aiStore.isStreaming"
            class="text-caption text-primary font-weight-medium d-flex align-center ga-1"
          >
            <v-progress-circular indeterminate size="10" width="2" color="primary" />
            Respondendo
          </span>
        </div>

        <!-- 2. CAMPO DE TEXTO MAIS ALTO -->
        <v-textarea
          v-model="inputContent"
          placeholder="Digite sua dúvida ou comando (ex: 'ping no 1.1.1.1')..."
          variant="outlined"
          density="comfortable"
          :rows="3"
          :max-rows="6"
          auto-grow
          hide-details
          class="drawer-chat-textarea"
          @keydown.enter.prevent="handleEnter"
        />

        <!-- 3. BARRA DE AÇÕES INFERIOR -->
        <div
          class="d-flex align-center justify-space-between px-3 py-2 bg-surface-variant rounded-b-lg border-b border-s border-e"
        >
          <div class="d-flex align-center ga-1">
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
            <v-btn
              v-if="inputContent.length > 0"
              variant="text"
              size="x-small"
              color="grey"
              icon="mdi-close"
              @click="inputContent = ''"
            />
          </div>

          <div class="d-flex align-center ga-2">
            <v-btn
              v-if="aiStore.isStreaming"
              color="error"
              variant="flat"
              size="small"
              prepend-icon="mdi-stop-circle-outline"
              @click="aiStore.cancelGeneration()"
            >
              Parar
            </v-btn>
            <v-btn
              v-else
              color="primary"
              variant="flat"
              size="small"
              prepend-icon="mdi-send"
              :disabled="!inputContent.trim() || !aiStore.settings?.enabled"
              @click="handleSend"
            >
              Enviar
            </v-btn>
          </div>
        </div>
      </div>
    </template>
  </v-navigation-drawer>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import { useAiStore } from '@/stores/ai'
import AiChatMessage from './AiChatMessage.vue'
import AiConversationList from './AiConversationList.vue'
import { responseStyleOption } from './aiResponseStyle'

const aiStore = useAiStore()
const inputContent = ref('')
const chatContainer = ref<HTMLElement | null>(null)
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

function handleEnter(e: KeyboardEvent) {
  if (e.shiftKey) {
    inputContent.value += '\n'
  } else {
    handleSend()
  }
}

function handleSend() {
  const text = inputContent.value.trim()
  if (!text || aiStore.isStreaming) return

  inputContent.value = ''
  aiStore.sendMessage(text)
  scrollToBottom()
}

function sendSuggestion(suggestion: string) {
  inputContent.value = suggestion
  handleSend()
}
</script>

<style scoped>
.chat-messages-container {
  height: calc(100vh - 210px);
}

.kbd-key {
  display: inline-block;
  padding: 0.1rem 0.3rem;
  font-size: 0.7rem;
  font-family: monospace;
  background-color: rgba(var(--v-theme-on-surface), 0.08);
  border-radius: 4px;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.15);
}

:deep(.drawer-chat-textarea .v-field) {
  border-radius: 0 !important;
  border-top: none !important;
  border-bottom: none !important;
}
</style>
