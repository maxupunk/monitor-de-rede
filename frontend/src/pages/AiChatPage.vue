<template>
  <div class="ai-chat-page fill-height d-flex flex-column">
    <PageHeader
      title="Assistente IA — Diagnóstico & Suporte"
      subtitle="Engenheiro de redes virtual para análise de conectividade, diagnóstico de falhas e suporte operacional"
    >
      <template #actions>
        <v-btn
          variant="outlined"
          color="primary"
          size="small"
          prepend-icon="mdi-delete-outline"
          :disabled="aiStore.messages.length === 0"
          @click="aiStore.clearMessages()"
        >
          Limpar Conversa
        </v-btn>
        <v-btn
          variant="text"
          color="grey-darken-1"
          size="small"
          prepend-icon="mdi-cog-outline"
          to="/settings"
          class="ms-2"
        >
          Configurações
        </v-btn>
      </template>
    </PageHeader>

    <v-row class="flex-grow-1 overflow-hidden" dense>
      <!-- Coluna Lateral de Ações Rápidas e Contexto -->
      <v-col cols="12" md="4" lg="3" class="d-none d-md-flex flex-column ga-3">
        <!-- Card: Status do Modelo Ativo -->
        <v-card class="rounded-lg pa-3" variant="outlined">
          <div class="d-flex align-center justify-space-between mb-2">
            <span class="text-caption font-weight-bold text-uppercase text-medium-emphasis">
              Motor de IA
            </span>
            <v-chip
              size="x-small"
              :color="aiStore.settings?.enabled ? 'success' : 'warning'"
              variant="tonal"
            >
              {{ aiStore.settings?.enabled ? 'Ativo' : 'Desativado' }}
            </v-chip>
          </div>
          <div class="d-flex align-center ga-2 mb-1">
            <v-avatar size="28" color="primary" variant="tonal">
              <v-icon size="16">mdi-robot</v-icon>
            </v-avatar>
            <div class="overflow-hidden">
              <div class="text-subtitle-2 font-weight-bold text-truncate">
                {{ activeDriverLabel }}
              </div>
              <div class="text-caption text-medium-emphasis font-mono text-truncate">
                {{ activeModelLabel }}
              </div>
            </div>
          </div>
        </v-card>

        <!-- Card: Ações Rápidas -->
        <v-card class="rounded-lg pa-3" variant="outlined">
          <div class="d-flex align-center ga-2 mb-2">
            <v-icon color="primary" size="18">mdi-flash-outline</v-icon>
            <span class="text-subtitle-2 font-weight-bold">Ações Rápidas</span>
          </div>
          <div class="d-flex flex-column ga-2">
            <v-btn
              v-for="s in quickPrompts"
              :key="s"
              variant="tonal"
              color="primary"
              size="small"
              class="justify-start text-none text-caption text-wrap py-2"
              prepend-icon="mdi-chevron-right"
              @click="handleQuickPrompt(s)"
            >
              {{ s }}
            </v-btn>
          </div>
        </v-card>

        <!-- Card: Capacidades do Agente -->
        <v-card class="rounded-lg pa-3 flex-grow-1" variant="outlined">
          <div class="d-flex align-center ga-2 mb-2">
            <v-icon color="info" size="18">mdi-shield-check-outline</v-icon>
            <span class="text-subtitle-2 font-weight-bold">Ferramentas Autônomas</span>
          </div>
          <ul class="text-caption text-medium-emphasis pl-4 d-flex flex-column ga-2">
            <li><strong>ICMP Ping:</strong> Medição de RTT médio, jitter e perda de pacotes.</li>
            <li>
              <strong>Traceroute:</strong> Rastreamento de saltos e identificação de rotas
              degradadas.
            </li>
            <li>
              <strong>Port Scan:</strong> Verificação de conectividade TCP em portas essenciais.
            </li>
            <li><strong>Playbooks:</strong> Checklists completos de internet e equipamentos.</li>
            <li>
              <strong>Base de Conhecimento:</strong> Orientações sobre SNMP, WireGuard VPN e
              alertas.
            </li>
          </ul>
        </v-card>
      </v-col>

      <!-- Janela Central do Chat -->
      <v-col cols="12" md="8" lg="9" class="d-flex flex-column fill-height">
        <v-card
          class="rounded-lg d-flex flex-column flex-grow-1 overflow-hidden"
          variant="outlined"
        >
          <!-- Barra Superior de Contexto dentro do Chat -->
          <div
            class="px-4 py-2 border-b bg-surface d-flex align-center justify-space-between flex-wrap ga-2"
          >
            <div class="d-flex align-center ga-2">
              <v-icon color="primary" size="20">mdi-chat-processing-outline</v-icon>
              <span class="text-subtitle-2 font-weight-bold">Canal de Diagnóstico Interativo</span>
              <v-chip size="x-small" color="primary" variant="outlined" class="font-mono">
                {{ activeModelLabel }}
              </v-chip>
            </div>
            <div class="d-flex align-center ga-2">
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
                prepend-icon="mdi-wrench-check"
              >
                Ferramentas Ativas
              </v-chip>
              <v-btn
                v-if="aiStore.messages.length > 0"
                variant="text"
                color="grey"
                size="x-small"
                prepend-icon="mdi-delete-outline"
                @click="aiStore.clearMessages()"
              >
                Limpar
              </v-btn>
            </div>
          </div>

          <!-- Área de Mensagens / Histórico -->
          <div ref="chatContainer" class="pa-4 flex-grow-1 overflow-y-auto chat-scroll-area">
            <!-- Alerta se o assistente estiver desativado -->
            <v-alert
              v-if="aiStore.settings && !aiStore.settings.enabled"
              type="warning"
              variant="tonal"
              density="compact"
              class="mb-4"
              icon="mdi-alert-circle-outline"
            >
              O Assistente IA está desativado nas configurações do sistema.
              <router-link to="/settings" class="font-weight-bold text-decoration-underline ms-1">
                Clique aqui para ativar.
              </router-link>
            </v-alert>

            <!-- Estado Vazio com Sugestões Categorizadas -->
            <div v-if="aiStore.messages.length === 0" class="text-center py-6 px-2">
              <v-avatar size="64" color="primary" variant="tonal" class="mb-3 elevation-1">
                <v-icon size="36">mdi-robot-outline</v-icon>
              </v-avatar>
              <div class="text-h6 font-weight-bold mb-1">Como posso te ajudar hoje?</div>
              <p class="text-body-2 text-medium-emphasis max-w-500 mx-auto mb-6">
                Envie perguntas sobre a topologia e métricas da rede, execute diagnósticos ativos de
                conectividade ou tire dúvidas sobre a configuração do NetMonitor.
              </p>

              <!-- Cards de Sugestões de Diagnóstico -->
              <v-row dense class="max-w-750 mx-auto text-left">
                <v-col v-for="cat in suggestionCategories" :key="cat.title" cols="12" sm="6" md="4">
                  <v-card
                    variant="outlined"
                    class="pa-3 rounded-lg fill-height hover-card cursor-pointer"
                    @click="handleQuickPrompt(cat.prompt)"
                  >
                    <div class="d-flex align-center ga-2 mb-1">
                      <v-icon size="18" :color="cat.color">{{ cat.icon }}</v-icon>
                      <span class="text-caption font-weight-bold">{{ cat.title }}</span>
                    </div>
                    <p class="text-caption text-medium-emphasis mb-0">
                      {{ cat.prompt }}
                    </p>
                  </v-card>
                </v-col>
              </v-row>
            </div>

            <!-- Lista de Mensagens -->
            <template v-else>
              <AiChatMessage v-for="msg in aiStore.messages" :key="msg.id" :message="msg" />
            </template>
          </div>

          <!-- Área Inferior do Chat (Barra de Instruções no TOPO + Textarea Alto + Ações) -->
          <div class="pa-3 border-t bg-surface">
            <!-- 1. TEXTO DE EXPLICAÇÃO E ATALHOS NA PARTE DE CIMA DO CAMPO -->
            <div
              class="d-flex align-center justify-space-between px-3 py-1 bg-surface-variant rounded-t-lg border-t border-s border-e"
            >
              <div class="d-flex align-center ga-1 text-caption text-medium-emphasis">
                <v-icon size="14" color="primary">mdi-keyboard-outline</v-icon>
                <span>
                  Pressione <kbd class="kbd-key">Enter</kbd> para enviar &bull;
                  <kbd class="kbd-key">Shift + Enter</kbd> para pular linha
                </span>
              </div>
              <div class="d-flex align-center ga-2">
                <span
                  v-if="aiStore.isStreaming"
                  class="text-caption text-primary font-weight-medium d-flex align-center ga-1"
                >
                  <v-progress-circular indeterminate size="12" width="2" color="primary" />
                  IA respondendo...
                </span>
                <span
                  v-else-if="inputContent.trim().length > 0"
                  class="text-caption text-medium-emphasis font-mono"
                >
                  {{ inputContent.trim().length }} caracteres
                </span>
              </div>
            </div>

            <!-- 2. CAMPO DE TEXTO MAIS ALTO PARA MELHOR USABILIDADE -->
            <v-textarea
              v-model="inputContent"
              placeholder="Digite sua dúvida ou instrução para a IA (ex: 'analisar latência para 1.1.1.1 e checar rota do gateway')..."
              variant="outlined"
              density="comfortable"
              :rows="3"
              :max-rows="8"
              auto-grow
              hide-details
              class="chat-input-textarea"
              @keydown.enter.prevent="handleEnter"
            />

            <!-- 3. BARRA DE AÇÕES INFERIOR DO CAMPO DE ENTRADA -->
            <div
              class="d-flex align-center justify-space-between px-3 py-2 bg-surface-variant rounded-b-lg border-b border-s border-e"
            >
              <div class="d-flex align-center ga-2">
                <v-tooltip location="top" text="Execução de ping, traceroute e scan de portas">
                  <template #activator="{ props: tipProps }">
                    <v-chip
                      v-bind="tipProps"
                      size="x-small"
                      color="primary"
                      variant="tonal"
                      prepend-icon="mdi-hammer-wrench"
                    >
                      Diagnósticos Ativos
                    </v-chip>
                  </template>
                </v-tooltip>
                <v-btn
                  v-if="inputContent.length > 0"
                  variant="text"
                  size="x-small"
                  color="grey"
                  prepend-icon="mdi-close"
                  @click="inputContent = ''"
                >
                  Limpar
                </v-btn>
              </div>

              <div class="d-flex align-center ga-2">
                <!-- Botão de Interromper Geração -->
                <v-btn
                  v-if="aiStore.isStreaming"
                  color="error"
                  variant="flat"
                  size="small"
                  prepend-icon="mdi-stop-circle-outline"
                  @click="aiStore.cancelGeneration()"
                >
                  Interromper
                </v-btn>

                <!-- Botão de Enviar Mensagem -->
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
        </v-card>
      </v-col>
    </v-row>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import { useAiStore } from '@/stores/ai'
import PageHeader from '@/components/PageHeader.vue'
import AiChatMessage from '@/components/ai/AiChatMessage.vue'
import { responseStyleOption } from '@/components/ai/aiResponseStyle'

const aiStore = useAiStore()
const inputContent = ref('')
const chatContainer = ref<HTMLElement | null>(null)

const activeDriverLabel = computed(() => {
  const driver = aiStore.settings?.activeDriver
  if (driver === 'opencode') return 'OpenCode Go / Zen'
  if (driver === 'openrouter') return 'OpenRouter Gateway'
  if (driver === 'ollama') return 'Ollama Local'
  return 'Provedor Desconhecido'
})

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

const quickPrompts = [
  'Testar conectividade e latência com a Internet',
  'Quais dispositivos estão offline ou com instabilidade?',
  'Resumir os alertas críticos das últimas horas',
  'Mostrar o gráfico de latência das últimas 24h do gateway',
  'Quais interfaces estão caídas ou saturadas?',
  'Como funciona o probe WireGuard e quando utilizá-lo?',
]

const suggestionCategories = [
  {
    title: 'Conectividade Internet',
    prompt: 'Executar diagnóstico de ping e latência para a Internet',
    icon: 'mdi-web',
    color: 'primary',
  },
  {
    title: 'Dispositivos Críticos',
    prompt: 'Quais equipamentos da rede estão offline ou oscilando agora?',
    icon: 'mdi-router-wireless-off',
    color: 'error',
  },
  {
    title: 'Alertas Recentes',
    prompt: 'Resumir os últimos alertas e apontar prováveis causas raiz',
    icon: 'mdi-bell-alert-outline',
    color: 'warning',
  },
  {
    title: 'Gráfico de Latência',
    prompt: 'Mostrar o gráfico de latência das últimas 24h do gateway',
    icon: 'mdi-chart-line',
    color: 'info',
  },
  {
    title: 'Interfaces com Problema',
    prompt: 'Quais interfaces estão caídas ou saturadas? Mostre o tráfego da mais carregada',
    icon: 'mdi-ethernet',
    color: 'success',
  },
  {
    title: 'WireGuard VPN',
    prompt: 'Como provisionar e monitorar roteadores remotos via túnel VPN?',
    icon: 'mdi-shield-lock-outline',
    color: 'deep-purple',
  },
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

function handleQuickPrompt(prompt: string) {
  inputContent.value = prompt
  handleSend()
}
</script>

<style scoped>
.ai-chat-page {
  height: calc(100vh - 110px);
}

.chat-scroll-area {
  min-height: 350px;
}

.max-w-500 {
  max-width: 500px;
}

.max-w-750 {
  max-width: 750px;
}

.kbd-key {
  display: inline-block;
  padding: 0.1rem 0.35rem;
  font-size: 0.72rem;
  font-family: monospace;
  background-color: rgba(var(--v-theme-on-surface), 0.08);
  border-radius: 4px;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.15);
}

.hover-card {
  transition:
    transform 0.2s ease,
    border-color 0.2s ease;
}

.hover-card:hover {
  transform: translateY(-2px);
  border-color: rgb(var(--v-theme-primary));
}

:deep(.chat-input-textarea .v-field) {
  border-radius: 0 !important;
  border-top: none !important;
  border-bottom: none !important;
}
</style>
