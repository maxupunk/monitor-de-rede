<template>
  <div>
    <v-alert v-if="error" type="error" variant="tonal" class="mb-4" border="start">
      {{ error }}
    </v-alert>

    <v-infinite-scroll :key="scrollKey" @load="load">
      <v-table density="compact" class="rounded-lg border">
        <thead>
          <tr>
            <th class="text-left" style="width: 170px">Recebido</th>
            <th class="text-left" style="width: 120px">Severidade</th>
            <th v-if="showSource" class="text-left" style="width: 200px">Origem</th>
            <th class="text-left">Mensagem</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="entry in entries" :key="entry.id">
            <td class="text-caption text-no-wrap">{{ formatReceivedAt(entry.receivedAt) }}</td>
            <td>
              <v-chip
                :color="severityColor(entry.severity)"
                size="x-small"
                variant="tonal"
                class="text-capitalize"
              >
                {{ entry.severityLabel ?? 'sem nível' }}
              </v-chip>
            </td>
            <td v-if="showSource" class="text-caption">
              <RouterLink
                v-if="entry.deviceId"
                :to="{ name: 'device-detail', params: { id: entry.deviceId } }"
                class="text-primary font-weight-medium text-decoration-none"
              >
                {{ entry.deviceName ?? `Dispositivo ${entry.deviceId}` }}
              </RouterLink>
              <span v-else class="text-grey">{{ entry.hostname ?? entry.sourceIp }}</span>
              <div class="text-grey text-caption">{{ entry.sourceIp }}</div>
            </td>
            <td class="text-body-2">
              <span class="log-message">{{ entry.message }}</span>
              <div v-if="entry.topics.length > 0" class="mt-1">
                <v-chip
                  v-for="topic in entry.topics"
                  :key="topic"
                  size="x-small"
                  variant="outlined"
                  class="mr-1"
                >
                  {{ topic }}
                </v-chip>
              </div>
              <div v-else-if="entry.appName" class="text-caption text-grey mt-1">
                {{ entry.appName }}<template v-if="entry.pid">[{{ entry.pid }}]</template>
              </div>
            </td>
          </tr>
        </tbody>
      </v-table>
      <template #empty>
        <div class="text-caption text-grey text-center py-4">
          Não há mais registros no período consultado.
        </div>
      </template>
    </v-infinite-scroll>

    <div v-if="entries.length === 0" class="pa-8 text-center text-grey">
      <v-icon size="48" color="grey-lighten-1" class="mb-2">mdi-text-box-search-outline</v-icon>
      <div class="text-subtitle-2 font-weight-medium">Nenhum registro encontrado</div>
      <div class="text-caption">{{ emptyHint }}</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { RouterLink } from 'vue-router'
import { severityColor, type LogEntry } from '@/stores/logs'

/**
 * A tabela é compartilhada pela página `/logs` e pela aba de logs do
 * dispositivo. Duas cópias divergiriam na primeira alteração de coluna — e a
 * coluna de origem é justamente a que muda entre as duas: na aba do
 * dispositivo ela seria a mesma linha repetida centenas de vezes.
 */
withDefaults(
  defineProps<{
    entries: LogEntry[]
    scrollKey: number
    load: (context: { done: (status: 'ok' | 'empty' | 'loading' | 'error') => void }) => void
    error?: string | null
    showSource?: boolean
    emptyHint?: string
  }>(),
  {
    error: null,
    showSource: true,
    emptyHint: 'Ajuste os filtros ou verifique se os roteadores estão enviando syslog.',
  }
)

function formatReceivedAt(value: string): string {
  return new Date(value).toLocaleString('pt-BR', { dateStyle: 'short', timeStyle: 'medium' })
}
</script>

<style scoped>
/* Mensagem de log é texto pré-formatado: quebrar palavra longa é melhor do que
   estourar a largura da tabela com uma linha de firewall. */
.log-message {
  font-family: 'Roboto Mono', 'Courier New', monospace;
  font-size: 0.8125rem;
  word-break: break-word;
}
</style>
