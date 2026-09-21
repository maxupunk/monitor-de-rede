<template>
  <v-card variant="outlined" density="compact" class="my-2 rounded-lg tool-card" :color="cardColor">
    <div
      class="pa-2 d-flex align-center justify-space-between cursor-pointer"
      @click="expanded = !expanded"
    >
      <div class="d-flex align-center ga-2">
        <v-avatar size="28" :color="meta.color" variant="tonal">
          <v-icon size="16">{{ meta.icon }}</v-icon>
        </v-avatar>
        <div>
          <div class="text-caption font-weight-bold">{{ meta.label }}</div>
          <div class="text-caption text-grey text-truncate max-w-300">
            {{ formatArgsSummary(tool.arguments) }}
          </div>
        </div>
      </div>

      <div class="d-flex align-center ga-1">
        <v-chip
          size="x-small"
          :color="
            tool.status === 'running' ? 'primary' : tool.status === 'error' ? 'error' : 'success'
          "
          variant="tonal"
          class="font-weight-medium"
        >
          <v-progress-circular
            v-if="tool.status === 'running'"
            indeterminate
            size="10"
            width="2"
            class="mr-1"
          />
          <v-icon v-else start size="12">
            {{ tool.status === 'error' ? 'mdi-alert-circle' : 'mdi-check' }}
          </v-icon>
          {{
            tool.status === 'running'
              ? 'Executando'
              : tool.status === 'error'
                ? 'Falha'
                : 'Concluído'
          }}
        </v-chip>

        <v-btn icon size="x-small" variant="text" :color="cardColor">
          <v-icon size="16">{{ expanded ? 'mdi-chevron-up' : 'mdi-chevron-down' }}</v-icon>
        </v-btn>
      </div>
    </div>

    <v-expand-transition>
      <div v-if="expanded" class="pa-3 pt-0 border-t mt-1">
        <div class="text-caption font-weight-bold text-grey-darken-1 mb-1">Parâmetros:</div>
        <pre class="bg-grey-lighten-4 pa-2 rounded text-caption font-mono mb-2 overflow-x-auto">{{
          JSON.stringify(tool.arguments, null, 2)
        }}</pre>

        <div v-if="tool.result" class="text-caption font-weight-bold text-grey-darken-1 mb-1">
          Resultado:
        </div>
        <pre
          v-if="tool.result"
          class="bg-grey-lighten-4 pa-2 rounded text-caption font-mono overflow-x-auto max-h-200"
          >{{ JSON.stringify(tool.result, null, 2) }}</pre>
      </div>
    </v-expand-transition>
  </v-card>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import type { AiToolCallState } from '@/stores/ai'

const props = defineProps<{
  tool: AiToolCallState
}>()

const expanded = ref(false)

interface ToolMeta {
  label: string
  icon: string
  color: string
}

const meta = computed<ToolMeta>(() => {
  switch (props.tool.name) {
    case 'ping_host':
      return { label: 'ICMP Ping', icon: 'mdi-pulse', color: 'primary' }
    case 'traceroute':
      return { label: 'Traceroute', icon: 'mdi-routes', color: 'info' }
    case 'scan_ports':
      return { label: 'Scan de Portas TCP', icon: 'mdi-lan-connect', color: 'warning' }
    case 'dns_lookup':
      return { label: 'Resolução DNS', icon: 'mdi-dns', color: 'teal' }
    case 'run_playbook':
      return {
        label: 'Playbook de Diagnóstico',
        icon: 'mdi-clipboard-play-outline',
        color: 'purple',
      }
    case 'list_devices':
      return { label: 'Consulta de Dispositivos', icon: 'mdi-devices', color: 'blue' }
    case 'get_device_detail':
      return { label: 'Detalhes do Dispositivo', icon: 'mdi-information-outline', color: 'indigo' }
    case 'get_active_alerts':
      return { label: 'Alertas Recentes', icon: 'mdi-bell-alert-outline', color: 'orange' }
    case 'get_system_summary':
      return { label: 'Resumo da Infraestrutura', icon: 'mdi-chart-box-outline', color: 'cyan' }
    case 'search_system_docs':
      return {
        label: 'Base de Conhecimento',
        icon: 'mdi-book-open-page-variant-outline',
        color: 'green',
      }
    default:
      return { label: props.tool.name, icon: 'mdi-cog-outline', color: 'grey' }
  }
})

const cardColor = computed(() => {
  if (props.tool.status === 'running') return 'primary'
  if (props.tool.status === 'error') return 'error'
  return 'grey-lighten-1'
})

function formatArgsSummary(args: Record<string, unknown>): string {
  if (args.target) return `Alvo: ${args.target}`
  if (args.hostname) return `Host: ${args.hostname}`
  if (args.identifier) return `Dispositivo: ${args.identifier}`
  if (args.playbook_type) return `Playbook: ${args.playbook_type}`
  if (args.query) return `Busca: "${args.query}"`
  const keys = Object.keys(args)
  if (keys.length > 0) return `${keys.join(', ')}`
  return 'Sem parâmetros adicionais'
}
</script>

<style scoped>
.tool-card {
  background-color: rgba(var(--v-theme-surface), 0.9);
  transition: border-color 0.2s ease;
}
.max-w-300 {
  max-width: 300px;
}
.max-h-200 {
  max-height: 200px;
}
</style>
