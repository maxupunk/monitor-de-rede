<template>
  <div class="copyable-command">
    <div class="d-flex align-center mb-1">
      <span class="text-subtitle-2">{{ label }}</span>
      <v-spacer></v-spacer>
      <v-btn
        size="small"
        variant="tonal"
        color="primary"
        :prepend-icon="copied ? 'mdi-check' : 'mdi-content-copy'"
        @click="copy"
      >
        {{ copied ? 'Copiado' : 'Copiar' }}
      </v-btn>
    </div>
    <pre class="copyable-command__code">{{ command }}</pre>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'

const props = defineProps<{
  label: string
  command: string
}>()

const copied = ref(false)

watch(
  () => props.command,
  () => {
    copied.value = false
  }
)

async function copy() {
  try {
    await navigator.clipboard.writeText(props.command)
    copied.value = true
  } catch {
    copied.value = false
  }
}
</script>

<style scoped>
.copyable-command__code {
  background: rgba(var(--v-theme-on-surface), 0.06);
  border-radius: 8px;
  padding: 12px;
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 0.8rem;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
</style>
