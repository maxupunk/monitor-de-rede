<template>
  <div>
    <div class="d-flex align-center mb-2">
      <div class="text-subtitle-1 font-weight-bold">Console</div>
      <LiveIndicator v-if="live" class="ml-2" />
      <v-spacer></v-spacer>
      <v-btn
        size="small"
        variant="text"
        color="primary"
        :prepend-icon="copied ? 'mdi-check' : 'mdi-content-copy'"
        :disabled="!lines.length"
        @click="copy"
      >
        {{ copied ? 'Copiado' : 'Copiar log' }}
      </v-btn>
    </div>
    <div
      ref="consoleRef"
      class="deploy-console"
      role="log"
      aria-live="polite"
      aria-label="Console"
      tabindex="0"
      data-test="console"
    >
      <div v-if="error" class="text-error">{{ error }}</div>
      <div v-else-if="!lines.length" class="console-empty">{{ emptyText }}</div>
      <div
        v-for="(line, index) in lines"
        :key="index"
        :class="{ 'console-error': isErrorLine(line), 'console-ok': isOkLine(line) }"
      >
        {{ line }}
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * O console ao vivo de um deploy ou de uma compilação: segue o fim enquanto
 * roda (sem roubar a rolagem de quem subiu para ler), destaca erro e
 * sucesso, e copia o log inteiro.
 */
import { nextTick, ref, watch } from 'vue'
import LiveIndicator from '@/components/LiveIndicator.vue'

const props = withDefaults(
  defineProps<{
    lines: string[]
    live?: boolean
    error?: string | null
    emptyText?: string
  }>(),
  { live: false, error: null, emptyText: 'Nenhuma linha registrada.' }
)

const consoleRef = ref<HTMLElement | null>(null)
const copied = ref(false)

/** Erro de verdade: "0 errors", "no errors" e "sem erros" não contam. */
const ERROR_LINE = /^✖|(?<!\b(?:0|no|sem) )\b(?:error|erro|falh|failed|unhealthy)/i

function isErrorLine(line: string): boolean {
  return ERROR_LINE.test(line)
}

function isOkLine(line: string): boolean {
  return line.startsWith('✔')
}

async function copy() {
  try {
    await navigator.clipboard.writeText(props.lines.join('\n'))
    copied.value = true
    setTimeout(() => (copied.value = false), 2000)
  } catch {
    copied.value = false
  }
}

watch(
  () => props.lines.length,
  async () => {
    const element = consoleRef.value
    if (!element) return
    const nearBottom = element.scrollHeight - element.scrollTop - element.clientHeight < 80
    await nextTick()
    if (nearBottom) element.scrollTop = element.scrollHeight
  }
)
</script>

<style scoped>
.deploy-console {
  background: #0f1419;
  color: #e6edf3;
  font-family:
    'JetBrains Mono', 'Fira Code', ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 0.8rem;
  line-height: 1.45;
  border-radius: 8px;
  padding: 12px 14px;
  height: min(46vh, 460px);
  overflow-y: auto;
  white-space: pre-wrap;
  word-break: break-word;
}

.console-empty {
  color: #c9d1d9;
}

.console-error {
  color: #ff8a80;
}

.console-ok {
  color: #7ee787;
}
</style>
