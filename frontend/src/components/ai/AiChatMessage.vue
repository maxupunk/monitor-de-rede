<template>
  <div>
    <div :class="['d-flex mb-3', message.role === 'user' ? 'justify-end' : 'justify-start']">
      <!-- Avatar do Assistente -->
      <v-avatar
        v-if="message.role === 'assistant'"
        size="32"
        color="primary"
        class="mr-2 elevation-1 flex-shrink-0 mt-1"
      >
        <v-icon size="18" color="white">mdi-robot-outline</v-icon>
      </v-avatar>

      <div
        :class="[
          'message-bubble-wrapper',
          message.role === 'user' ? 'user-wrapper' : 'assistant-wrapper',
          { 'has-chart': hasChart },
        ]"
      >
        <!-- Ferramentas executadas: consultas concluídas viram uma linha só -->
        <AiToolActivity
          v-if="toolCards.length > 0"
          :tools="toolCards"
          :message-id="message.id"
          class="mb-2"
        />

        <!-- Pergunta da IA antes de prosseguir -->
        <AiQuestionCard
          v-for="item in questions"
          :key="item.id"
          :question="item.question"
          :answerable="answerable"
        />

        <!-- Balão de Mensagem -->
        <v-card
          v-if="message.content || message.isStreaming || message.error || message.notice"
          :color="message.role === 'user' ? 'primary' : 'surface'"
          :class="[
            'pa-3 rounded-xl elevation-1 message-bubble',
            message.role === 'user' ? 'text-white' : 'text-body-medium',
          ]"
        >
          <!-- Erro -->
          <v-alert
            v-if="message.error"
            type="error"
            variant="tonal"
            density="compact"
            class="mb-2 text-body-small"
          >
            {{ message.error }}
          </v-alert>

          <!-- Aviso (ex: janela de contexto pequena) -->
          <v-alert
            v-if="message.notice"
            type="warning"
            variant="tonal"
            density="compact"
            class="mb-2 text-body-small"
          >
            <div>{{ message.notice }}</div>
            <div class="mt-2 d-flex justify-end">
              <v-btn
                size="x-small"
                variant="outlined"
                color="warning"
                prepend-icon="mdi-cog-outline"
                to="/settings?tab=ia"
              >
                Ajustar Configurações de IA
              </v-btn>
            </div>
          </v-alert>

          <!-- Conteúdo Renderizado -->
          <div v-if="message.content" class="markdown-body" v-html="renderedContent" />

          <!-- Marcados com @ -->
          <div v-if="message.mentions?.length" class="d-flex flex-wrap ga-1 mt-2">
            <v-chip
              v-for="mention in message.mentions"
              :key="mention.kind + mention.id"
              size="x-small"
              variant="outlined"
              color="white"
              :prepend-icon="mentionKindMeta(mention.kind).icon"
            >
              {{ mention.label }}
            </v-chip>
          </div>

          <!-- Cursor de Streaming -->
          <span v-if="message.isStreaming" class="streaming-cursor" />

          <!-- Rodapé do Balão -->
          <div
            v-if="message.content && !message.isStreaming"
            class="d-flex align-center justify-end ga-2 mt-1"
          >
            <div
              v-if="metrics && (metrics.model || metrics.details.length > 0)"
              class="usage-metrics d-flex align-center ga-1 me-auto text-body-small"
            >
              <span v-if="metrics.model" class="usage-model d-inline-flex align-center ga-1">
                <v-icon size="12">mdi-chip</v-icon>
                <span class="text-truncate">{{ metrics.model }}</span>
              </span>
              <v-tooltip v-if="metrics.details.length > 0" location="top" max-width="360">
                <template #activator="{ props: tooltipProps }">
                  <v-btn
                    v-bind="tooltipProps"
                    icon
                    size="x-small"
                    variant="text"
                    color="primary"
                    aria-label="Consumo da resposta"
                  >
                    <v-icon size="14">mdi-information-outline</v-icon>
                  </v-btn>
                </template>
                <div v-for="line in metrics.details" :key="line">{{ line }}</div>
              </v-tooltip>
            </div>
            <v-btn
              v-if="message.role === 'user'"
              icon
              size="x-small"
              variant="text"
              color="white"
              title="Desfazer: volta a conversa para antes desta pergunta e devolve o texto ao campo. Ações já executadas não são revertidas."
              @click="emit('rewind', message.id)"
            >
              <v-icon size="14">mdi-undo-variant</v-icon>
            </v-btn>
            <v-btn
              icon
              size="x-small"
              variant="text"
              :color="message.role === 'user' ? 'white' : 'primary'"
              title="Copiar"
              @click="copyContent"
            >
              <v-icon size="14">{{ copied ? 'mdi-check' : 'mdi-content-copy' }}</v-icon>
            </v-btn>
          </div>
        </v-card>
      </div>

      <!-- Avatar do Usuário -->
      <v-avatar
        v-if="message.role === 'user'"
        size="32"
        color="secondary"
        class="ml-2 elevation-1 flex-shrink-0 mt-1 d-none d-sm-flex"
      >
        <v-icon size="18" color="white">mdi-account</v-icon>
      </v-avatar>
    </div>

    <!-- Compactação: o que está acima virou resumo na memória da IA -->
    <div v-if="message.compaction" class="compaction mb-3">
      <div class="compaction__line d-flex align-center ga-2 text-body-small">
        <v-icon size="16" color="primary">mdi-archive-arrow-down-outline</v-icon>
        <span>
          {{
            message.compaction.summarized
              ? 'Contexto compactado — as mensagens acima foram resumidas para a IA'
              : 'Contexto reduzido — mensagens antigas saíram da memória da IA sem resumo'
          }}
          <span class="font-weight-medium">
            ({{ formatCompactCount(message.compaction.tokensBefore) }} →
            {{ formatCompactCount(message.compaction.tokensAfter) }} tokens)
          </span>
        </span>
        <v-btn
          variant="text"
          size="x-small"
          color="primary"
          :append-icon="showSummary ? 'mdi-chevron-up' : 'mdi-chevron-down'"
          @click="showSummary = !showSummary"
        >
          Resumo
        </v-btn>
      </div>
      <div v-if="showSummary" class="compaction__summary text-body-medium">
        {{ message.compaction.summary }}
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useAiStore, type AiDisplayMessage } from '@/stores/ai'
import AiToolActivity from './AiToolActivity.vue'
import AiQuestionCard from './AiQuestionCard.vue'
import { summarizeUsage } from './aiUsageSummary'
import { formatCompactCount } from '@/utils/formatters'
import { toolQuestion } from '@/utils/aiChatStream'
import { mentionKindMeta } from '@/utils/aiMentions'
import { renderMarkdown } from '@/utils/markdown'

const props = defineProps<{
  message: AiDisplayMessage
}>()

const emit = defineEmits<{
  /** "Desfazer" da pergunta: quem mostra o campo recebe o texto de volta. */
  rewind: [messageId: string]
}>()

const aiStore = useAiStore()

/** Perguntas da IA (`ask_user`) viram cartão com opções; o resto, cartão de ferramenta. */
const questions = computed(() =>
  (props.message.toolCalls ?? []).flatMap((tool) => {
    const question = toolQuestion(tool)
    return question ? [{ id: tool.id, question }] : []
  })
)

const toolCards = computed(() =>
  (props.message.toolCalls ?? []).filter((tool) => !toolQuestion(tool))
)

/** Só a última mensagem, com a IA parada, aceita resposta pelos botões. */
const answerable = computed(
  () =>
    !aiStore.isStreaming && aiStore.messages[aiStore.messages.length - 1]?.id === props.message.id
)

const copied = ref(false)
const showSummary = ref(false)

/** Modelo à vista; consumo, velocidade e contexto no detalhe (ⓘ). */
const metrics = computed(() => summarizeUsage(props.message.usage))

/** Gráfico precisa da largura toda, mesmo quando o texto da resposta é curto. */
const hasChart = computed(() => props.message.toolCalls?.some((tool) => tool.chart) ?? false)

/** Markdown leve e seguro (ver `utils/markdown`). */
const renderedContent = computed(() => renderMarkdown(props.message.content))

async function copyContent() {
  if (!props.message.content) return
  try {
    await navigator.clipboard.writeText(props.message.content)
    copied.value = true
    setTimeout(() => {
      copied.value = false
    }, 2000)
  } catch (err) {
    console.error('Falha ao copiar:', err)
  }
}
</script>

<style scoped>
.message-bubble-wrapper {
  max-width: 85%;
  min-width: 0;
}
.message-bubble-wrapper.has-chart {
  width: 85%;
}
.user-wrapper {
  align-items: flex-end;
}
.assistant-wrapper {
  align-items: flex-start;
}
.message-bubble {
  word-break: break-word;
  overflow-wrap: anywhere;
  line-height: 1.5;
}
/* Código, tabela e imagem rolam dentro do balão — nunca alargam a conversa. */
.message-bubble :deep(pre),
.message-bubble :deep(table) {
  display: block;
  max-width: 100%;
  overflow-x: auto;
}
/* No balão do usuário (fundo primary) link e borda seguem o texto branco. */
.message-bubble.text-white :deep(a),
.message-bubble.text-white :deep(.md-table th),
.message-bubble.text-white :deep(.md-table td) {
  color: inherit;
  border-color: rgba(255, 255, 255, 0.4);
}
.message-bubble :deep(img) {
  max-width: 100%;
  height: auto;
}
/* Cinza fixo (`bg-grey-lighten-*`) deixava o texto claro do tema escuro
   ilegível: fundo e texto acompanham o tema. Vale também para os blocos de
   parâmetros e resultado dos cartões de ferramenta. */
.message-bubble-wrapper :deep(.code-block) {
  background: rgba(var(--v-theme-on-surface), 0.08);
  border: 1px solid rgba(var(--v-theme-on-surface), 0.12);
  color: rgb(var(--v-theme-on-surface));
}
.message-bubble :deep(.inline-code) {
  background: rgba(var(--v-theme-on-surface), 0.1);
  color: inherit;
}
.compaction__line {
  color: rgb(var(--v-theme-on-surface));
  padding: 4px 12px;
  border-top: 1px dashed rgba(var(--v-theme-primary), 0.5);
  border-bottom: 1px dashed rgba(var(--v-theme-primary), 0.5);
  background: rgba(var(--v-theme-primary), 0.06);
  flex-wrap: wrap;
}
.compaction__summary {
  white-space: pre-wrap;
  margin: 8px 12px 0;
  padding: 8px 12px;
  border-left: 3px solid rgb(var(--v-theme-primary));
  background: rgba(var(--v-theme-on-surface), 0.04);
  color: rgb(var(--v-theme-on-surface));
}
.usage-metrics {
  color: rgb(var(--v-theme-on-surface));
  line-height: 1.4;
  min-width: 0;
}
.usage-model {
  min-width: 0;
  max-width: 220px;
  padding: 0 6px;
  border-radius: 6px;
  background: rgba(var(--v-theme-primary), 0.14);
  color: rgb(var(--v-theme-on-surface));
  font-weight: 500;
}
.streaming-cursor {
  display: inline-block;
  width: 7px;
  height: 15px;
  background-color: currentColor;
  margin-left: 3px;
  vertical-align: middle;
  animation: blink 0.8s infinite;
}
@keyframes blink {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0;
  }
}
</style>
