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
      ]"
    >
      <!-- Tool Cards Executados -->
      <div v-if="message.toolCalls && message.toolCalls.length > 0" class="mb-2">
        <AiToolCard v-for="tool in message.toolCalls" :key="tool.id" :tool="tool" />
      </div>

      <!-- Balão de Mensagem -->
      <v-card
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

        <!-- Cursor de Streaming -->
        <span v-if="message.isStreaming" class="streaming-cursor" />

        <!-- Rodapé do Balão -->
        <div v-if="message.content && !message.isStreaming" class="d-flex justify-end mt-1">
          <v-btn
            icon
            size="x-small"
            variant="text"
            :color="message.role === 'user' ? 'white' : 'grey'"
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
import type { AiDisplayMessage } from '@/stores/ai'
import AiToolCard from './AiToolCard.vue'

const props = defineProps<{
  message: AiDisplayMessage
}>()

const copied = ref(false)

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
