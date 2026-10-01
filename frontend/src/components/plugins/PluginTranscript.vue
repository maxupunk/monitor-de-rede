<template>
  <div v-if="entries.length === 0" class="text-body-2">Nenhum acesso ao equipamento.</div>
  <v-timeline v-else density="compact" side="end" align="start" truncate-line="both">
    <v-timeline-item
      v-for="entry in entries"
      :key="entry.seq"
      :dot-color="kindPresentation(entry.kind).color"
      size="small"
    >
      <div class="d-flex align-center flex-wrap ga-2 mb-1">
        <v-chip size="x-small" :color="kindPresentation(entry.kind).color" variant="flat">
          <v-icon start size="12">{{ kindPresentation(entry.kind).icon }}</v-icon>
          {{ kindPresentation(entry.kind).label }}
        </v-chip>
        <v-chip
          v-if="entry.effect"
          size="x-small"
          :color="effectPresentation(entry.effect).color"
          variant="tonal"
        >
          {{ effectPresentation(entry.effect).label }}
        </v-chip>
        <span v-if="entry.status !== undefined" class="text-body-small font-weight-medium">
          {{ entry.kind === 'http' ? `HTTP ${entry.status}` : `saída ${entry.status}` }}
        </span>
        <span class="text-body-small">
          {{ formatElapsedMs(entry.durationMs) }} · {{ entry.origin }}
        </span>
      </div>
      <code v-if="entry.request" class="d-block font-mono text-body-small mb-1 request">{{
        entry.request
      }}</code>
      <pre v-if="entry.output" class="output font-mono text-body-small pa-2 rounded">{{
        entry.output
      }}</pre>
    </v-timeline-item>
  </v-timeline>
</template>

<script setup lang="ts">
import type { TranscriptEntry } from '@/bindings/TranscriptEntry'
import { formatElapsedMs } from '@/utils/formatters'
import { effectPresentation, type Presentation } from '@/utils/pluginPresentation'

defineProps<{ entries: TranscriptEntry[] }>()

const KINDS: Record<string, Presentation> = {
  ssh: { label: 'SSH', color: 'primary', icon: 'mdi-console' },
  telnet: { label: 'Telnet', color: 'primary', icon: 'mdi-console-line' },
  http: { label: 'HTTP', color: 'info', icon: 'mdi-web' },
  log: { label: 'Log do script', color: 'secondary', icon: 'mdi-text' },
  denied: { label: 'Recusado', color: 'error', icon: 'mdi-hand-back-right-off-outline' },
  error: { label: 'Erro', color: 'error', icon: 'mdi-alert-circle-outline' },
}

function kindPresentation(kind: string): Presentation {
  return KINDS[kind] ?? { label: kind, color: 'secondary', icon: 'mdi-circle-small' }
}
</script>

<style scoped>
.request {
  word-break: break-all;
}
.output {
  background-color: rgb(var(--v-theme-surface));
  border: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 220px;
  overflow: auto;
}
</style>
