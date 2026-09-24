<template>
  <div class="ai-composer">
    <!-- Dicas de teclado no topo do campo -->
    <div
      class="composer-bar d-flex align-center justify-space-between ga-2 px-3 py-1 rounded-t-lg border-t border-s border-e"
    >
      <div v-if="touch" class="text-body-small text-medium-emphasis">
        <kbd class="kbd-key">@</kbd> marca um recurso
      </div>
      <div v-else class="d-flex align-center ga-1 text-body-small text-medium-emphasis">
        <v-icon size="14" color="primary">mdi-keyboard-outline</v-icon>
        <span v-if="compact">
          <kbd class="kbd-key">Enter</kbd> envia &bull; <kbd class="kbd-key">@</kbd> marca
        </span>
        <span v-else>
          <kbd class="kbd-key">Enter</kbd> envia &bull;
          <kbd class="kbd-key">Shift + Enter</kbd> pula linha &bull;
          <kbd class="kbd-key">@</kbd> marca dispositivo, monitor, container ou fonte
        </span>
      </div>
      <div class="d-flex align-center ga-3 flex-shrink-0">
        <span
          v-if="aiStore.isStreaming"
          class="text-body-small text-primary font-weight-medium d-flex align-center ga-1"
        >
          <v-progress-circular indeterminate size="12" width="2" color="primary" />
          {{ compact ? 'Respondendo' : 'IA respondendo...' }}
        </span>
        <v-chip
          v-if="aiStore.compactNext"
          size="x-small"
          color="primary"
          variant="tonal"
          prepend-icon="mdi-archive-arrow-down-outline"
          closable
          @click:close="aiStore.requestCompaction(false)"
        >
          {{ compact ? 'Compactar' : 'Compactar ao enviar' }}
        </v-chip>
        <v-menu v-if="context" location="top end" :close-on-content-click="false" max-width="320">
          <template #activator="{ props: menuProps }">
            <button
              v-bind="menuProps"
              type="button"
              class="context-meter d-flex align-center ga-1 text-body-small"
              :aria-label="context.detail"
            >
              <v-progress-circular
                :model-value="context.percent"
                :color="context.color"
                bg-color="on-surface"
                size="14"
                width="3"
              />
              <span>{{ context.label }}</span>
            </button>
          </template>
          <v-card class="pa-3" rounded="lg">
            <div class="text-title-small font-weight-bold mb-1">Janela de contexto</div>
            <v-progress-linear
              :model-value="context.percent"
              :color="context.color"
              height="6"
              rounded
              class="mb-2"
            />
            <div class="text-body-medium">
              {{ context.amount }} tokens ({{ context.percentLabel }})
            </div>
            <div class="text-body-small text-medium-emphasis mb-2">
              {{
                context.reported
                  ? 'Janela informada pelo provedor do modelo.'
                  : 'Janela estimada pelo nome do modelo — o provedor não informou.'
              }}
            </div>
            <div class="text-body-small mb-3">
              A partir de {{ autoCompactLabel }} a conversa é compactada sozinha: as mensagens
              antigas viram um resumo e a janela volta para cerca de metade.
            </div>
            <v-btn
              block
              size="small"
              color="primary"
              :variant="aiStore.compactNext ? 'tonal' : 'flat'"
              prepend-icon="mdi-archive-arrow-down-outline"
              :disabled="aiStore.messages.length < 3"
              @click="aiStore.requestCompaction(!aiStore.compactNext)"
            >
              {{ aiStore.compactNext ? 'Cancelar compactação' : 'Compactar na próxima pergunta' }}
            </v-btn>
          </v-card>
        </v-menu>
      </div>
    </div>

    <div class="composer-field">
      <!-- Lista do @ -->
      <v-card v-if="picker.isOpen.value" class="mention-menu" elevation="8" rounded="lg">
        <div class="px-3 pt-2 pb-1 text-body-small font-weight-bold text-medium-emphasis">
          Marcar com @
        </div>
        <div v-if="picker.options.value.length === 0" class="px-3 pb-3 text-body-small">
          <v-progress-circular indeterminate size="12" width="2" color="primary" class="me-1" />
          Buscando...
        </div>
        <v-list v-else density="compact" class="py-0 mention-list" max-height="260">
          <v-list-item
            v-for="(option, index) in picker.options.value"
            :key="option.kind + option.id"
            :title="option.label"
            :subtitle="optionSubtitle(option)"
            :prepend-icon="mentionKindMeta(option.kind).icon"
            :active="index === picker.activeIndex.value"
            color="primary"
            @mousedown.prevent="select(option)"
          />
        </v-list>
      </v-card>

      <!-- Marcações da pergunta -->
      <div v-if="mentions.length > 0" class="d-flex flex-wrap ga-1 px-3 pt-2 border-s border-e">
        <v-chip
          v-for="mention in mentions"
          :key="mention.kind + mention.id"
          size="x-small"
          variant="tonal"
          :color="mentionKindMeta(mention.kind).color"
          :prepend-icon="mentionKindMeta(mention.kind).icon"
          closable
          @click:close="removeMention(mention)"
        >
          {{ mention.label }}
        </v-chip>
      </div>

      <v-textarea
        ref="field"
        v-model="content"
        :placeholder="placeholder"
        variant="outlined"
        density="comfortable"
        :rows="touch ? 2 : 3"
        :max-rows="maxRows"
        auto-grow
        hide-details
        class="composer-textarea"
        @keydown="onKeydown"
        @input="syncPicker"
        @click="syncPicker"
        @keyup="onKeyup"
        @blur="picker.close()"
      />
    </div>

    <!-- Barra de ações -->
    <div
      class="composer-bar d-flex align-center justify-space-between px-3 py-2 rounded-b-lg border-b border-s border-e"
    >
      <div class="d-flex align-center ga-1 flex-wrap">
        <slot name="status" />
        <v-btn
          variant="text"
          size="x-small"
          color="primary"
          prepend-icon="mdi-at"
          title="Marcar dispositivo, monitor, container ou fonte de dados"
          @click="startMention"
        >
          Marcar
        </v-btn>
        <v-btn
          v-if="content.length > 0"
          variant="text"
          size="x-small"
          color="error"
          prepend-icon="mdi-close"
          @click="clear"
        >
          Limpar
        </v-btn>
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
          {{ compact ? 'Parar' : 'Interromper' }}
        </v-btn>
        <v-btn
          v-else
          color="primary"
          variant="flat"
          size="small"
          prepend-icon="mdi-send"
          :disabled="!content.trim() || !aiStore.settings?.enabled"
          @click="send"
        >
          Enviar
        </v-btn>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { useAiStore, type AiDraft, type AiMention } from '@/stores/ai'
import { useMentionPicker } from '@/composables/useMentionPicker'
import { addMention, insertMention, mentionKindMeta, mentionsInText } from '@/utils/aiMentions'
import { AUTO_COMPACT_RATIO, contextUsage } from '@/utils/aiChatStream'
import { formatCompactCount, formatPercent } from '@/utils/formatters'

/** Perto da compactação automática: vale avisar. */
const CONTEXT_WARNING_RATIO = 0.7
const autoCompactLabel = formatPercent(AUTO_COMPACT_RATIO * 100, 0)

const props = withDefaults(
  defineProps<{
    placeholder: string
    maxRows?: number
    /** Painel lateral: dicas e rótulos mais curtos. */
    compact?: boolean
  }>(),
  { maxRows: 8, compact: false }
)

const aiStore = useAiStore()

/** Tela de toque: teclado virtual, sem atalhos de teclado físico. */
const touch =
  typeof window !== 'undefined' && window.matchMedia?.('(pointer: coarse)').matches === true

/** Quanto da janela do modelo a conversa já ocupa (medida da última resposta). */
const context = computed(() => {
  const usage = contextUsage(aiStore.messages)
  if (!usage) return null
  const percent = usage.ratio * 100
  const color =
    usage.ratio >= AUTO_COMPACT_RATIO
      ? 'error'
      : usage.ratio >= CONTEXT_WARNING_RATIO
        ? 'warning'
        : 'primary'
  const amount = `${formatCompactCount(usage.used)} / ${formatCompactCount(usage.window)}`
  const percentLabel = formatPercent(percent, 0)
  const estimated = usage.reported ? '' : ' (janela estimada)'
  return {
    percent,
    percentLabel,
    color,
    amount,
    reported: usage.reported,
    label: props.compact ? percentLabel : `${amount} tokens${usage.reported ? '' : ' ~'}`,
    detail: `Janela de contexto: ${amount} tokens, ${percentLabel}${estimated}. Clique para detalhes e para compactar.`,
  }
})

const picker = useMentionPicker((query) => aiStore.searchMentions(query))

const content = ref('')
const mentions = ref<AiMention[]>([])
const field = ref<{ $el: HTMLElement } | null>(null)

function textarea(): HTMLTextAreaElement | null {
  return field.value?.$el.querySelector('textarea') ?? null
}

function caret(): number {
  return textarea()?.selectionStart ?? content.value.length
}

function placeCaret(position: number) {
  void nextTick(() => {
    const element = textarea()
    if (!element) return
    element.focus()
    element.setSelectionRange(position, position)
  })
}

function syncPicker() {
  picker.update(textarea()?.value ?? content.value, caret())
}

// Apagar o `@Rótulo` do texto desmarca o recurso.
watch(content, (text) => {
  const kept = mentionsInText(text, mentions.value)
  if (kept.length !== mentions.value.length) mentions.value = kept
})

function optionSubtitle(option: AiMention): string {
  const kind = mentionKindMeta(option.kind).label
  return option.detail ? `${kind} · ${option.detail}` : kind
}

function select(option: AiMention) {
  const query = picker.query.value
  if (!query) return
  const next = insertMention(content.value, query, caret(), option.label)
  mentions.value = addMention(mentions.value, option)
  content.value = next.text
  picker.close()
  placeCaret(next.caret)
}

function removeMention(mention: AiMention) {
  mentions.value = mentions.value.filter(
    (item) => item.kind !== mention.kind || item.id !== mention.id
  )
  content.value = content.value.replace(`@${mention.label} `, '').replace(`@${mention.label}`, '')
}

/** Botão "Marcar": põe um `@` no cursor e abre a lista. */
function startMention() {
  const position = caret()
  const before = content.value.slice(0, position)
  const token = before.length === 0 || /\s$/.test(before) ? '@' : ' @'
  content.value = before + token + content.value.slice(position)
  const next = position + token.length
  placeCaret(next)
  picker.update(content.value, next)
}

function send() {
  const text = content.value.trim()
  if (!text || aiStore.isStreaming || !aiStore.settings?.enabled) return
  const marked = mentionsInText(text, mentions.value)
  clear()
  void aiStore.sendMessage(text, {}, marked)
}

function clear() {
  content.value = ''
  mentions.value = []
  picker.close()
}

function onKeydown(event: KeyboardEvent) {
  if (picker.isOpen.value) {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault()
      picker.move(event.key === 'ArrowDown' ? 1 : -1)
      return
    }
    if ((event.key === 'Enter' || event.key === 'Tab') && picker.active.value) {
      event.preventDefault()
      select(picker.active.value)
      return
    }
    if (event.key === 'Escape') {
      event.preventDefault()
      picker.close()
      return
    }
  }
  // No teclado de toque, Enter pula linha (como nos apps de mensagem): enviar
  // é pelo botão. Sem isso, a pergunta ia pela metade ao tentar quebrar linha.
  if (event.key === 'Enter' && !event.shiftKey && !event.isComposing && !touch) {
    event.preventDefault()
    send()
  }
}

/** Setas e Home/End mexem o cursor sem digitar: a lista acompanha. */
function onKeyup(event: KeyboardEvent) {
  if (!picker.isOpen.value && ['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) {
    syncPicker()
  }
}

/** Devolve ao campo a pergunta desfeita, com o que estava marcado. */
function setDraft(draft: AiDraft) {
  mentions.value = draft.mentions
  content.value = draft.content
  picker.close()
  placeCaret(draft.content.length)
}

/** Foco no campo (o painel chama ao abrir, fora do celular). */
function focus() {
  placeCaret(content.value.length)
}

defineExpose({ setDraft, focus })
</script>

<style scoped>
/* `bg-surface-variant` é cinza-claro no tema escuro e o texto de ênfase média
   é branco: o contraste ficava ilegível. A barra usa um véu do próprio tema. */
.composer-bar {
  background: rgba(var(--v-theme-on-surface), 0.05);
  color: rgb(var(--v-theme-on-surface));
}

.context-meter {
  color: rgba(var(--v-theme-on-surface), var(--v-medium-emphasis-opacity));
  white-space: nowrap;
  cursor: pointer;
  padding: 2px 6px;
  border-radius: 999px;
  background: none;
  border: none;
  font: inherit;
}

.context-meter:hover,
.context-meter:focus-visible {
  background: rgba(var(--v-theme-on-surface), 0.08);
  outline: none;
}

.composer-field {
  position: relative;
}

.mention-menu {
  position: absolute;
  bottom: calc(100% + 4px);
  left: 12px;
  right: 12px;
  max-width: 420px;
  z-index: 10;
}

/* O painel lateral esconde o overflow de toda v-list; esta precisa rolar. */
.mention-list {
  overflow-y: auto !important;
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

:deep(.composer-textarea .v-field) {
  border-radius: 0 !important;
  border-top: none !important;
  border-bottom: none !important;
}
</style>
