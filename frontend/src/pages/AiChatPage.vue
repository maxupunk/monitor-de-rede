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
      <AiChatHeader title="Assistente IA" :subtitle="modelLabel">
        <template v-if="!wide" #leading>
          <v-btn
            icon="mdi-forum-outline"
            variant="text"
            color="primary"
            aria-label="Conversas salvas"
            title="Conversas salvas"
            @click="historyOpen = true"
          ></v-btn>
        </template>
        <template #actions>
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
              <div v-if="aiStore.settings?.allowActions" class="d-flex flex-wrap ga-1 mb-3">
                <v-chip
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
          <AiNewConversationButton v-if="!wide" variant="flat" size="small" />
        </template>
      </AiChatHeader>

      <AiChatThread ref="thread" :placeholder="placeholder" />
    </section>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useDisplay } from 'vuetify'
import { useAiStore } from '@/stores/ai'
import { useAiModelInfo } from '@/composables/useAiModelInfo'
import { useAiChatShell } from '@/composables/useAiChatShell'
import AiChatHeader from '@/components/ai/AiChatHeader.vue'
import AiChatThread from '@/components/ai/AiChatThread.vue'
import AiChatSidebar from '@/components/ai/AiChatSidebar.vue'
import AiNewConversationButton from '@/components/ai/AiNewConversationButton.vue'

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
const { placeholder } = useAiChatShell(thread, { focusOnMount: true })
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

.ai-page__capabilities {
  padding-left: 18px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}
</style>
