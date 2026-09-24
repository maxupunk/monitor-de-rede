<template>
  <div class="ai-thread">
    <div class="ai-thread__viewport">
      <div
        ref="scroller"
        class="ai-thread__messages"
        :class="compact ? 'pa-3' : 'pa-4'"
        @scroll.passive="onScroll"
      >
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
            @click="emit('navigate')"
          >
            Configurações</router-link
          >.
        </v-alert>

        <!-- Conversa vazia: o que dá para pedir -->
        <div v-if="aiStore.messages.length === 0" class="ai-thread__empty">
          <v-avatar :size="compact ? 52 : 64" color="primary" variant="tonal" class="mb-3">
            <v-icon :size="compact ? 28 : 34">mdi-creation-outline</v-icon>
          </v-avatar>
          <div
            :class="compact ? 'text-title-medium' : 'text-title-large'"
            class="font-weight-bold mb-1"
          >
            Como posso ajudar?
          </div>
          <p class="text-body-medium text-medium-emphasis mb-4 ai-thread__intro">
            Pergunte sobre a rede, peça um diagnóstico ou tire dúvidas sobre o NetMonitor. Use
            <strong>@</strong> para marcar um equipamento.
          </p>

          <div v-if="compact" class="d-flex flex-column ga-2">
            <v-btn
              v-for="suggestion in visibleSuggestions"
              :key="suggestion.prompt"
              variant="tonal"
              color="primary"
              class="ai-thread__suggestion text-none"
              :prepend-icon="suggestion.icon"
              :disabled="!canSend"
              @click="send(suggestion.prompt)"
            >
              {{ suggestion.prompt }}
            </v-btn>
          </div>
          <v-row v-else dense class="ai-thread__cards text-left">
            <v-col
              v-for="suggestion in visibleSuggestions"
              :key="suggestion.prompt"
              cols="12"
              sm="6"
            >
              <v-card
                variant="outlined"
                class="pa-3 rounded-lg fill-height ai-thread__card"
                :disabled="!canSend"
                @click="send(suggestion.prompt)"
              >
                <div class="d-flex align-center ga-2 mb-1">
                  <v-icon size="18" :color="suggestion.color">{{ suggestion.icon }}</v-icon>
                  <span class="text-body-small font-weight-bold">{{ suggestion.title }}</span>
                </div>
                <p class="text-body-small text-medium-emphasis mb-0">{{ suggestion.prompt }}</p>
              </v-card>
            </v-col>
          </v-row>
        </div>

        <template v-else>
          <AiChatMessage
            v-for="message in aiStore.messages"
            :key="message.id"
            :message="message"
            @rewind="handleRewind"
          />
        </template>
      </div>

      <v-fade-transition>
        <v-btn
          v-if="showJump"
          class="ai-thread__jump"
          icon="mdi-arrow-down"
          size="small"
          color="primary"
          elevation="6"
          aria-label="Ir para a mensagem mais recente"
          title="Ir para a mensagem mais recente"
          @click="scrollToBottom(true)"
        />
      </v-fade-transition>
    </div>

    <div class="ai-thread__composer" :class="compact ? 'pa-2' : 'pa-3'">
      <AiChatComposer
        ref="composer"
        :compact="compact"
        :max-rows="compact ? 6 : 8"
        :placeholder="placeholder"
      >
        <template #status>
          <slot name="status" />
        </template>
      </AiChatComposer>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useAiStore } from '@/stores/ai'
import { useChatAutoScroll } from '@/composables/useChatAutoScroll'
import AiChatMessage from './AiChatMessage.vue'
import AiChatComposer from './AiChatComposer.vue'
import { AI_SUGGESTIONS } from './aiSuggestions'

/**
 * A conversa com a IA: mensagens, estado vazio com sugestões, rolagem que
 * respeita quem está lendo e o campo da pergunta. O painel lateral e a tela
 * cheia usam este mesmo componente — só a moldura muda.
 */
const props = withDefaults(
  defineProps<{
    /** Painel lateral: menos espaço, menos sugestões, dicas curtas. */
    compact?: boolean
    placeholder: string
  }>(),
  { compact: false }
)

const emit = defineEmits<{
  /** Um link levou para outra tela: quem é sobreposição pode se fechar. */
  navigate: []
}>()

const aiStore = useAiStore()
const scroller = ref<HTMLElement | null>(null)
const composer = ref<InstanceType<typeof AiChatComposer> | null>(null)

const visibleSuggestions = computed(() =>
  props.compact ? AI_SUGGESTIONS.slice(0, 4) : AI_SUGGESTIONS
)
const canSend = computed(() => !aiStore.isStreaming && aiStore.settings?.enabled === true)

const { onScroll, scrollToBottom, showJump } = useChatAutoScroll(
  scroller,
  () => aiStore.messages.length,
  () => {
    const last = aiStore.messages[aiStore.messages.length - 1]
    return [last?.content, last?.toolCalls?.length, last?.isStreaming]
  }
)

function send(prompt: string) {
  if (!canSend.value) return
  void aiStore.sendMessage(prompt)
}

function handleRewind(messageId: string) {
  const draft = aiStore.rewind(messageId)
  if (draft) composer.value?.setDraft(draft)
}

defineExpose({
  focus: () => composer.value?.focus(),
  scrollToBottom,
})
</script>

<style scoped>
/*
 * Mensagens e campo dividem a altura de quem hospeda: só as mensagens rolam.
 * Nada de `100vh` — no celular ele inclui a barra do navegador e empurrava o
 * campo para fora da tela.
 */
.ai-thread {
  position: relative;
  display: flex;
  flex-direction: column;
  flex: 1 1 auto;
  min-height: 0;
}

.ai-thread__viewport {
  position: relative;
  display: flex;
  flex-direction: column;
  flex: 1 1 auto;
  min-height: 0;
}

.ai-thread__messages {
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
  overscroll-behavior: contain;
}

.ai-thread__composer {
  flex: 0 0 auto;
  border-top: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
  background: rgb(var(--v-theme-surface));
}

.ai-thread__empty {
  text-align: center;
  padding: 24px 4px;
}

.ai-thread__intro {
  max-width: 520px;
  margin-inline: auto;
}

.ai-thread__cards {
  max-width: 760px;
  margin-inline: auto;
}

/* O texto da sugestão quebra linha em vez de ser cortado. */
.ai-thread__suggestion {
  justify-content: flex-start;
  height: auto !important;
  min-height: 40px;
  padding-block: 8px;
  white-space: normal;
  text-align: left;
  letter-spacing: normal;
}

.ai-thread__suggestion :deep(.v-btn__content) {
  white-space: normal;
  text-align: left;
}

.ai-thread__card {
  transition:
    transform 0.2s ease,
    border-color 0.2s ease;
}

.ai-thread__card:hover {
  transform: translateY(-2px);
  border-color: rgb(var(--v-theme-primary));
}

.ai-thread__jump {
  position: absolute;
  right: 16px;
  bottom: 12px;
  z-index: 2;
}
</style>
