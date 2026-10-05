<template>
  <div class="ai-page">
    <!-- Conversas: coluna fixa no desktop, painel deslizante no celular -->
    <aside v-if="wide" class="ai-page__sidebar" aria-label="Conversas salvas">
      <AiChatSidebar />
    </aside>
    <v-navigation-drawer
      v-else
      v-model="historyOpen"
      temporary
      touchless
      location="left"
      :width="320"
      aria-label="Conversas salvas"
    >
      <AiChatSidebar @selected="historyOpen = false" />
    </v-navigation-drawer>

    <section class="ai-page__main">
      <header class="ai-page__toolbar">
        <v-btn
          v-if="!wide"
          icon="mdi-forum-outline"
          variant="text"
          color="primary"
          aria-label="Conversas salvas"
          title="Conversas salvas"
          @click="historyOpen = true"
        ></v-btn>
        <v-avatar v-else size="32" color="primary">
          <v-icon size="18" color="white">mdi-robot-outline</v-icon>
        </v-avatar>

        <div class="ai-page__title">
          <h1 class="text-title-medium font-weight-bold text-truncate">Assistente IA</h1>
          <div class="text-body-small font-mono text-truncate">{{ modelLabel }}</div>
        </div>

        <div class="ai-page__actions">
          <v-chip
            v-if="!aiStore.settings?.enabled && aiStore.settings"
            color="warning"
            variant="flat"
            size="small"
            prepend-icon="mdi-power-plug-off-outline"
            class="d-none d-sm-flex"
          >
            Desativado
          </v-chip>
          <v-chip
            size="small"
            color="primary"
            variant="tonal"
            :prepend-icon="responseStyle.icon"
            :title="responseStyle.hint"
            class="d-none d-md-flex"
          >
            {{ responseStyle.title }}
          </v-chip>

          <v-menu location="bottom end" :close-on-content-click="false" max-width="360">
            <template #activator="{ props: menuProps }">
              <v-btn
                v-bind="menuProps"
                icon="mdi-information-outline"
                variant="text"
                color="primary"
                aria-label="O que o assistente pode fazer"
                title="O que o assistente pode fazer"
              ></v-btn>
            </template>
            <v-card rounded="lg" class="pa-4">
              <div class="text-title-small font-weight-bold mb-2">O que o assistente faz</div>
              <div class="d-flex flex-wrap ga-1 mb-3">
                <v-chip
                  size="small"
                  :color="aiStore.settings?.allowActiveTools ? 'success' : 'warning'"
                  variant="tonal"
                  :prepend-icon="
                    aiStore.settings?.allowActiveTools ? 'mdi-wrench-check' : 'mdi-wrench-clock'
                  "
                >
                  {{
                    aiStore.settings?.allowActiveTools
                      ? 'Testes ativos liberados'
                      : 'Testes ativos desligados'
                  }}
                </v-chip>
                <v-chip
                  v-if="aiStore.settings?.allowActions"
                  size="small"
                  color="warning"
                  variant="tonal"
                  prepend-icon="mdi-hand-back-right-outline"
                >
                  Ações com confirmação
                </v-chip>
              </div>
              <ul class="ai-page__capabilities text-body-medium">
                <li v-for="capability in CAPABILITIES" :key="capability.title">
                  <strong>{{ capability.title }}:</strong> {{ capability.text }}
                </li>
              </ul>
            </v-card>
          </v-menu>

          <v-btn
            icon="mdi-cog-outline"
            variant="text"
            color="primary"
            to="/settings?tab=ia"
            aria-label="Configurações da IA"
            title="Configurações da IA"
          ></v-btn>
          <!-- No desktop o botão fica na coluna de conversas. -->
          <v-btn
            v-if="!wide"
            icon="mdi-plus"
            color="primary"
            variant="flat"
            size="small"
            aria-label="Nova conversa"
            title="Nova conversa"
            :disabled="aiStore.messages.length === 0 || aiStore.isStreaming"
            @click="aiStore.newConversation()"
          ></v-btn>
        </div>
      </header>

      <AutoAcceptBanner />
      <AiChatThread ref="thread" :placeholder="placeholder" />
    </section>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, ref } from 'vue'
import { useDisplay } from 'vuetify'
import { useAiStore } from '@/stores/ai'
import { useAiModelInfo } from '@/composables/useAiModelInfo'
import AiChatThread from '@/components/ai/AiChatThread.vue'
import AiChatSidebar from '@/components/ai/AiChatSidebar.vue'
import AutoAcceptBanner from '@/components/plugins/AutoAcceptBanner.vue'
import { responseStyleOption } from '@/components/ai/aiResponseStyle'

const CAPABILITIES = [
  { title: 'Dados do sistema', text: 'interfaces, monitores, histórico de uptime e métricas.' },
  { title: 'Logs e busca', text: 'panorama e grep (regex, contagem, contexto) em logs e alertas.' },
  { title: 'Causa raiz', text: 'correlação pela topologia e comparação com o normal.' },
  { title: 'Gráficos', text: 'latência, tráfego, CPU/memória e padrão por hora.' },
  { title: 'Testes ativos', text: 'ping, traceroute, portas, DNS e playbooks.' },
  { title: '@ e desfazer', text: 'marque um recurso; desfaça a pergunta que saiu errada.' },
  {
    title: 'Ações',
    text: 'silenciar alerta, janela de manutenção e monitor — só com confirmação.',
  },
]

const aiStore = useAiStore()
const display = useDisplay()
const { modelLabel } = useAiModelInfo()
const thread = ref<InstanceType<typeof AiChatThread> | null>(null)
const historyOpen = ref(false)

const wide = computed(() => display.mdAndUp.value)
const responseStyle = computed(() => responseStyleOption(aiStore.settings?.responseStyle))
const placeholder = computed(() =>
  display.xs.value
    ? 'Pergunte sobre a rede…'
    : 'Pergunte sobre a rede ou peça um diagnóstico — @ marca um equipamento'
)

onMounted(async () => {
  if (!aiStore.settings) await aiStore.loadSettings()
  // No celular, focar abriria o teclado por cima da conversa.
  if (!display.mobile.value) {
    await nextTick()
    thread.value?.focus()
  }
})
</script>

<style scoped>
/*
 * A página ocupa a área inteira do layout (`fullBleed`) e divide a altura:
 * só a lista de conversas e as mensagens rolam — nunca a página.
 */
.ai-page {
  display: flex;
  flex: 1 1 auto;
  height: 100%;
  min-height: 0;
  background: rgb(var(--v-theme-background));
}

.ai-page__sidebar {
  display: flex;
  flex-direction: column;
  flex: 0 0 300px;
  min-height: 0;
  border-right: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
  background: rgb(var(--v-theme-surface));
}

.ai-page__main {
  display: flex;
  flex-direction: column;
  flex: 1 1 auto;
  min-width: 0;
  min-height: 0;
  background: rgb(var(--v-theme-surface));
}

.ai-page__toolbar {
  display: flex;
  align-items: center;
  gap: 10px;
  flex: 0 0 auto;
  min-height: 56px;
  padding: 6px 8px 6px 12px;
  border-bottom: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
}

.ai-page__title {
  flex: 1 1 auto;
  min-width: 0;
  line-height: 1.25;
}

.ai-page__title h1 {
  font-size: inherit;
  margin: 0;
}

.ai-page__actions {
  display: flex;
  align-items: center;
  gap: 4px;
  flex: 0 0 auto;
}

.ai-page__capabilities {
  padding-left: 18px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}
</style>
