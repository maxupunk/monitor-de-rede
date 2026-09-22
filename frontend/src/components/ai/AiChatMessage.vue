<template>
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
      <!-- Tool Cards Executados -->
      <div v-if="toolCards.length > 0" class="mb-2">
        <AiToolCard
          v-for="tool in toolCards"
          :key="tool.id"
          :tool="tool"
          :message-id="message.id"
        />
      </div>

      <!-- Pergunta da IA antes de prosseguir -->
      <AiQuestionCard
        v-for="item in questions"
        :key="item.id"
        :question="item.question"
        :answerable="answerable"
      />

      <!-- Balão de Mensagem -->
      <v-card
        v-if="message.content || message.isStreaming || message.error"
        :color="message.role === 'user' ? 'primary' : 'surface'"
        :class="[
          'pa-3 rounded-xl elevation-1 message-bubble',
          message.role === 'user' ? 'text-white' : 'text-body-2',
        ]"
      >
        <!-- Erro -->
        <v-alert
          v-if="message.error"
          type="error"
          variant="tonal"
          density="compact"
          class="mb-2 text-caption"
        >
          {{ message.error }}
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
          <span
            v-if="usageLabel"
            class="text-caption text-medium-emphasis me-auto"
            title="Tokens enviados ao provedor (↑) e gerados na resposta (↓)"
          >
            {{ usageLabel }}
          </span>
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
      color="grey-darken-1"
      class="ml-2 elevation-1 flex-shrink-0 mt-1"
    >
      <v-icon size="18" color="white">mdi-account</v-icon>
    </v-avatar>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import DOMPurify from 'dompurify'
import { useAiStore, type AiDisplayMessage } from '@/stores/ai'
import AiToolCard from './AiToolCard.vue'
import AiQuestionCard from './AiQuestionCard.vue'
import { formatCompactCount } from '@/utils/formatters'
import { toolQuestion } from '@/utils/aiChatStream'
import { mentionKindMeta } from '@/utils/aiMentions'

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

/** Consumo da resposta, quando o provedor informou. */
const usageLabel = computed(() => {
  const usage = props.message.usage
  if (!usage) return null
  return `↑ ${formatCompactCount(usage.promptTokens)} · ↓ ${formatCompactCount(usage.completionTokens)} tokens`
})

/** Gráfico precisa da largura toda, mesmo quando o texto da resposta é curto. */
const hasChart = computed(() => props.message.toolCalls?.some((tool) => tool.chart) ?? false)

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;')
}

/**
 * Conversor leve de Markdown para HTML seguro utilizando DOMPurify.
 */
const renderedContent = computed(() => {
  if (!props.message.content) return ''

  let text = props.message.content

  // Bloco de código: ```linguagem\n...\n```
  text = text.replace(/```([a-zA-Z0-9_-]*)\n([\s\S]*?)```/g, (_match, _lang, code) => {
    return `<pre class="code-block bg-grey-lighten-4 pa-2 rounded my-2 font-mono text-caption overflow-x-auto"><code>${escapeHtml(code.trim())}</code></pre>`
  })

  // Código inline: `...`
  text = text.replace(/`([^`]+)`/g, (_match, code) => {
    return `<code class="bg-grey-lighten-3 px-1 rounded font-mono text-caption text-primary">${escapeHtml(code)}</code>`
  })

  // Negrito: **...**
  text = text.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')

  // Itálico: *...*
  text = text.replace(/\*([^*]+)\*/g, '<em>$1</em>')

  // Listas não ordenadas: - item ou * item
  text = text.replace(/^[*-]\s+(.+)$/gm, '<li class="ml-4">$1</li>')

  // Listas ordenadas: 1. item
  text = text.replace(/^\d+\.\s+(.+)$/gm, '<li class="ml-4 list-decimal">$1</li>')

  // Quebras de linha normais
  text = text.replace(/\n\n/g, '<br/><br/>').replace(/\n/g, '<br/>')

  return DOMPurify.sanitize(text, {
    ALLOWED_TAGS: ['strong', 'em', 'code', 'pre', 'li', 'ul', 'ol', 'br', 'p', 'span'],
    ALLOWED_ATTR: ['class'],
  })
})

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
  line-height: 1.5;
}
.code-block {
  border: 1px solid rgba(0, 0, 0, 0.08);
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
