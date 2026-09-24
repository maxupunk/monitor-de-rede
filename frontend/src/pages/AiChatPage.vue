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
            <span class="text-body-small font-weight-bold text-uppercase text-medium-emphasis">
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
              <div class="text-title-small font-weight-bold text-truncate">
                {{ activeDriverLabel }}
              </div>
              <div class="text-body-small text-medium-emphasis font-mono text-truncate">
                {{ activeModelLabel }}
              </div>
            </div>
          </div>
        </v-card>

        <!-- Card: Conversas Salvas -->
        <v-card class="rounded-lg pa-3" variant="outlined">
          <div class="d-flex align-center ga-2 mb-2">
            <v-icon color="primary" size="18">mdi-history</v-icon>
            <span class="text-title-small font-weight-bold">Conversas</span>
          </div>
          <AiConversationList />
        </v-card>

        <!-- Card: Ações Rápidas -->
        <v-card class="rounded-lg pa-3" variant="outlined">
          <div class="d-flex align-center ga-2 mb-2">
            <v-icon color="primary" size="18">mdi-flash-outline</v-icon>
            <span class="text-title-small font-weight-bold">Ações Rápidas</span>
          </div>
          <div class="d-flex flex-column ga-2">
            <v-btn
              v-for="s in quickPrompts"
              :key="s"
              variant="tonal"
              color="primary"
              size="small"
              class="justify-start text-none text-body-small text-wrap py-2"
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
            <span class="text-title-small font-weight-bold">Ferramentas Autônomas</span>
          </div>
          <ul class="text-body-small text-medium-emphasis pl-4 d-flex flex-column ga-2">
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
              <span class="text-title-small font-weight-bold">Canal de Diagnóstico Interativo</span>
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

          <AiChatThread :placeholder="placeholder">
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
          </AiChatThread>
        </v-card>
      </v-col>
    </v-row>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { useDisplay } from 'vuetify'
import { useAiStore } from '@/stores/ai'
import { useAiModelInfo } from '@/composables/useAiModelInfo'
import PageHeader from '@/components/PageHeader.vue'
import AiChatThread from '@/components/ai/AiChatThread.vue'
import AiConversationList from '@/components/ai/AiConversationList.vue'
import { responseStyleOption } from '@/components/ai/aiResponseStyle'
import { AI_SUGGESTIONS } from '@/components/ai/aiSuggestions'

const aiStore = useAiStore()
const display = useDisplay()
const { driverLabel: activeDriverLabel, modelLabel: activeModelLabel } = useAiModelInfo()

const responseStyle = computed(() => responseStyleOption(aiStore.settings?.responseStyle))

const placeholder = computed(() =>
  display.xs.value
    ? 'Pergunte sobre a rede…'
    : "Digite sua dúvida ou instrução (ex: 'analisar latência para 1.1.1.1'). Use @ para marcar um dispositivo ou recurso…"
)

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

const quickPrompts = AI_SUGGESTIONS.map((suggestion) => suggestion.prompt)

onMounted(async () => {
  if (!aiStore.settings) {
    await aiStore.loadSettings()
  }
})

function handleQuickPrompt(prompt: string) {
  if (aiStore.isStreaming || !aiStore.settings?.enabled) return
  void aiStore.sendMessage(prompt)
}
</script>

<style scoped>
.ai-chat-page {
  height: calc(100dvh - 110px);
}
</style>
