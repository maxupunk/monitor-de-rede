<template>
  <div>
    <PageHeader title="Containers" subtitle="Inventário, consumo, logs e controle do ciclo de vida">
      <template #actions>
        <v-btn
          color="primary"
          prepend-icon="mdi-refresh"
          :loading="docker.loading"
          @click="docker.refreshContainers()"
        >
          Atualizar
        </v-btn>
      </template>
    </PageHeader>

    <v-alert v-if="docker.error" type="error" variant="tonal" closable class="mb-4">
      {{ docker.error }}
    </v-alert>
    <v-alert
      v-if="docker.metrics?.failedContainerCount"
      type="warning"
      variant="tonal"
      class="mb-4"
    >
      Métricas parciais: {{ docker.metrics.failedContainerCount }} container(s) sem amostra.
    </v-alert>
    <v-card rounded="xl" variant="outlined" class="mb-4">
      <div class="d-flex flex-column flex-md-row ga-3 pa-4">
        <v-text-field
          v-model="search"
          prepend-inner-icon="mdi-magnify"
          label="Buscar por nome, imagem ou projeto"
          variant="outlined"
          density="compact"
          hide-details
          clearable
        ></v-text-field>
        <v-select
          v-model="stateFilter"
          :items="stateOptions"
          label="Estado"
          variant="outlined"
          density="compact"
          hide-details
          class="docker-filter"
        ></v-select>
      </div>
    </v-card>

    <v-expansion-panels v-if="containerGroups.length" v-model="expandedGroups" multiple>
      <v-expansion-panel
        v-for="group in containerGroups"
        :key="group.key"
        :value="group.key"
        rounded="xl"
        class="mb-3 docker-group-panel"
      >
        <v-expansion-panel-title>
          <div class="d-flex flex-wrap align-center ga-2 w-100 pr-2">
            <v-avatar color="primary" variant="tonal" size="32">
              <v-icon size="18">mdi-folder-multiple-outline</v-icon>
            </v-avatar>
            <div class="font-weight-bold">{{ group.label }}</div>
            <v-chip size="x-small" color="primary" variant="tonal">
              {{ group.containers.length }}
            </v-chip>
            <v-chip size="x-small" color="success" variant="tonal">
              {{ group.running }} ativos
            </v-chip>
            <v-spacer></v-spacer>
            <div class="docker-group-totals text-caption text-medium-emphasis">
              CPU {{ group.cpuPercent.toFixed(2) }}% · RAM
              {{ formatBinaryBytes(group.memoryBytes) }}
            </div>
          </div>
        </v-expansion-panel-title>
        <v-expansion-panel-text>
          <ResponsiveDataTable
            :headers="headers"
            :items="group.containers"
            :loading="docker.loading"
            :items-per-page="15"
            no-data-text="Nenhum container encontrado"
            clickable
            @click:row="onRowClick"
          >
            <template #item.name="{ item }">
              <div class="py-2">
                <div class="font-weight-bold">{{ containerName(item) }}</div>
                <div class="text-caption text-medium-emphasis font-mono">
                  {{ shortId(item.id) }}
                </div>
              </div>
            </template>
            <template #item.state="{ item }">
              <v-chip :color="stateColor(item.state)" size="small" variant="tonal">
                {{ stateLabel(item.state) }}
              </v-chip>
            </template>
            <template #item.resources="{ item }">
              <div v-if="metricFor(item.id)" class="text-caption py-1">
                <div>CPU {{ metricFor(item.id)?.cpu.usagePercent.toFixed(2) }}%</div>
                <div>RAM {{ metricFor(item.id)?.memory.usagePercent.toFixed(2) }}%</div>
              </div>
              <span v-else class="text-medium-emphasis">—</span>
            </template>
            <template #item.actions="{ item }">
              <div class="d-flex ga-1 justify-end" @click.stop>
                <v-btn
                  icon="mdi-eye-outline"
                  size="small"
                  variant="text"
                  title="Inspecionar"
                  @click="openDetail(item)"
                ></v-btn>
                <v-btn
                  v-if="auth.isAdmin && item.state !== 'running'"
                  icon="mdi-play"
                  size="small"
                  color="success"
                  variant="text"
                  title="Iniciar"
                  :loading="docker.actionLoading"
                  @click="runContainerAction(item, 'start')"
                ></v-btn>
                <v-btn
                  v-if="auth.isAdmin && item.state === 'running'"
                  icon="mdi-stop"
                  size="small"
                  color="warning"
                  variant="text"
                  title="Parar"
                  :loading="docker.actionLoading"
                  @click="runContainerAction(item, 'stop')"
                ></v-btn>
                <v-btn
                  v-if="auth.isAdmin"
                  icon="mdi-restart"
                  size="small"
                  color="info"
                  variant="text"
                  title="Reiniciar"
                  :loading="docker.actionLoading"
                  @click="runContainerAction(item, 'restart')"
                ></v-btn>
                <v-btn
                  v-if="auth.isAdmin"
                  icon="mdi-delete-outline"
                  size="small"
                  color="error"
                  variant="text"
                  title="Remover"
                  :loading="docker.actionLoading"
                  @click="runContainerAction(item, 'remove')"
                ></v-btn>
              </div>
            </template>
            <template #mobile-item="{ item }">
              <div class="d-flex align-start justify-space-between ga-2">
                <div class="min-w-0">
                  <div class="font-weight-bold text-truncate">{{ containerName(item) }}</div>
                  <div class="text-caption text-medium-emphasis text-truncate">
                    {{ item.image }}
                  </div>
                </div>
                <v-chip :color="stateColor(item.state)" size="small" variant="tonal">
                  {{ stateLabel(item.state) }}
                </v-chip>
              </div>
            </template>
          </ResponsiveDataTable>
        </v-expansion-panel-text>
      </v-expansion-panel>
    </v-expansion-panels>
    <v-alert v-else type="info" variant="tonal">
      Nenhum container encontrado para os filtros selecionados.
    </v-alert>

    <v-dialog v-model="detailDialog" max-width="980" scrollable>
      <v-card rounded="xl" class="docker-detail-card">
        <v-card-title class="d-flex align-center ga-2">
          <v-icon color="primary">mdi-cube-outline</v-icon>
          <span class="text-truncate">{{ detail?.name || 'Container' }}</span>
          <v-spacer></v-spacer>
          <v-btn icon="mdi-close" variant="text" @click="detailDialog = false"></v-btn>
        </v-card-title>
        <v-tabs v-model="detailTab" color="primary" show-arrows>
          <v-tab value="overview">Detalhes</v-tab>
          <v-tab value="mounts">Volumes</v-tab>
          <v-tab value="networks">Redes</v-tab>
          <v-tab value="logs">Logs</v-tab>
        </v-tabs>
        <v-divider></v-divider>
        <v-card-text class="pa-0 docker-detail-body">
          <v-window v-model="detailTab">
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
                <v-btn prepend-icon="mdi-refresh" :loading="logsLoading" @click="loadLogs()">
                  Atualizar
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
              <pre class="docker-logs">{{ formattedLogs }}</pre>
            </v-window-item>
          </v-window>
        </v-card-text>
      </v-card>
    </v-dialog>

    <v-snackbar v-model="feedback.visible" :color="feedback.color" timeout="4500">
      {{ feedback.message }}
    </v-snackbar>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import PageHeader from '@/components/PageHeader.vue'
import ResponsiveDataTable from '@/components/ResponsiveDataTable.vue'
import { confirm } from '@/composables/useConfirm'
import { dockerService } from '@/services/dockerService'
import { useAuthStore } from '@/stores/auth'
import { useDockerStore } from '@/stores/docker'
import type { DockerContainerDetail } from '@/bindings/DockerContainerDetail'
import type { DockerContainerSummary } from '@/bindings/DockerContainerSummary'
import type { DockerLogEntry } from '@/bindings/DockerLogEntry'
import { formatBinaryBytes } from '@/utils/formatters'

type ContainerActionName = 'start' | 'stop' | 'restart' | 'remove'
interface ContainerGroup {
  key: string
  label: string
  containers: DockerContainerSummary[]
  running: number
  cpuPercent: number
  memoryBytes: number
  standalone: boolean
}

const docker = useDockerStore()
const auth = useAuthStore()
const search = ref('')
const stateFilter = ref('all')
const expandedGroups = ref<string[]>([])
const detailDialog = ref(false)
const detailLoading = ref(false)
const detail = ref<DockerContainerDetail | null>(null)
const detailTab = ref('overview')
const logs = ref<DockerLogEntry[]>([])
const logsLoading = ref(false)
const logsDownloading = ref(false)
const logsClearing = ref(false)
const logTail = ref<number | 'all'>(500)
const networkToConnect = ref<string | null>(null)
const feedback = ref({ visible: false, color: 'success', message: '' })
let detailRefreshing = false
let logsRefreshing = false

const headers = [
  { title: 'Container', key: 'name' },
  { title: 'Imagem', key: 'image' },
  { title: 'Estado', key: 'state', width: '120px' },
  { title: 'Recursos', key: 'resources', width: '130px', sortable: false },
  { title: 'Ações', key: 'actions', width: '220px', sortable: false },
]
const stateOptions = [
  { title: 'Todos', value: 'all' },
  { title: 'Em execução', value: 'running' },
  { title: 'Parados', value: 'stopped' },
]
const logTailOptions = [100, 500, 1000, 5000, { title: 'Todos', value: 'all' }]

const filteredContainers = computed(() => {
  const term = search.value.trim().toLocaleLowerCase()
  return docker.containers.filter((container) => {
    const stateMatches =
      stateFilter.value === 'all' ||
      (stateFilter.value === 'running' && container.state === 'running') ||
      (stateFilter.value === 'stopped' && container.state !== 'running')
    const textMatches =
      !term ||
      [containerName(container), container.image, container.projectName || '']
        .join(' ')
        .toLocaleLowerCase()
        .includes(term)
    return stateMatches && textMatches
  })
})

const containerGroups = computed<ContainerGroup[]>(() => {
  const groups = new Map<string, ContainerGroup>()
  for (const container of filteredContainers.value) {
    const projectName = container.projectName?.trim() || ''
    const key = projectName ? projectName.toLocaleLowerCase() : '__standalone__'
    const metric = metricFor(container.id)
    const existing = groups.get(key)
    if (existing) {
      existing.containers.push(container)
      existing.running += container.state === 'running' ? 1 : 0
      existing.cpuPercent += metric?.cpu.usagePercent ?? 0
      existing.memoryBytes += metric?.memory.usageBytes ?? 0
      continue
    }
    groups.set(key, {
      key,
      label: projectName || 'Containers avulsos',
      containers: [container],
      running: container.state === 'running' ? 1 : 0,
      cpuPercent: metric?.cpu.usagePercent ?? 0,
      memoryBytes: metric?.memory.usageBytes ?? 0,
      standalone: !projectName,
    })
  }
  return [...groups.values()]
    .map((group) => ({
      ...group,
      containers: [...group.containers].sort((left, right) =>
        containerName(left).localeCompare(containerName(right))
      ),
    }))
    .sort((left, right) => {
      if (left.standalone !== right.standalone) return left.standalone ? 1 : -1
      return left.label.localeCompare(right.label)
    })
})

const availableNetworks = computed(() => {
  const connected = new Set((detail.value?.networks ?? []).map((network) => network.networkId))
  return docker.networks
    .filter((network) => !connected.has(network.id))
    .sort((left, right) => left.name.localeCompare(right.name))
})

const formattedLogs = computed(() =>
  logs.value.length
    ? logs.value
        .map(
          (entry) =>
            `${entry.timestamp ? `[${entry.timestamp}] ` : ''}${entry.stream}: ${entry.message}`
        )
        .join('\n')
    : 'Nenhum log encontrado.'
)

watch(
  () => containerGroups.value.map((group) => group.key),
  (keys) => {
    const current = expandedGroups.value.filter((key) => keys.includes(key))
    expandedGroups.value = [...new Set([...current, ...keys])]
  },
  { immediate: true }
)

watch(detailTab, (tab) => {
  if (tab === 'logs' && logs.value.length === 0) void loadLogs()
})

function containerName(container: DockerContainerSummary): string {
  return container.names[0]?.replace(/^\//, '') || shortId(container.id)
}

function shortId(id: string): string {
  return id.replace(/^sha256:/, '').slice(0, 12)
}

function stateColor(state: string): string {
  if (state === 'running') return 'success'
  if (state === 'paused' || state === 'restarting') return 'warning'
  return 'grey'
}

function stateLabel(state: string): string {
  return (
    {
      running: 'Executando',
      exited: 'Parado',
      created: 'Criado',
      paused: 'Pausado',
      restarting: 'Reiniciando',
      dead: 'Inativo',
    }[state] || state
  )
}

function metricFor(id: string) {
  return docker.metrics?.containers.find((metric) => metric.containerId === id)
}

function onRowClick(_event: MouseEvent, row: { item: DockerContainerSummary }): void {
  void openDetail(row.item)
}

async function openDetail(container: DockerContainerSummary): Promise<void> {
  detail.value = null
  detailDialog.value = true
  detailTab.value = 'overview'
  detailLoading.value = true
  logs.value = []
  networkToConnect.value = null
  try {
    detail.value = await dockerService.container(container.id)
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
    detail.value = await dockerService.container(detail.value.id)
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
    logs.value = await dockerService.logs(detail.value.id, {
      tail: logTail.value,
      timestamps: true,
    })
  } catch (reason: unknown) {
    notify(reason instanceof Error ? reason.message : 'Erro ao carregar logs', 'error')
  } finally {
    logsRefreshing = false
    logsLoading.value = false
  }
}

async function downloadLogs(): Promise<void> {
  if (!detail.value) return
  logsDownloading.value = true
  try {
    const entries = await dockerService.logs(detail.value.id, {
      tail: 'all',
      timestamps: true,
    })
    const content = entries
      .map(
        (entry) =>
          `${entry.timestamp ? `[${entry.timestamp}] ` : ''}${entry.stream}: ${entry.message}`
      )
      .join('\n')
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
    const response = await dockerService.clearLogs(detail.value.id)
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
    () => dockerService.connectNetwork(networkId, containerId),
    async () => {
      await refreshDetail(true)
    }
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
    () => dockerService.disconnectNetwork(networkId, containerId),
    async () => {
      await refreshDetail(true)
    }
  )
  notify(
    success ? 'Rede removida do container.' : docker.error || 'Erro ao remover rede',
    success ? 'success' : 'error'
  )
}

function sanitizeFileName(value: string): string {
  return value.replace(/^\//, '').replace(/[^a-zA-Z0-9._-]+/g, '-') || 'container'
}

async function runContainerAction(
  container: DockerContainerSummary,
  action: ContainerActionName
): Promise<void> {
  const name = containerName(container)
  const destructive = action === 'remove' || action === 'stop'
  if (destructive) {
    const accepted = await confirm({
      title: action === 'remove' ? 'Remover container' : 'Parar container',
      message: `${action === 'remove' ? 'Remover' : 'Parar'} o container "${name}"?`,
      confirmText: action === 'remove' ? 'Remover' : 'Parar',
      confirmColor: action === 'remove' ? 'error' : 'warning',
      icon: action === 'remove' ? 'mdi-delete-alert-outline' : 'mdi-stop-circle-outline',
    })
    if (!accepted) return
  }

  let responseMessage = ''
  const success = await docker.runAction(async () => {
    const response = await {
      start: () => dockerService.startContainer(container.id),
      stop: () => dockerService.stopContainer(container.id),
      restart: () => dockerService.restartContainer(container.id),
      remove: () => dockerService.removeContainer(container.id),
    }[action]()
    responseMessage = response.message
  })
  notify(
    success ? responseMessage : docker.error || 'Operação não concluída',
    success ? 'success' : 'error'
  )
}

function notify(message: string, color: string): void {
  feedback.value = { visible: true, color, message }
}
</script>

<style scoped>
.docker-filter {
  max-width: 220px;
}
.docker-group-panel {
  border: 1px solid rgba(var(--v-theme-on-surface), 0.1);
}
.docker-detail-card {
  background: rgb(var(--v-theme-surface));
  color: rgb(var(--v-theme-on-surface));
}
.docker-detail-body {
  min-height: 420px;
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

@media (max-width: 700px) {
  .docker-group-totals {
    display: none;
  }
}
</style>
