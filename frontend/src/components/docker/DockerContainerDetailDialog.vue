<template>
  <v-dialog v-model="open" :fullscreen="$vuetify.display.xs" max-width="980" scrollable>
    <v-card rounded="xl" class="docker-detail-card">
      <v-card-title class="d-flex align-center ga-2">
        <v-icon color="primary">mdi-cube-outline</v-icon>
        <span class="text-truncate">{{ live ? containerName(live) : 'Container' }}</span>
        <v-spacer></v-spacer>
        <v-btn icon="mdi-close" variant="text" @click="open = false"></v-btn>
      </v-card-title>
      <div v-if="live" class="d-flex flex-wrap align-center ga-2 px-4 pb-3">
        <v-chip
          :color="containerState(live.state).color"
          :prepend-icon="containerState(live.state).icon"
          size="small"
          variant="flat"
        >
          {{ containerState(live.state).label }}
        </v-chip>
        <v-chip size="small" color="primary" variant="tonal" prepend-icon="mdi-layers-outline">
          {{ live.image }}
        </v-chip>
        <v-spacer></v-spacer>
        <DockerContainerActions :container="live" labeled @done="onActionDone" />
      </div>
      <v-tabs v-model="detailTab" color="primary" show-arrows>
        <v-tab value="usage" prepend-icon="mdi-chart-areaspline">Consumo</v-tab>
        <v-tab value="overview" prepend-icon="mdi-information-outline">Detalhes</v-tab>
        <v-tab value="mounts" prepend-icon="mdi-database-outline">Volumes</v-tab>
        <v-tab value="networks" prepend-icon="mdi-lan">Redes</v-tab>
        <v-tab value="logs" prepend-icon="mdi-text-box-outline">Logs</v-tab>
      </v-tabs>
      <v-divider></v-divider>
      <v-card-text class="pa-0 docker-detail-body">
        <v-window v-model="detailTab">
          <v-window-item value="usage" class="pa-5">
            <div class="d-flex flex-wrap align-center ga-2 mb-4">
              <v-chip
                v-if="detailMetric"
                color="info"
                variant="tonal"
                prepend-icon="mdi-cpu-64-bit"
              >
                CPU agora: {{ formatPercent(detailMetric.cpu.usagePercent, 1) }}
              </v-chip>
              <v-chip v-if="detailMetric" color="primary" variant="tonal" prepend-icon="mdi-memory">
                RAM agora: {{ formatBinaryBytes(detailMetric.memory.usageBytes) }}
              </v-chip>
              <v-spacer></v-spacer>
              <v-btn-toggle
                v-model="usageRange"
                mandatory
                color="primary"
                variant="outlined"
                density="compact"
                divided
              >
                <v-btn v-for="option in usageRanges" :key="option" :value="option" size="small">
                  {{ option }}
                </v-btn>
              </v-btn-toggle>
            </div>
            <v-skeleton-loader v-if="usageLoading && !usage" type="card, card"></v-skeleton-loader>
            <v-alert v-else-if="usageError" type="error" variant="tonal" density="compact">
              {{ usageError }}
            </v-alert>
            <v-alert
              v-else-if="!usage || usage.points.length === 0"
              type="info"
              variant="tonal"
              density="compact"
            >
              Ainda não há consumo registrado nesta janela. O primeiro minuto aparece cerca de um
              minuto depois de o container começar a enviar métricas.
            </v-alert>
            <v-row v-else dense>
              <v-col cols="12" md="6">
                <div class="text-subtitle-2 mb-2">CPU</div>
                <BaseMetricChart :series="usageCpuSeries" unit-type="percentage"></BaseMetricChart>
              </v-col>
              <v-col cols="12" md="6">
                <div class="text-subtitle-2 mb-2">Memória</div>
                <BaseMetricChart :series="usageMemorySeries" unit-type="bytes"></BaseMetricChart>
              </v-col>
            </v-row>
            <div class="text-caption mt-3">
              Médias de 1 minuto guardadas por 30 dias.
              <router-link :to="{ name: 'docker-history', query: docker.hostQuery }"
                >Ver histórico do host</router-link
              >
            </div>
          </v-window-item>
          <v-window-item value="overview" class="pa-5">
            <v-skeleton-loader v-if="detailLoading" type="article"></v-skeleton-loader>
            <v-row v-else-if="detail">
              <v-col cols="12" md="6">
                <v-list density="compact">
                  <v-list-item title="ID" :subtitle="detail.id"></v-list-item>
                  <v-list-item title="Imagem" :subtitle="detail.image"></v-list-item>
                  <v-list-item
                    title="Hostname"
                    :subtitle="detail.config.hostname || '—'"
                  ></v-list-item>
                  <v-list-item
                    title="Usuário"
                    :subtitle="detail.config.user || 'padrão da imagem'"
                  ></v-list-item>
                </v-list>
              </v-col>
              <v-col cols="12" md="6">
                <v-list density="compact">
                  <v-list-item title="Estado" :subtitle="detail.state.status"></v-list-item>
                  <v-list-item
                    title="PID"
                    :subtitle="String(detail.state.pid || '—')"
                  ></v-list-item>
                  <v-list-item
                    title="Reinício"
                    :subtitle="detail.hostConfig.restartPolicy.name || 'não'"
                  ></v-list-item>
                  <v-list-item
                    title="Modo de rede"
                    :subtitle="detail.hostConfig.networkMode || '—'"
                  ></v-list-item>
                </v-list>
              </v-col>
              <v-col cols="12">
                <div class="text-subtitle-2 mb-2">Comando</div>
                <pre class="docker-code">{{ detail.config.command.join(' ') || '—' }}</pre>
                <div class="text-subtitle-2 mt-4 mb-2">
                  Ambiente (segredos ocultados pelo servidor)
                </div>
                <pre class="docker-code">{{ detail.config.environment.join('\n') || '—' }}</pre>
              </v-col>
            </v-row>
          </v-window-item>
          <v-window-item value="mounts" class="pa-5">
            <v-table density="compact">
              <thead>
                <tr>
                  <th>Tipo</th>
                  <th>Origem</th>
                  <th>Destino</th>
                  <th>Modo</th>
                </tr>
              </thead>
              <tbody>
                <tr
                  v-for="mount in detail?.mounts || []"
                  :key="`${mount.source}-${mount.destination}`"
                >
                  <td>{{ mount.mountType }}</td>
                  <td>{{ mount.name || mount.source }}</td>
                  <td>{{ mount.destination }}</td>
                  <td>{{ mount.readWrite ? 'RW' : 'RO' }}</td>
                </tr>
              </tbody>
            </v-table>
          </v-window-item>
          <v-window-item value="networks" class="pa-5">
            <div
              v-if="auth.isAdmin"
              class="d-flex flex-column flex-sm-row align-sm-center ga-2 mb-4"
            >
              <v-select
                v-model="networkToConnect"
                :items="availableNetworks"
                item-title="name"
                item-value="id"
                label="Adicionar rede ao container"
                variant="outlined"
                density="compact"
                hide-details
                clearable
                no-data-text="Nenhuma outra rede disponível"
                class="flex-grow-1"
              ></v-select>
              <v-btn
                color="primary"
                prepend-icon="mdi-lan-connect"
                :disabled="!networkToConnect"
                :loading="docker.actionLoading"
                @click="connectSelectedNetwork"
              >
                Adicionar
              </v-btn>
            </div>
            <v-table density="compact">
              <thead>
                <tr>
                  <th>Rede</th>
                  <th>IP</th>
                  <th>Gateway</th>
                  <th v-if="auth.isAdmin" class="text-right">Ações</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="network in detail?.networks || []" :key="network.networkId">
                  <td>{{ network.networkName }}</td>
                  <td>{{ network.ipAddress || '—' }}</td>
                  <td>{{ network.gateway || '—' }}</td>
                  <td v-if="auth.isAdmin" class="text-right">
                    <v-btn
                      icon="mdi-lan-disconnect"
                      size="small"
                      color="error"
                      variant="text"
                      :disabled="(detail?.networks.length || 0) <= 1"
                      :title="
                        (detail?.networks.length || 0) <= 1
                          ? 'O container precisa permanecer em pelo menos uma rede'
                          : `Remover da rede ${network.networkName}`
                      "
                      @click="disconnectNetwork(network.networkId, network.networkName)"
                    ></v-btn>
                  </td>
                </tr>
              </tbody>
            </v-table>
            <v-alert
              v-if="auth.isAdmin && (detail?.networks.length || 0) <= 1"
              type="info"
              variant="tonal"
              density="compact"
              class="mt-4"
            >
              A última rede não pode ser removida, evitando deixar o container sem conectividade.
            </v-alert>
          </v-window-item>
          <v-window-item value="logs" class="pa-4">
            <div class="d-flex flex-wrap align-center ga-2 mb-3">
              <v-select
                v-model="logTail"
                :items="logTailOptions"
                label="Linhas"
                variant="outlined"
                density="compact"
                hide-details
                max-width="180"
              ></v-select>
              <v-btn
                color="primary"
                variant="tonal"
                prepend-icon="mdi-refresh"
                :loading="logsLoading"
                :disabled="Boolean(followStreamId)"
                @click="loadLogs()"
              >
                Atualizar
              </v-btn>
              <v-btn
                :color="followStreamId ? 'success' : 'primary'"
                :variant="followStreamId ? 'flat' : 'tonal'"
                :prepend-icon="followStreamId ? 'mdi-pause' : 'mdi-play-circle-outline'"
                :loading="followStarting"
                @click="toggleFollow"
              >
                {{ followStreamId ? 'Pausar ao vivo' : 'Ao vivo' }}
              </v-btn>
              <v-spacer></v-spacer>
              <v-btn
                color="primary"
                variant="tonal"
                prepend-icon="mdi-download"
                :loading="logsDownloading"
                @click="downloadLogs"
              >
                Baixar logs
              </v-btn>
              <v-btn
                v-if="auth.isAdmin"
                color="error"
                variant="tonal"
                prepend-icon="mdi-delete-forever-outline"
                :loading="logsClearing"
                @click="clearLogs"
              >
                Limpar logs
              </v-btn>
            </div>
            <v-alert v-if="followEnded" type="info" variant="tonal" density="compact" class="mb-2">
              {{ followEnded }}
            </v-alert>
            <pre class="docker-logs">{{ formattedLogs }}</pre>
          </v-window-item>
        </v-window>
      </v-card-text>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import BaseMetricChart, { type ChartSeriesInput } from '@/components/BaseMetricChart.vue'
import DockerContainerActions from '@/components/docker/DockerContainerActions.vue'
import { confirm } from '@/composables/useConfirm'
import type { ActionOutcome } from '@/composables/useContainerActions'
import { useAuthStore } from '@/stores/auth'
import { useDockerStore } from '@/stores/docker'
import type { ContainerHistoryResponse } from '@/bindings/ContainerHistoryResponse'
import type { DockerContainerDetail } from '@/bindings/DockerContainerDetail'
import type { DockerContainerSummary } from '@/bindings/DockerContainerSummary'
import type { DockerLogEntry } from '@/bindings/DockerLogEntry'
import type { MetricsRange } from '@/bindings/MetricsRange'
import { containerName, containerState } from '@/utils/dockerPresentation'
import { formatBinaryBytes, formatPercent } from '@/utils/formatters'

/**
 * Detalhe de um container: consumo, configuração, volumes, redes e logs, com
 * as ações de ciclo de vida no topo. Os dados vêm sob demanda (não-live),
 * menos o log ao vivo, que chega pelo SSE.
 */
const props = defineProps<{ container: DockerContainerSummary | null }>()
const open = defineModel<boolean>({ required: true })
const emit = defineEmits<{ notify: [message: string, color: string] }>()

const docker = useDockerStore()
const auth = useAuthStore()

const detailLoading = ref(false)
const detail = ref<DockerContainerDetail | null>(null)
const detailTab = ref('usage')
const usageRanges: MetricsRange[] = ['1h', '6h', '24h', '7d', '30d']
const usageRange = ref<MetricsRange>('1h')
const usage = ref<ContainerHistoryResponse | null>(null)
const usageLoading = ref(false)
const usageError = ref<string | null>(null)
let usageContainerName = ''
const logs = ref<DockerLogEntry[]>([])
const logsLoading = ref(false)
const logsDownloading = ref(false)
const logsClearing = ref(false)
const logTail = ref<number | 'all'>(500)
const logTailOptions = [100, 500, 1000, 5000, { title: 'Todos', value: 'all' }]
const followStreamId = ref<string | null>(null)
const followStarting = ref(false)
const networkToConnect = ref<string | null>(null)
let detailRefreshing = false
let logsRefreshing = false

/** O estado atual vem do inventário ao vivo; o da abertura é só o ponto de partida. */
const live = computed(
  () => docker.containers.find((item) => item.id === props.container?.id) ?? props.container
)

const detailMetric = computed(() =>
  detail.value
    ? docker.metrics?.containers.find((metric) => metric.containerId === detail.value?.id)
    : undefined
)

const availableNetworks = computed(() => {
  const connected = new Set((detail.value?.networks ?? []).map((network) => network.networkId))
  return docker.networks
    .filter((network) => !connected.has(network.id))
    .sort((left, right) => left.name.localeCompare(right.name))
})

/** No modo ao vivo o log vem do stream SSE (`docker:log`), não de nova consulta. */
const shownLogs = computed<DockerLogEntry[]>(() =>
  followStreamId.value ? (docker.logStreams[followStreamId.value]?.entries ?? []) : logs.value
)

const followEnded = computed(() =>
  followStreamId.value ? (docker.logStreams[followStreamId.value]?.ended ?? null) : null
)

function logLine(entry: DockerLogEntry): string {
  return `${entry.timestamp ? `[${entry.timestamp}] ` : ''}${entry.stream}: ${entry.message}`
}

const formattedLogs = computed(() =>
  shownLogs.value.length ? shownLogs.value.map(logLine).join('\n') : 'Nenhum log encontrado.'
)

const usageCpuSeries = computed<ChartSeriesInput[]>(() => [
  {
    id: 'usage-cpu-avg',
    label: 'Média',
    color: '#2196F3',
    fillArea: true,
    data: (usage.value?.points ?? []).map((point) => ({ time: point.at, value: point.cpuAvg })),
  },
  {
    id: 'usage-cpu-max',
    label: 'Pico',
    color: '#FF7043',
    data: (usage.value?.points ?? []).map((point) => ({ time: point.at, value: point.cpuMax })),
  },
])

const usageMemorySeries = computed<ChartSeriesInput[]>(() => [
  {
    id: 'usage-memory',
    label: 'Pico',
    color: '#7E57C2',
    fillArea: true,
    data: (usage.value?.points ?? []).map((point) => ({
      time: point.at,
      value: point.memoryMax,
      formattedValue: formatBinaryBytes(point.memoryMax),
    })),
  },
])

function notify(message: string, color: string) {
  emit('notify', message, color)
}

function onActionDone(outcome: ActionOutcome) {
  notify(outcome.message, outcome.ok ? 'success' : 'error')
  // Removido não tem mais detalhe para mostrar.
  if (outcome.ok && !docker.containers.some((item) => item.id === props.container?.id)) {
    open.value = false
    return
  }
  void refreshDetail(true)
}

/** Histórico sob demanda (não-live): só carrega ao abrir ou trocar a janela. */
async function loadUsage(): Promise<void> {
  if (!usageContainerName) return
  usageLoading.value = true
  usageError.value = null
  try {
    usage.value = await docker.api.containerHistory(usageContainerName, usageRange.value)
  } catch (reason: unknown) {
    usageError.value = reason instanceof Error ? reason.message : 'Erro ao carregar o consumo'
  } finally {
    usageLoading.value = false
  }
}

async function load(container: DockerContainerSummary): Promise<void> {
  detail.value = null
  detailTab.value = 'usage'
  detailLoading.value = true
  logs.value = []
  usage.value = null
  usageContainerName = containerName(container)
  void loadUsage()
  networkToConnect.value = null
  try {
    detail.value = await docker.api.container(container.id)
  } catch (reason: unknown) {
    notify(reason instanceof Error ? reason.message : 'Erro ao inspecionar container', 'error')
  } finally {
    detailLoading.value = false
  }
}

async function refreshDetail(silent = false): Promise<void> {
  if (!detail.value || detailRefreshing) return
  detailRefreshing = true
  if (!silent) detailLoading.value = true
  try {
    detail.value = await docker.api.container(detail.value.id)
  } catch (reason: unknown) {
    if (!silent) {
      notify(reason instanceof Error ? reason.message : 'Erro ao atualizar container', 'error')
    }
  } finally {
    detailRefreshing = false
    if (!silent) detailLoading.value = false
  }
}

async function loadLogs(): Promise<void> {
  if (!detail.value || logsRefreshing) return
  logsRefreshing = true
  logsLoading.value = true
  try {
    logs.value = await docker.api.logs(detail.value.id, { tail: logTail.value, timestamps: true })
  } catch (reason: unknown) {
    notify(reason instanceof Error ? reason.message : 'Erro ao carregar logs', 'error')
  } finally {
    logsRefreshing = false
    logsLoading.value = false
  }
}

function sanitizeFileName(value: string): string {
  return value.replace(/^\//, '').replace(/[^a-zA-Z0-9._-]+/g, '-') || 'container'
}

async function downloadLogs(): Promise<void> {
  if (!detail.value) return
  logsDownloading.value = true
  try {
    const entries = await docker.api.logs(detail.value.id, { tail: 'all', timestamps: true })
    const content = entries.map(logLine).join('\n')
    const url = URL.createObjectURL(new Blob([content], { type: 'text/plain;charset=utf-8' }))
    const anchor = document.createElement('a')
    anchor.href = url
    anchor.download = `${sanitizeFileName(detail.value.name || detail.value.id)}-logs.txt`
    document.body.appendChild(anchor)
    anchor.click()
    anchor.remove()
    URL.revokeObjectURL(url)
    notify('Download dos logs iniciado.', 'success')
  } catch (reason: unknown) {
    notify(reason instanceof Error ? reason.message : 'Erro ao baixar logs', 'error')
  } finally {
    logsDownloading.value = false
  }
}

async function clearLogs(): Promise<void> {
  if (!detail.value) return
  const accepted = await confirm({
    title: 'Apagar logs da Docker Engine',
    message:
      'Esta ação é irreversível. Containers ativos serão parados brevemente e iniciados novamente para apagar o arquivo de log com segurança.',
    confirmText: 'Apagar logs',
    confirmColor: 'error',
    icon: 'mdi-delete-forever-outline',
  })
  if (!accepted) return
  logsClearing.value = true
  try {
    const response = await docker.api.clearLogs(detail.value.id)
    logs.value = []
    await refreshDetail(true)
    notify(response.message, 'success')
  } catch (reason: unknown) {
    notify(reason instanceof Error ? reason.message : 'Erro ao apagar logs do container', 'error')
  } finally {
    logsClearing.value = false
  }
}

async function connectSelectedNetwork(): Promise<void> {
  if (!detail.value || !networkToConnect.value) return
  const containerId = detail.value.id
  const networkId = networkToConnect.value
  const success = await docker.runAction(
    () => docker.api.connectNetwork(networkId, containerId),
    () => refreshDetail(true)
  )
  if (success) networkToConnect.value = null
  notify(
    success ? 'Rede adicionada ao container.' : docker.error || 'Erro ao adicionar rede',
    success ? 'success' : 'error'
  )
}

async function disconnectNetwork(networkId: string, networkName: string): Promise<void> {
  if (!detail.value || detail.value.networks.length <= 1) return
  const accepted = await confirm({
    title: 'Remover rede do container',
    message: `Desconectar o container da rede "${networkName}"?`,
    confirmText: 'Desconectar',
    confirmColor: 'error',
    icon: 'mdi-lan-disconnect',
  })
  if (!accepted) return
  const containerId = detail.value.id
  const success = await docker.runAction(
    () => docker.api.disconnectNetwork(networkId, containerId),
    () => refreshDetail(true)
  )
  notify(
    success ? 'Rede removida do container.' : docker.error || 'Erro ao remover rede',
    success ? 'success' : 'error'
  )
}

async function toggleFollow(): Promise<void> {
  if (followStreamId.value) {
    await stopFollow()
    return
  }
  if (!detail.value) return
  followStarting.value = true
  try {
    followStreamId.value = await docker.startLogStream(detail.value.id, logTail.value)
  } catch (reason: unknown) {
    notify(reason instanceof Error ? reason.message : 'Erro ao acompanhar os logs', 'error')
  } finally {
    followStarting.value = false
  }
}

async function stopFollow(): Promise<void> {
  const streamId = followStreamId.value
  followStreamId.value = null
  if (streamId) await docker.stopLogStream(streamId)
}

watch(
  () => [open.value, props.container?.id] as const,
  ([isOpen]) => {
    if (isOpen && props.container) void load(props.container)
    if (!isOpen) void stopFollow()
  }
)

watch(detailTab, (tab) => {
  if (tab === 'logs' && logs.value.length === 0) void loadLogs()
})

watch(usageRange, () => void loadUsage())

onBeforeUnmount(() => {
  void stopFollow()
})
</script>

<style scoped>
.docker-detail-card {
  background: rgb(var(--v-theme-surface));
  color: rgb(var(--v-theme-on-surface));
}
/*
 * O card do diálogo é um flex column: título, abas e corpo dividem a altura
 * disponível. O corpo é o único item rolável, mas o `min-height` fixo o
 * impedia de encolher — num viewport curto o excedente espremia o título e as
 * abas, que ficavam por baixo do conteúdo. Travar o cabeçalho elimina a
 * sobreposição e o `min()` mantém a altura estável entre as abas sem pedir
 * mais espaço do que o diálogo tem para dar.
 */
.docker-detail-card > .v-card-title,
.docker-detail-card > .v-tabs,
.docker-detail-card > .v-divider {
  flex: 0 0 auto;
}
.docker-detail-body {
  min-height: min(420px, 45vh);
}
.docker-code,
.docker-logs {
  border-radius: 8px;
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  font-size: 0.78rem;
  overflow: auto;
  padding: 12px;
  white-space: pre-wrap;
  word-break: break-all;
}
.docker-code {
  background: rgba(var(--v-theme-primary), 0.065);
  border: 1px solid rgba(var(--v-theme-primary), 0.18);
  color: rgb(var(--v-theme-on-surface));
}
.docker-logs {
  background: #0f172a;
  border: 1px solid #334155;
  color: #e2e8f0;
  min-height: 340px;
  max-height: 55vh;
  white-space: pre;
}
</style>
