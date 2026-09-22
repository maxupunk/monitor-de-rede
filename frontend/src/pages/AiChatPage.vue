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
          prepend-icon="mdi-plus"
          :disabled="aiStore.messages.length === 0 || aiStore.isStreaming"
          @click="aiStore.newConversation()"
        >
          Nova conversa
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

        <!-- Card: Conversas Salvas -->
        <v-card class="rounded-lg pa-3" variant="outlined">
          <div class="d-flex align-center ga-2 mb-2">
            <v-icon color="primary" size="18">mdi-history</v-icon>
            <span class="text-subtitle-2 font-weight-bold">Conversas</span>
          </div>
          <AiConversationList />
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
            <li v-for="capability in capabilities" :key="capability.title">
              <strong>{{ capability.title }}:</strong> {{ capability.text }}
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
              <v-menu location="bottom start" :close-on-content-click="false">
                <template #activator="{ props: menuProps }">
                  <v-btn
                    v-bind="menuProps"
                    icon="mdi-history"
                    size="small"
                    variant="text"
                    color="primary"
                    class="d-md-none"
                    title="Conversas salvas"
                  />
                </template>
                <v-card class="pa-3" min-width="280" max-width="340">
                  <AiConversationList />
                </v-card>
              </v-menu>
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
              <v-chip
                v-if="aiStore.settings?.allowActions"
                size="x-small"
                color="warning"
                variant="tonal"
                prepend-icon="mdi-hand-back-right-outline"
                title="A IA pode propor ações; cada uma espera a sua confirmação"
              >
                Ações com confirmação
              </v-chip>
              <v-btn
                v-if="aiStore.messages.length > 0"
                variant="text"
                color="primary"
                size="x-small"
                prepend-icon="mdi-plus"
                :disabled="aiStore.isStreaming"
                @click="aiStore.newConversation()"
              >
                Nova
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
              <AiChatMessage
                v-for="msg in aiStore.messages"
                :key="msg.id"
                :message="msg"
                @rewind="handleRewind"
              />
            </template>
          </div>

          <!-- Campo da pergunta, com @ para marcar recursos -->
          <div class="pa-3 border-t bg-surface">
            <AiChatComposer
              ref="composer"
              placeholder="Digite sua dúvida ou instrução (ex: 'analisar latência para 1.1.1.1'). Use @ para marcar um dispositivo ou recurso..."
            >
              <template #status>
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
              </template>
            </AiChatComposer>
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
import AiChatComposer from '@/components/ai/AiChatComposer.vue'
import AiConversationList from '@/components/ai/AiConversationList.vue'
import { responseStyleOption } from '@/components/ai/aiResponseStyle'

const aiStore = useAiStore()
const chatContainer = ref<HTMLElement | null>(null)
const composer = ref<InstanceType<typeof AiChatComposer> | null>(null)

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

const capabilities = [
  { title: 'Dados do sistema', text: 'interfaces, monitores, histórico de uptime e métricas.' },
  {
    title: 'Logs e grep',
    text: 'panorama por padrão e busca (regex, contagem, contexto) em logs, alertas e checagens.',
  },
  { title: 'Causa raiz', text: 'correlação pela topologia e comparação com o normal.' },
  {
    title: '@ e desfazer',
    text: 'marque dispositivo, monitor, container ou fonte; desfaça a pergunta que saiu errada.',
  },
  { title: 'Gráficos', text: 'latência, tráfego, CPU/memória e padrão por hora.' },
  { title: 'Testes ativos', text: 'ping, traceroute, portas, DNS e playbooks.' },
  {
    title: 'Ações',
    text: 'silenciar alerta, janela de manutenção e monitor — só com confirmação.',
  },
]

const quickPrompts = [
  'Testar conectividade e latência com a Internet',
  'Quais dispositivos estão offline ou com instabilidade?',
  'Resumir os alertas críticos das últimas horas',
  'Mostrar o gráfico de latência das últimas 24h do gateway',
  'Quais interfaces estão caídas ou saturadas?',
  'Quais erros se repetem nos logs das últimas 24h?',
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

function handleQuickPrompt(prompt: string) {
  if (aiStore.isStreaming || !aiStore.settings?.enabled) return
  void aiStore.sendMessage(prompt)
}

function handleRewind(messageId: string) {
  const draft = aiStore.rewind(messageId)
  if (draft) composer.value?.setDraft(draft)
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

.hover-card {
  transition:
    transform 0.2s ease,
    border-color 0.2s ease;
}

.hover-card:hover {
  transform: translateY(-2px);
  border-color: rgb(var(--v-theme-primary));
}
</style>
