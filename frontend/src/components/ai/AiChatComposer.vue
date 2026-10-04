<template>
  <div>
    <div class="ai-composer">
      <!-- Lista do @ -->
      <v-card v-if="picker.isOpen.value" class="mention-menu" elevation="8" rounded="lg">
        <div class="px-3 pt-2 pb-1 text-body-small font-weight-bold">Marcar com @</div>
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
      <div v-if="mentions.length > 0" class="d-flex flex-wrap ga-1 px-3 pt-2">
        <v-chip
          v-for="mention in mentions"
          :key="mention.kind + mention.id"
          size="small"
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
        variant="plain"
        density="comfortable"
        :rows="2"
        :max-rows="maxRows"
        auto-grow
        hide-details
        class="composer-textarea"
        aria-label="Mensagem para o assistente"
        @keydown="onKeydown"
        @input="syncPicker"
        @click="syncPicker"
        @keyup="onKeyup"
        @blur="picker.close()"
      />

      <!-- Barra de ações -->
      <div class="composer-actions">
        <v-btn
          icon="mdi-at"
          size="small"
          variant="text"
          color="primary"
          aria-label="Marcar dispositivo, monitor, container ou fonte"
          title="Marcar dispositivo, monitor, container ou fonte (@)"
          @click="startMention"
        />
        <slot name="status" />
        <span
          v-if="aiStore.isStreaming"
          class="text-body-small text-primary font-weight-medium d-flex align-center ga-1 ms-1"
        >
          <v-progress-circular indeterminate size="12" width="2" color="primary" />
          Respondendo
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

        <v-spacer />

        <v-menu v-if="context" location="top end" :close-on-content-click="false" max-width="320">
          <template #activator="{ props: menuProps }">
            <button
              v-bind="menuProps"
              type="button"
              class="context-meter d-flex align-center ga-1 text-body-small"
              :aria-label="context.detail"
              :title="context.detail"
            >
              <v-progress-circular
                :model-value="context.percent"
                :color="context.color"
                bg-color="on-surface"
                size="16"
                width="3"
              />
              <span class="d-none d-sm-inline">{{ context.label }}</span>
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
            <div class="text-body-small mb-2">
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

        <v-btn
          v-if="content.length > 0 && !aiStore.isStreaming"
          icon="mdi-close"
          size="small"
          variant="text"
          color="error"
          aria-label="Limpar o campo"
          title="Limpar o campo"
          @click="clear"
        />
        <v-btn
          v-if="aiStore.isStreaming"
          color="error"
          variant="flat"
          :size="compact ? 'small' : 'default'"
          prepend-icon="mdi-stop"
          @click="aiStore.cancelGeneration()"
        >
          Parar
        </v-btn>
        <v-btn
          v-else
          color="primary"
          variant="flat"
          :size="compact ? 'small' : 'default'"
          append-icon="mdi-send"
          :disabled="!content.trim() || !aiStore.settings?.enabled"
          @click="send"
        >
          Enviar
        </v-btn>
      </div>
    </div>

    <div v-if="showHints" class="composer-hint text-body-small">
      <kbd class="kbd-key">Enter</kbd> envia · <kbd class="kbd-key">Shift + Enter</kbd> quebra linha
      · <kbd class="kbd-key">@</kbd> marca um equipamento, monitor, container ou fonte
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { useDisplay } from 'vuetify'
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

const display = useDisplay()
/** Dicas de teclado só onde há teclado físico e espaço para elas. */
const showHints = computed(() => !touch && !props.compact && display.mdAndUp.value)

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
/* Campo e ações num bloco só, com a borda do tema — legível nos dois temas. */
.ai-composer {
  position: relative;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.22);
  border-radius: 16px;
  background: rgb(var(--v-theme-surface));
  color: rgb(var(--v-theme-on-surface));
  transition:
    border-color 0.15s ease,
    box-shadow 0.15s ease;
}

.ai-composer:focus-within {
  border-color: rgb(var(--v-theme-primary));
  box-shadow: 0 0 0 1px rgb(var(--v-theme-primary));
}

.composer-textarea :deep(.v-field__input) {
  padding: 12px 14px 4px;
  -webkit-mask-image: none;
  mask-image: none;
}

.composer-actions {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 4px;
  padding: 4px 8px 8px;
}

.composer-hint {
  margin-top: 6px;
  text-align: center;
  color: rgb(var(--v-theme-on-surface));
}

.context-meter {
  color: rgb(var(--v-theme-on-surface));
  white-space: nowrap;
  cursor: pointer;
  padding: 4px 8px;
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

.mention-menu {
  position: absolute;
  bottom: calc(100% + 6px);
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
  padding: 0 0.35rem;
  font-size: 0.72rem;
  font-family: monospace;
  background-color: rgba(var(--v-theme-on-surface), 0.08);
  border-radius: 4px;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.2);
}
</style>
